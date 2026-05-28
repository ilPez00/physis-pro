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
            let body = std::fs::read_to_string(&path).unwrap_or_default();
            docs.push(IngestedDoc {
                path: path.clone(),
                kind,
                title: path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string(),
                body,
                timestamp: chrono::Utc::now(),
                ontology_tags: Vec::new(),
            });
        }
    }
    Ok(())
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
            },
            IngestedDoc {
                path: PathBuf::from("email.eml"),
                kind: DocKind::Email,
                title: "email".into(),
                body: String::new(),
                timestamp: chrono::Utc::now(),
                ontology_tags: vec![],
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
