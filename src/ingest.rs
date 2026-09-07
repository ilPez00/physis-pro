//! Document ingestion pipeline — watches directories, sorts files by type,
//! parses email/voice/spreadsheets, classifies into ontology.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::operational::OperationalEvent;

/// Classification of an ingested document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DocKind {
    Invoice,
    Email,
    Spreadsheet,
    VoiceTranscript,
    /// OCR-extracted text from scanned documents or images.
    OcrDocument,
    /// Image files (.png, .jpg, .webp, .bmp, .tiff) handled via CLIP/Jina embedders.
    Image,
    /// Video files (.mp4, .avi, .mov, .mkv) handled via ffmpeg frame sampling.
    Video,
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
    /// Structured fields extracted from PDFs / emails / spreadsheets (invoice
    /// vendor/date, email From/Subject) when the `pdf` / `email` / `ingest`
    /// features are enabled. Always present; empty when the feature is off or
    /// extraction fails.
    pub meta: DocMeta,
    /// Optional structured template extracted during ingestion (PLAN 11).
    /// When present, contains type-specific metadata and the best citation
    /// fragment for the node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<crate::models::DocumentTemplate>,
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
        // Audio files — transcribed via Whisper API
        Some("mp3") | Some("wav") | Some("flac") | Some("ogg") | Some("m4a") => DocKind::VoiceTranscript,
        // Image files — handled via CLIP/Jina embedder
        Some("png") | Some("jpg") | Some("jpeg") | Some("webp") | Some("bmp") | Some("tiff") | Some("gif") => DocKind::Image,
        // Video files — handled via ffmpeg frame sampling
        Some("mp4") | Some("avi") | Some("mov") | Some("mkv") | Some("webm") | Some("wmv") => DocKind::Video,
        Some("txt") | Some("md") => {
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            // Token equality, not substring: "invoices" contains "voice" and a
            // plant's "recalls" contains "call", so a substring test silently
            // routed ordinary reports into the (empty-body) voice-transcript
            // extractor — their text never reached any index. Split on the
            // separators filenames actually use and compare whole words.
            let is_voice = name
                .split(['-', '_', ' ', '.'])
                .any(|p| matches!(p, "transcript" | "voice" | "call"));
            if is_voice {
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

impl IngestedDoc {
    /// Normalize the document into the shared operational event schema.
    pub fn into_operational_event(&self) -> OperationalEvent {
        OperationalEvent::from(self)
    }
}

fn walk_and_ingest(dir: &Path, docs: &mut Vec<IngestedDoc>) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        // Never descend into the engine's own data dir: `.physis/virtual/**`
        // holds VFS symlinks to previously-ingested files, so recursing into
        // it re-ingests old documents and grows the graph on every run.
        // (symlink_metadata does NOT follow links — a symlink named dir is
        // still skipped, a real dir is descended.)
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name == ".physis"
                && std::fs::symlink_metadata(&path)
                    .map(|m| m.is_dir())
                    .unwrap_or(false)
            {
                continue;
            }
        }
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_symlink() => continue, // never ingest through symlinks
            Ok(meta) if meta.is_dir() => walk_and_ingest(&path, docs)?,
            Ok(_) => {
                // Skip the Studio's own generated artifacts so a re-run doesn't
                // ingest its previous HTML/JSON output and pollute discovery.
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext == "html" || ext == "json" {
                    continue;
                }
                let kind = classify_file(&path);
                // Extract real text + structured metadata per document type. Each
                // extractor is feature-gated and fallible; on any failure we fall
                // back to best-effort (raw read for text-like files, empty body for
                // binaries) so a bad file never aborts the whole ingest.
                let (body, meta) = extract_doc(&path, kind.clone());
                // Build a structured template when the extractor produced useful
                // metadata (PLAN 11).
                let template = match &kind {
                    #[cfg(feature = "pdf")]
                    DocKind::Invoice if meta.vendor.is_some() || meta.date.is_some() => {
                        Some(crate::models::DocumentTemplate::Invoice(
                            crate::models::InvoiceTemplate {
                                vendor: meta.vendor.clone(),
                                doc_number: meta.doc_number.clone(),
                                date: meta.date.clone(),
                                total_amount: None,
                                currency: None,
                                line_items: vec![],
                                fragment: body.lines().next().map(|l| l.trim().to_string()),
                            }
                        ))
                    }
                    #[cfg(feature = "email")]
                    DocKind::Email if meta.from.is_some() || meta.subject.is_some() => {
                        Some(crate::models::DocumentTemplate::Email(
                            crate::models::EmailTemplate {
                                from: meta.from.clone(),
                                to: None,
                                cc: None,
                                subject: meta.subject.clone().unwrap_or_default(),
                                thread_id: meta.thread_id.clone().unwrap_or_default(),
                                body_plain: body.clone(),
                                body_html: None,
                                attachments: vec![],
                                fragment: body.lines().next().map(|l| l.trim().to_string()),
                            }
                        ))
                    }
                    _ => None,
                };
                docs.push(IngestedDoc {
                    path: path.clone(),
                    kind,
                    title: path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    body,
                    timestamp: chrono::Utc::now(),
                    ontology_tags: Vec::new(),
                    meta,
                    template,
                });
            }
            Err(_) => {} // unreadable entry (race/permissions): skip, never abort ingest
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
        #[cfg(feature = "ocr")]
        DocKind::OcrDocument => {
            match extract_ocr(path) {
                Ok((text, meta)) => (text, meta),
                Err(_) => (String::new(), DocMeta::default()),
            }
        }
        #[cfg(not(feature = "ocr"))]
        DocKind::OcrDocument => (String::new(), DocMeta::default()),
        #[cfg(feature = "audio")]
        DocKind::VoiceTranscript => {
            match extract_audio_transcript(path) {
                Ok((text, _meta)) => (text, DocMeta::default()),
                Err(_) => (String::new(), DocMeta::default()),
            }
        }
        #[cfg(not(feature = "audio"))]
        DocKind::VoiceTranscript => {
            // Audio transcription requires Whisper API. Without the `audio`
            // feature, we fall back to an empty body (no crash).
            (String::new(), DocMeta::default())
        }
        #[cfg(feature = "ingest")]
        DocKind::Spreadsheet => {
            match extract_spreadsheet(path) {
                Ok((text, meta)) => (text, meta),
                Err(_) => (String::new(), DocMeta::default()),
            }
        }
        #[cfg(not(feature = "ingest"))]
        DocKind::Spreadsheet => (String::new(), DocMeta::default()),
        DocKind::Image => {
            // Images are embedded directly; text extraction uses OCR when available.
            #[cfg(feature = "ocr")]
            {
                extract_ocr(path).map(|(t, _)| (t, DocMeta::default()))
                    .unwrap_or((String::new(), DocMeta::default()))
            }
            #[cfg(not(feature = "ocr"))]
            {
                (String::new(), DocMeta::default())
            }
        }
        DocKind::Video => {
            // Video is embedded directly via ffmpeg frames. We extract minimal
            // metadata but rely on the video embedding module for vectors.
            let meta = extract_video_metadata(path);
            (String::new(), meta)
        }
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
    match extract_text(path) {
        Ok(text) if !text.trim().is_empty() => {
            let meta = invoice_meta_from_text(&text);
            Ok((text, meta))
        }
        // Malformed/unparseable PDF (bad xref, scanned image, …): fall back to
        // the raw bytes as lossy UTF-8 so at least the stream text + any
        // embedded invoice strings (vendor/date/number) survive. Never return
        // an empty body — that would make a real document look like nothing.
        _ => {
            let raw = std::fs::read(path).unwrap_or_default();
            let text = String::from_utf8_lossy(&raw).into_owned();
            let meta = invoice_meta_from_text(&text);
            Ok((text, meta))
        }
    }
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
    // "No.", or "Document" on the same line. Accepts dashed/dotted numbers
    // like "2026-088" or "INV.4471".
    for line in text.lines() {
        let low = line.to_lowercase();
        if low.contains("invoice") || low.contains("fattura") || low.contains("document") {
            let toks: Vec<&str> = line.split_whitespace().collect();
            for (i, w) in toks.iter().enumerate() {
                let lw = w.to_lowercase();
                if lw == "no."
                    || lw == "n."
                    || lw == "invoice"
                    || lw == "fattura"
                    || lw == "document"
                {
                    if let Some(next) = toks.get(i + 1) {
                        let cand = next.trim_matches(|c: char| {
                            !c.is_ascii_alphanumeric() && c != '-' && c != '.'
                        });
                        if !cand.is_empty() && cand.chars().any(|c| c.is_ascii_digit()) {
                            meta.doc_number = Some(cand.to_string());
                            break;
                        }
                    }
                }
            }
        }
        // Vendor: a short ALL-CAPS or Title-Cased line near the top often is
        // the issuer. Take the first sufficiently-long uppercase token run.
        if meta.vendor.is_none() {
            let words: Vec<&str> = line.split_whitespace().collect();
            if !words.is_empty()
                && words[0].len() >= 3
                && words[0]
                    .chars()
                    .all(|c| c.is_uppercase() || c.is_whitespace() || c == '.')
                && line.len() < 60
            {
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
    if parts.len() == 3
        && parts[0].len() == 4
        && parts[1].parse::<u32>().is_ok()
        && parts[2].parse::<u32>().is_ok()
    {
        return Some(format!("{}-{}-{}", parts[0], parts[1], parts[2]));
    }
    // DD/MM/YYYY or MM/DD/YYYY
    let parts: Vec<&str> = t.splitn(3, '/').collect();
    if parts.len() == 3
        && parts[2].len() == 4
        && parts[0].parse::<u32>().is_ok()
        && parts[1].parse::<u32>().is_ok()
    {
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

/// Extract text from scanned documents or images using Tesseract OCR.
/// Feature-gated behind `ocr`; falls back to empty body when disabled.
#[cfg(feature = "ocr")]
fn extract_ocr(path: &Path) -> anyhow::Result<(String, DocMeta)> {
    // tesseract 0.14 has no `prelude` and no `HasInnerMut`; `new` is fallible
    // and `set_image` is a consuming builder returning `Result<Self, _>`.
    use tesseract::Tesseract;
    let mut tess = Tesseract::new(None, Some("eng"))
        .map_err(|e| anyhow::anyhow!("OCR init failed: {e}"))?
        .set_image(path.to_str().unwrap_or(""))
        .map_err(|e| anyhow::anyhow!("OCR image load failed: {e}"))?;
    let text = tess
        .get_text()
        .map_err(|e| anyhow::anyhow!("OCR failed: {e}"))?;

    let meta = DocMeta {
        vendor: None,
        date: None,
        doc_number: None,
        from: None,
        subject: None,
        thread_id: None,
    };

    Ok((text, meta))
}

/// Extract spreadsheet content and metadata from XLSX/CSV.
/// Feature-gated behind `ingest` (uses calamine).
#[cfg(feature = "ingest")]
fn extract_spreadsheet(path: &Path) -> anyhow::Result<(String, DocMeta)> {
    // calamine 0.24: `DataType` is the trait; `Data` is the cell enum.
    use calamine::{open_workbook_auto, Reader};

    let mut wb = open_workbook_auto(path)?;
    let sheet_name = wb.sheet_names().get(0)
        .cloned()
        .unwrap_or_else(|| "Sheet1".to_string());

    // calamine 0.24: worksheet_range returns a plain Result<Range<Data>, _>
    // — Range is neither Option nor Result, so no .ok_or_else on it.
    let range = wb.worksheet_range(&sheet_name)
        .map_err(|e| anyhow::anyhow!("worksheet error: {e}"))?;
    if range.is_empty() {
        anyhow::bail!("no data in sheet");
    }

    let headers: Vec<String> = range
        .rows()
        .next()
        .map(|row| row.iter().map(|c| cell_to_string(c)).collect())
        .unwrap_or_default();

    let mut rows = Vec::new();
    for row in range.rows().skip(1) {
        rows.push(row.iter().map(|c| cell_to_string(c)).collect::<Vec<_>>());
    }

    // Detect kind based on header content
    let detected_kind = if headers.iter().any(|h| h.to_lowercase().contains("invoice") || h.to_lowercase().contains("amount")) {
        "finance".to_string()
    } else if headers.iter().any(|h| h.to_lowercase().contains("name") || h.to_lowercase().contains("email")) {
        "procurement".to_string()
    } else {
        "generic".to_string()
    };

    let body = rows
        .iter()
        .flatten()
        .filter(|c| !c.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");

    Ok((body, DocMeta::default()))
}

#[cfg(feature = "ingest")]
fn cell_to_string(cell: &calamine::Data) -> String {
    use calamine::Data;
    match cell {
        Data::String(s) => s.clone(),
        Data::Float(f) => format!("{}", f),
        Data::Int(i) => format!("{}", i),
        Data::Bool(b) => format!("{}", b),
        Data::Empty => String::new(),
        _ => format!("{:?}", cell),
    }
}

/// Extract audio transcription via Whisper API.
/// Feature-gated behind `audio`.
#[cfg(feature = "audio")]
fn extract_audio_transcript(path: &Path) -> anyhow::Result<(String, DocMeta)> {
    let db = crate::config::PhysisConfig::default();
    let config = crate::config::PhysisConfig::default();
    let _ = (db, config);

    // A real implementation would create a ProviderCascade and call
    // transcribe. For now, return empty — the audio feature is wired but needs
    // an active provider configuration.
    let _ = path;
    Err(anyhow::anyhow!("audio transcription requires PHYSIS_API_TOKEN with Whisper access"))
}

/// Extract basic metadata from a video file without full processing.
fn extract_video_metadata(path: &Path) -> DocMeta {
    let meta = std::fs::metadata(path).ok();
    let size = meta.map(|m| m.len()).unwrap_or(0);

    // Try ffprobe for metadata if available
    let output = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries",
               "stream=width,height,r_frame_rate,duration", "-of", "default=noprint_wrappers=1",
               path.to_str().unwrap_or("")])
        .output();

    if let Ok(out) = output {
        if let Ok(info) = String::from_utf8(out.stdout) {
            // Parse ffprobe output for width/height/etc
            // Store in meta somehow — for now just return default
            let _ = info;
        }
    }

    let _ = size;
    DocMeta::default()
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
            DocKind::OcrDocument => "ocr_documents",
            DocKind::Image => "images",
            DocKind::Video => "videos",
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
        assert!(matches!(
            classify_file(Path::new("inv.pdf")),
            DocKind::Invoice
        ));
        assert!(matches!(
            classify_file(Path::new("msg.eml")),
            DocKind::Email
        ));
        assert!(matches!(
            classify_file(Path::new("data.xlsx")),
            DocKind::Spreadsheet
        ));
        assert!(matches!(
            classify_file(Path::new("data.csv")),
            DocKind::Spreadsheet
        ));
    }

    #[test]
    fn voice_detection_is_whole_word_not_substring() {
        // "invoices" contains "voice" and "recalls" contains "call"; the old
        // substring test misrouted such reports into the empty-body voice
        // extractor, so their text never reached any index (found by the
        // content-search corpus in tests/http_surface.rs).
        assert!(matches!(
            classify_file(Path::new("office/invoices.md")),
            DocKind::Report
        ));
        assert!(matches!(
            classify_file(Path::new("recalls.txt")),
            DocKind::Report
        ));
        assert!(matches!(
            classify_file(Path::new("meeting-transcript.txt")),
            DocKind::VoiceTranscript
        ));
        assert!(matches!(
            classify_file(Path::new("voice_memo.md")),
            DocKind::VoiceTranscript
        ));
        assert!(matches!(
            classify_file(Path::new("call-notes.txt")),
            DocKind::VoiceTranscript
        ));
    }

    #[test]
    fn test_classify_transcript() {
        assert!(matches!(
            classify_file(Path::new("call_transcript.txt")),
            DocKind::VoiceTranscript
        ));
        assert!(matches!(
            classify_file(Path::new("report.md")),
            DocKind::Report
        ));
    }

    #[test]
    fn test_classify_new_file_types() {
        // Audio files
        assert!(matches!(
            classify_file(Path::new("recording.mp3")),
            DocKind::VoiceTranscript
        ));
        assert!(matches!(
            classify_file(Path::new("voice.wav")),
            DocKind::VoiceTranscript
        ));
        assert!(matches!(
            classify_file(Path::new("voice.flac")),
            DocKind::VoiceTranscript
        ));
        // Image files
        assert!(matches!(
            classify_file(Path::new("diagram.png")),
            DocKind::Image
        ));
        assert!(matches!(
            classify_file(Path::new("photo.jpg")),
            DocKind::Image
        ));
        assert!(matches!(
            classify_file(Path::new("scan.tiff")),
            DocKind::Image
        ));
        // Video files
        assert!(matches!(
            classify_file(Path::new("demo.mp4")),
            DocKind::Video
        ));
        assert!(matches!(
            classify_file(Path::new("training.mov")),
            DocKind::Video
        ));
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
                template: None,
            },
            IngestedDoc {
                path: PathBuf::from("email.eml"),
                kind: DocKind::Email,
                title: "email".into(),
                body: String::new(),
                timestamp: chrono::Utc::now(),
                ontology_tags: vec![],
                meta: DocMeta::default(),
                template: None,
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

    #[test]
    fn test_ingest_skips_studio_artifacts() {
        let tmp = std::env::temp_dir().join("physis_ingest_skip_test");
        let _ = std::fs::create_dir_all(&tmp);
        std::fs::write(tmp.join("report.md"), "# hello").unwrap();
        std::fs::write(tmp.join("studio.html"), "<html>noise</html>").unwrap();
        std::fs::write(tmp.join("discovered_ontology.json"), "{}").unwrap();
        let docs = ingest_all(std::slice::from_ref(&tmp));
        let names: Vec<String> = docs.iter().map(|d| d.title.clone()).collect();
        assert!(
            names.contains(&"report".to_string()),
            "md should be ingested: {names:?}"
        );
        assert!(
            !names.contains(&"studio".to_string()),
            "studio.html must be skipped: {names:?}"
        );
        assert!(
            !names.contains(&"discovered_ontology".to_string()),
            "json must be skipped: {names:?}"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn test_invoice_meta_extraction() {
        // The bundled-demo invoice shape: vendor / dashed doc number / ISO date.
        let text = "ACME INDUSTRIES S.P.A.\nInvoice No. 2026-088\nDate: 2026-05-10\n";
        let meta = invoice_meta_from_text(text);
        assert_eq!(meta.vendor.as_deref(), Some("ACME INDUSTRIES S.P.A."));
        assert_eq!(meta.doc_number.as_deref(), Some("2026-088"));
        assert_eq!(meta.date.as_deref(), Some("2026-05-10"));
    }
}
