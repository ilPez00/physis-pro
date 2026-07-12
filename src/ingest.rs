//! Document ingestion pipeline — watches directories, sorts files by type,
//! parses email/voice/spreadsheets, classifies into ontology.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Classification of an ingested document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocKind {
    Invoice,
    Email,
    Spreadsheet,
    VoiceTranscript,
    Report,
    Unknown,
}

/// An ingested document with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestedDoc {
    pub path: PathBuf,
    pub kind: DocKind,
    pub title: String,
    pub body: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub ontology_tags: Vec<String>,
    /// Structured fields extracted from PDFs / emails (invoice vendor/date,
    /// email From/Subject) when the `pdf` / `email` features are enabled.
    /// Always present; empty when the feature is off or extraction fails.
    pub meta: DocMeta,
}

/// Optional structured metadata for a document, populated by the `pdf` and
/// `email` ingestion backends. Keeping it separate from `body` means the rest
/// of the engine (embeddings, graph, discovery) just uses `body`, while the
/// Studio UI can show "Vendor: Acme · 2026-05-12" without parsing the text.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocMeta {
    /// PDF invoice / order: vendor or issuer name.
    pub vendor: Option<String>,
    /// PDF invoice / order: issue date (ISO, if parseable).
    pub date: Option<String>,
    /// PDF invoice / order: invoice or document number.
    pub doc_number: Option<String>,
    /// Email: sender address.
    pub from: Option<String>,
    /// Email: subject line.
    pub subject: Option<String>,
    /// Email: a short thread-id (Message-ID or normalized subject) so the
    /// Studio can group replies.
    pub thread_id: Option<String>,
}

/// Classify a file by extension into DocKind.
pub fn classify_file(path: &Path) -> DocKind {
    match path.extension().and_then(|e| e.to_str()) {
        Some("pdf") => DocKind::Invoice,
        Some("eml") | Some("msg") => DocKind::Email,
        Some("xlsx") | Some("xls") | Some("csv") => DocKind::Spreadsheet,
        Some("txt") | Some("md") => {
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if name.contains("transcript") || name.contains("voice") || name.contains("call") {
                DocKind::VoiceTranscript
            } else {
                DocKind::Report
            }
        }
        _ => DocKind::Unknown,
    }
}

/// Ingest all documents from a list of watch directories.
pub fn ingest_all(watch_dirs: &[PathBuf]) -> Vec<IngestedDoc> {
    let mut docs = Vec::new();
    for dir in watch_dirs {
        if !dir.exists() {
            continue;
        }
        let _ = walk_and_ingest(dir, &mut docs);
    }
    docs
}

fn walk_and_ingest(dir: &Path, docs: &mut Vec<IngestedDoc>) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_and_ingest(&path, docs)?;
        } else {
            let kind = classify_file(&path);
            // Extract real text + structured metadata per document type. Each
            // extractor is feature-gated and fallible; on any failure we fall
            // back to best-effort (raw read for text-like files, empty body for
            // binaries) so a bad file never aborts the whole ingest.
            let (body, meta) = extract_doc(&path, kind.clone());
            docs.push(IngestedDoc {
                path: path.clone(),
                kind,
                title: path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string(),
                body,
                timestamp: chrono::Utc::now(),
                ontology_tags: Vec::new(),
                meta,
            });
        }
    }
    Ok(())
}

/// Extract text body + structured metadata for one document.
/// PDF/email paths use the `pdf`/`email` feature backends; everything else
/// (txt/md/csv/log) is a plain UTF-8 read. Spreadsheets stay raw unless the
/// `ingest` feature (calamine) is on — kept simple here; the backoffice layer
/// handles XLSX semantics separately.
fn extract_doc(path: &Path, kind: DocKind) -> (String, DocMeta) {
    match kind {
        #[cfg(feature = "pdf")]
        DocKind::Invoice => {
            // .pdf → real text. (Invoice is the only PDF shape classify_file
            // emits today; if other PDF kinds appear, branch here.)
            match extract_pdf(path) {
                Ok((text, meta)) => (text, meta),
                Err(_) => (String::new(), DocMeta::default()),
            }
        }
        #[cfg(not(feature = "pdf"))]
        DocKind::Invoice => (String::new(), DocMeta::default()),
        #[cfg(feature = "email")]
        DocKind::Email => match extract_email(path) {
            Ok((text, meta)) => (text, meta),
            Err(_) => (String::new(), DocMeta::default()),
        },
        #[cfg(not(feature = "email"))]
        DocKind::Email => (String::new(), DocMeta::default()),
        _ => {
            // Plain text-like files: best-effort UTF-8.
            let raw = std::fs::read_to_string(path).unwrap_or_default();
            (raw, DocMeta::default())
        }
    }
}

/// Extract text + invoice metadata from a PDF using `pdf-extract`.
/// Wrapped in a single function so the feature gate keeps the dependency out
/// of builds that don't enable `pdf`.
#[cfg(feature = "pdf")]
fn extract_pdf(path: &Path) -> anyhow::Result<(String, DocMeta)> {
    use pdf_extract::extract_text;
    let text = extract_text(path)
        .map_err(|e| anyhow::anyhow!("pdf text extraction failed: {e}"))?;
    let meta = invoice_meta_from_text(&text);
    Ok((text, meta))
}

/// Heuristic invoice metadata: pull a vendor line, an ISO-ish date, and a
/// document number. Deliberately lightweight (regex-free) — good enough to
/// show "Vendor · Date" in the Studio without a full invoice OCR pipeline.
#[cfg(feature = "pdf")]
fn invoice_meta_from_text(text: &str) -> DocMeta {
    let mut meta = DocMeta::default();
    // Date: first token that looks like YYYY-MM-DD or DD/MM/YYYY.
    for tok in text.split_whitespace() {
        if let Some(d) = parse_flexible_date(tok) {
            meta.date = Some(d);
            break;
        }
    }
    // Invoice / document number: a token following "Invoice", "Fattura", "N.",
    // or "No." on the same line.
    for line in text.lines() {
        let low = line.to_lowercase();
        if low.contains("invoice") || low.contains("fattura") || low.contains("document") {
            if let Some(num) = line.split_whitespace().find(|w| w.contains('#') || w.parse::<u32>().is_ok()) {
                meta.doc_number = Some(num.to_string());
            }
        }
        // Vendor: a short ALL-CAPS or Title-Cased line near the top often is
        // the issuer. Take the first sufficiently-long uppercase token run.
        if meta.vendor.is_none() {
            let words: Vec<&str> = line.split_whitespace().collect();
            if !words.is_empty() && words[0].len() >= 3 && words[0].chars().all(|c| c.is_uppercase() || c.is_whitespace() || c == '.') && line.len() < 60 {
                meta.vendor = Some(line.trim().to_string());
            }
        }
    }
    meta
}

/// Parse a few common date shapes into ISO `YYYY-MM-DD`. Returns None if none match.
#[cfg(feature = "pdf")]
fn parse_flexible_date(tok: &str) -> Option<String> {
    let t = tok.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '/');
    // YYYY-MM-DD
    let parts: Vec<&str> = t.splitn(3, '-').collect();
    if parts.len() == 3 && parts[0].len() == 4 && parts[1].parse::<u32>().is_ok() && parts[2].parse::<u32>().is_ok() {
        return Some(format!("{}-{}-{}", parts[0], parts[1], parts[2]));
    }
    // DD/MM/YYYY or MM/DD/YYYY
    let parts: Vec<&str> = t.splitn(3, '/').collect();
    if parts.len() == 3 && parts[2].len() == 4 && parts[0].parse::<u32>().is_ok() && parts[1].parse::<u32>().is_ok() {
        return Some(format!("{}-{}-{}", parts[2], parts[0], parts[1]));
    }
    None
}

/// Extract body + headers from an RFC-822 (.eml) message using `mailparse`.
#[cfg(feature = "email")]
fn extract_email(path: &Path) -> anyhow::Result<(String, DocMeta)> {
    use mailparse::{parse_mail, MailHeaderMap, MailParseError};
    let raw = std::fs::read(path).map_err(|e| anyhow::anyhow!("read email: {e}"))?;
    let mail = parse_mail(&raw).map_err(|e: MailParseError| anyhow::anyhow!("parse email: {e}"))?;
    // Walk parts for the first text/plain body; concatenate if multipart.
    let mut body = String::new();
    collect_text_plain(&mail, &mut body);
    let headers = mail.get_headers();
    let get = |n: &str| headers.get_first_value(n).filter(|s| !s.is_empty());
    let from = get("From");
    let subject = get("Subject");
    let mid = get("Message-ID");
    let thread_id = mid.or_else(|| subject.as_ref().map(|s| normalize_thread(s)));
    let meta = DocMeta {
        from,
        subject,
        thread_id,
        ..Default::default()
    };
    Ok((body, meta))
}

#[cfg(feature = "email")]
fn collect_text_plain(mail: &mailparse::ParsedMail, out: &mut String) {
    if mail.subparts.is_empty() {
        if mail.ctype.mimetype.starts_with("text/plain") {
            if let Ok(s) = mail.get_body() {
                out.push_str(&s);
                out.push('\n');
            }
        }
    } else {
        for p in &mail.subparts {
            collect_text_plain(p, out);
        }
    }
}

#[cfg(feature = "email")]
fn normalize_thread(subject: &str) -> String {
    // Strip a leading "Re:"/"Fwd:" and collapse whitespace so replies group.
    subject
        .trim_start_matches("Re:")
        .trim_start_matches("Fwd:")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Sort documents by kind, returning each group.
pub fn sort_by_kind(docs: &[IngestedDoc]) -> HashMap<&'static str, Vec<&IngestedDoc>> {
    let mut map: HashMap<&'static str, Vec<&IngestedDoc>> = HashMap::new();
    for doc in docs {
        let key = match doc.kind {
            DocKind::Invoice => "invoices",
            DocKind::Email => "emails",
            DocKind::Spreadsheet => "spreadsheets",
            DocKind::VoiceTranscript => "transcripts",
            DocKind::Report => "reports",
            DocKind::Unknown => "unknown",
        };
        map.entry(key).or_default().push(doc);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_by_extension() {
        assert!(matches!(classify_file(Path::new("inv.pdf")), DocKind::Invoice));
        assert!(matches!(classify_file(Path::new("msg.eml")), DocKind::Email));
        assert!(matches!(classify_file(Path::new("data.xlsx")), DocKind::Spreadsheet));
        assert!(matches!(classify_file(Path::new("data.csv")), DocKind::Spreadsheet));
    }

    #[test]
    fn test_classify_transcript() {
        assert!(matches!(classify_file(Path::new("call_transcript.txt")), DocKind::VoiceTranscript));
        assert!(matches!(classify_file(Path::new("report.md")), DocKind::Report));
    }

    #[test]
    fn test_sort_by_kind_empty() {
        let sorted = sort_by_kind(&[]);
        assert!(sorted.is_empty());
    }

    #[test]
    fn test_sort_by_kind_groups() {
        let docs = vec![
            IngestedDoc {
                path: PathBuf::from("inv.pdf"),
                kind: DocKind::Invoice,
                title: "inv".into(),
                body: String::new(),
                timestamp: chrono::Utc::now(),
                ontology_tags: vec![],
                meta: DocMeta::default(),
            },
            IngestedDoc {
                path: PathBuf::from("email.eml"),
                kind: DocKind::Email,
                title: "email".into(),
                body: String::new(),
                timestamp: chrono::Utc::now(),
                ontology_tags: vec![],
                meta: DocMeta::default(),
            },
        ];
        let sorted = sort_by_kind(&docs);
        assert_eq!(sorted.get("invoices").map(|v| v.len()), Some(1));
        assert_eq!(sorted.get("emails").map(|v| v.len()), Some(1));
    }

    #[test]
    fn test_ingest_nonexistent_dir() {
        let docs = ingest_all(&[PathBuf::from("/nonexistent/ingest/path")]);
        assert!(docs.is_empty());
    }
}
