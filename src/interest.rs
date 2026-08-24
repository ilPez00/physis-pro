//! Interest engine — observing what a user actually works on and distilling
//! it into an interest profile over the ontological grid.
//!
//! Three observation sources feed one stream of [`ObservedItem`]s:
//! - **Clipboard** captures (copy/paste history), deduplicated by content hash;
//! - **Documents** (.pptx/.docx/.txt/.md/…) analyzed for text + metadata;
//! - **File events** from watched directories (the Study Log pattern).
//!
//! The profile aggregates domain scores with time decay, so it answers
//! "what has this person been into lately" rather than "what did they ever
//! touch once". Pure module: no OS access here — the daemon binary feeds it,
//! tests inject stub scorers.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// How long an observation keeps full weight before decay sets in.
const HALF_LIFE_DAYS: f64 = 14.0;
/// Cap on stored observations per tenant (same ceiling philosophy as history).
pub const EVENTS_CAP: usize = 5000;
/// Preview length for clipboard captures — enough to classify, not enough to leak secrets.
pub const PREVIEW_CHARS: usize = 280;

// ── Clipboard ────────────────────────────────────────────────────────────────

/// One copy event. Full text never persists — only a preview plus the hash
/// that lets us dedupe repeated copies of the same thing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardCapture {
    pub at: DateTime<Utc>,
    /// Total length of the copied text (chars).
    pub char_count: usize,
    /// SHA-256 of the full text, hex-encoded.
    pub hash: String,
    /// First [`PREVIEW_CHARS`] characters, newlines flattened.
    pub preview: String,
}

impl ClipboardCapture {
    pub fn capture(at: DateTime<Utc>, text: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        let flat: String = text
            .chars()
            .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
            .collect();
        let preview: String = flat.chars().take(PREVIEW_CHARS).collect();
        Self {
            at,
            char_count: text.chars().count(),
            hash,
            preview,
        }
    }
}

// ── Document analysis ────────────────────────────────────────────────────────

/// Text + metadata pulled out of a document. Metadata is whatever the format
/// honestly offers (OOXML core properties, line/word counts); we never guess.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocAnalysis {
    pub path: String,
    /// Lowercased extension without dot ("md", "pptx", …).
    pub kind: String,
    /// Best-known title: OOXML dc:title, else first heading/line, else file name.
    pub title: String,
    pub word_count: usize,
    pub metadata: BTreeMap<String, String>,
    /// First ~1200 chars of extracted text, newlines flattened — classification input.
    pub excerpt: String,
}

/// Extensions the analyzer can actually read. Anything else → None.
pub fn analyzable(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("txt") | Some("md") | Some("csv") | Some("log") | Some("pptx") | Some("docx")
    )
}

/// Extract text + metadata from a document file.
pub fn analyze_document(path: &Path) -> anyhow::Result<DocAnalysis> {
    let kind = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match kind.as_str() {
        // Plain text family: read directly, metadata from shape of the text.
        "txt" | "md" | "csv" | "log" => {
            let raw = std::fs::read_to_string(path)?;
            Ok(plaintext_analysis(path, &kind, &raw))
        }
        #[cfg(feature = "daemon")]
        "pptx" | "docx" => ooxml_analysis(path, &kind),
        #[cfg(not(feature = "daemon"))]
        "pptx" | "docx" => Err(anyhow::anyhow!(
            "OOXML analysis requires the `daemon` feature"
        )),
        other => Err(anyhow::anyhow!("no analyzer for '.{other}'")),
    }
}

fn plaintext_analysis(path: &Path, kind: &str, raw: &str) -> DocAnalysis {
    let mut metadata = BTreeMap::new();
    metadata.insert("lines".into(), raw.lines().count().to_string());
    let words = raw.split_whitespace().count();
    let title = raw
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim_start_matches('#').trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.chars().take(120).collect::<String>())
        .unwrap_or_else(|| fallback_title(path));
    DocAnalysis {
        path: path.display().to_string(),
        kind: kind.into(),
        title,
        word_count: words,
        metadata,
        excerpt: flatten_excerpt(raw),
    }
}

fn fallback_title(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled")
        .to_string()
}

fn flatten_excerpt(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|c| {
            if c == '\n' || c == '\r' || c == '\t' {
                ' '
            } else {
                c
            }
        })
        .collect();
    flat.chars().take(1200).collect()
}

/// OOXML (.pptx/.docx) = ZIP of XML parts. We pull the main document part's
/// text runs plus `docProps/core.xml` metadata (title, author, dates).
#[cfg(feature = "daemon")]
fn ooxml_analysis(path: &Path, kind: &str) -> anyhow::Result<DocAnalysis> {
    use std::io::Read;

    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file)?;

    // Which part holds body text?
    let body_part = match kind {
        "docx" => "word/document.xml",
        _ => "ppt/slides/slide1.xml",
    };

    let mut metadata = BTreeMap::new();
    if let Ok(mut core) = zip.by_name("docProps/core.xml") {
        let mut xml = String::new();
        core.read_to_string(&mut xml)?;
        for key in [
            "dc:title",
            "dc:creator",
            "cp:lastModifiedBy",
            "dcterms:created",
            "dcterms:modified",
        ] {
            if let Some(value) = xml_tag_value(&xml, key) {
                metadata.insert(key.rsplit(':').next().unwrap_or(key).to_string(), value);
            }
        }
    }
    // Slide count for decks (cheap provenance signal).
    if kind == "pptx" {
        let slides = (1..)
            .take_while(|i| zip.by_name(&format!("ppt/slides/slide{i}.xml")).is_ok())
            .count();
        metadata.insert("slides".into(), slides.to_string());
    }

    let mut body = String::new();
    if let Ok(mut part) = zip.by_name(body_part) {
        part.read_to_string(&mut body)?;
    }

    let text = xml_run_text(&body);
    let title = metadata
        .get("title")
        .cloned()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| {
            text.lines()
                .find(|l| !l.trim().is_empty())
                .map(|l| l.trim().chars().take(120).collect())
                .unwrap_or_else(|| fallback_title(path))
        });

    Ok(DocAnalysis {
        path: path.display().to_string(),
        kind: kind.into(),
        title,
        word_count: text.split_whitespace().count(),
        metadata,
        excerpt: flatten_excerpt(&text),
    })
}

/// First value of an XML element by tag name (namespace-prefixed as written).
#[cfg(feature = "daemon")]
fn xml_tag_value(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = xml.find(&open)? + open.len();
    let after_gt = xml[start..].find('>')? + start + 1;
    let end_rel = xml[after_gt..].find(r"</")?;
    let raw = &xml[after_gt..after_gt + end_rel];
    let decoded = raw
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'");
    let trimmed = decoded.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Pull human-readable text out of OOXML markup (`<w:t>` runs in Word,
/// `<a:t>` in PowerPoint): keep only the character data inside those nodes,
/// inserting spaces at run boundaries.
#[cfg(feature = "daemon")]
fn xml_run_text(xml: &str) -> String {
    fn is_text_tag(tag: &str) -> bool {
        for prefix in ["a:t", "w:t"] {
            if tag == prefix
                || tag.starts_with(&format!("{prefix} "))
                || tag.starts_with(&format!("{prefix}>"))
            {
                return true;
            }
        }
        false
    }

    let mut out = String::new();
    let mut i = 0usize;
    while let Some(rel) = xml[i..].find('<') {
        let open = i + rel;
        let Some(close_rel) = xml[open..].find('>') else {
            break;
        };
        let close = open + close_rel;
        let tag = &xml[open + 1..close];
        if !tag.starts_with('/') && is_text_tag(tag) {
            // Capture character data up to the matching close tag.
            let body_start = close + 1;
            if let Some(end_rel) = xml[body_start..].find("</") {
                out.push_str(&xml[body_start..body_start + end_rel]);
                out.push(' ');
                // Skip past the close tag itself.
                if let Some(after_close) = xml[body_start + end_rel..].find('>') {
                    i = body_start + end_rel + after_close + 1;
                    continue;
                }
                break;
            }
            break;
        }
        i = close + 1;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ── Observation stream + profile ─────────────────────────────────────────────

/// What kind of thing was observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationKind {
    Paste,
    Document,
    FileEvent,
}

/// One observed moment in the user's workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedItem {
    pub at: DateTime<Utc>,
    pub kind: ObservationKind,
    /// Short human label (doc title, paste preview head, event description).
    pub label: String,
    /// Text used for classification (excerpt/preview).
    pub text: String,
    /// Free-form extra facts (path, char_count, slides, …).
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl ObservedItem {
    pub fn from_clipboard(cap: &ClipboardCapture) -> Self {
        Self {
            at: cap.at,
            kind: ObservationKind::Paste,
            label: cap.preview.chars().take(60).collect(),
            text: cap.preview.clone(),
            metadata: BTreeMap::from([
                ("char_count".into(), cap.char_count.to_string()),
                ("hash".into(), cap.hash[..16].to_string()),
            ]),
        }
    }

    pub fn from_document(a: &DocAnalysis) -> Self {
        Self {
            at: Utc::now(),
            kind: ObservationKind::Document,
            label: a.title.clone(),
            text: a.excerpt.clone(),
            metadata: a.metadata.clone(),
        }
    }
}

/// One domain's standing in the profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterestEntry {
    pub domain: String,
    /// Decayed, normalized score (0..1]; entries sorted descending.
    pub score: f64,
    /// How many observations backed this entry.
    pub samples: u32,
}

/// The distilled answer to "what is this user into?".
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InterestProfile {
    pub updated_at: Option<DateTime<Utc>>,
    pub total_observations: u64,
    pub entries: Vec<InterestEntry>,
}

/// Weight an observation carries given its age (exponential half-life decay).
fn age_weight(observed_at: DateTime<Utc>) -> f64 {
    let age_days = (Utc::now() - observed_at).num_seconds().max(0) as f64 / 86_400.0;
    0.5_f64.powf(age_days / HALF_LIFE_DAYS)
}

/// Fold observations into the profile. `score_text` maps an item's text onto
/// weighted ontology domains — the real daemon supplies the shared
/// CellClassifier, tests supply deterministic stubs. Scores are re-normalized
/// each pass, so the profile always sums to ~1 across domains.
pub fn update_profile(
    profile: &mut InterestProfile,
    items: &[ObservedItem],
    score_text: &dyn Fn(&str) -> Vec<(String, f32)>,
) {
    let mut acc: BTreeMap<String, (f64, u32)> = BTreeMap::new();

    for item in items {
        // The observation happened even if nothing classified it — the profile
        // counts activity separately from scored interest.
        profile.total_observations += 1;
        let w = age_weight(item.at);
        let hits = score_text(&item.text);
        let mass: f32 = hits.iter().map(|(_, s)| *s).sum();
        if mass <= 0.0 {
            continue;
        }
        for (domain, s) in hits {
            let e = acc.entry(domain).or_default();
            e.0 += (s / mass) as f64 * w;
            e.1 += 1;
        }
    }

    let total: f64 = acc.values().map(|(s, _)| *s).sum();
    let mut entries: Vec<InterestEntry> = acc
        .into_iter()
        .map(|(domain, (score, samples))| InterestEntry {
            domain,
            score: if total > 0.0 { score / total } else { 0.0 },
            samples,
        })
        .collect();
    entries.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    profile.entries = entries;
    profile.updated_at = Some(Utc::now());
}

// ── Persistence (sled, per tenant) ───────────────────────────────────────────

pub fn events_key(uid: &str) -> String {
    format!("interest:{uid}:events")
}

pub fn save_items(db: &sled::Db, uid: &str, items: &[ObservedItem]) -> anyhow::Result<()> {
    db.insert(events_key(uid), serde_json::to_vec(items)?)?;
    Ok(())
}

pub fn load_items(db: &sled::Db, uid: &str) -> Vec<ObservedItem> {
    db.get(events_key(uid))
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_slice::<Vec<ObservedItem>>(&v).ok())
        .unwrap_or_default()
}

/// Append observations, capping at the newest [`EVENTS_CAP`]. Returns new total.
pub fn append_items(
    db: &sled::Db,
    uid: &str,
    mut incoming: Vec<ObservedItem>,
) -> anyhow::Result<usize> {
    let mut all = load_items(db, uid);
    all.append(&mut incoming);
    if all.len() > EVENTS_CAP {
        all = all[all.len() - EVENTS_CAP..].to_vec();
    }
    save_items(db, uid, &all)?;
    Ok(all.len())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn days_ago(n: i64) -> DateTime<Utc> {
        Utc::now() - Duration::days(n)
    }

    #[test]
    fn clipboard_captures_hash_and_preview_without_full_text() {
        let long_secret: String = "x".repeat(5000);
        let combined = format!("token ghp_abcdef\nsecond line {long_secret}");
        let cap = ClipboardCapture::capture(days_ago(0), &combined);

        assert_eq!(cap.char_count, combined.chars().count());
        assert!(cap.preview.chars().count() <= PREVIEW_CHARS);
        assert!(!cap.preview.contains('\n'));
        assert_eq!(cap.hash.len(), 64);
        // Same content → same hash; different content → different.
        assert_eq!(
            cap.hash,
            ClipboardCapture::capture(Utc::now(), &combined).hash
        );
        assert_ne!(
            cap.hash,
            ClipboardCapture::capture(Utc::now(), "other").hash
        );
    }

    #[test]
    fn plaintext_documents_get_shape_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("notes.md");
        std::fs::write(&p, "# Kernel Bypass Networking\n\nZero-copy matters.\n").unwrap();
        let a = analyze_document(&p).unwrap();
        assert_eq!(a.kind, "md");
        assert_eq!(a.title, "Kernel Bypass Networking");
        assert!(a.word_count >= 4);
        assert!(a.metadata.contains_key("lines"));
        assert!(a.excerpt.contains("Zero-copy"));
    }

    #[test]
    fn unknown_extension_is_a_clean_error() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("binary.exe");
        std::fs::write(&p, b"MZ").unwrap();
        let err = analyze_document(&p).unwrap_err().to_string();
        assert!(err.contains("no analyzer"), "{err}");
        assert!(!analyzable(&p));
    }

    #[cfg(feature = "daemon")]
    #[test]
    fn ooxml_extraction_reads_runs_and_core_metadata() {
        use std::io::Write;

        // Build a minimal .docx by hand: body with two runs + core properties.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.docx");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        zip.add_directory("word/", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.start_file(
            "word/document.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(
            br#"<?xml version="1.0"?><w:document><w:body>
               <w:p><w:r><w:t>Semiotic</w:t></w:r><w:r><w:t>Grid Handbook</w:t></w:r></w:p>
               <w:p><w:r><w:t>Chapter one.</w:t></w:r></w:p>
               </w:body></w:document>"#,
        )
        .unwrap();
        zip.start_file(
            "docProps/core.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(
            br#"<?xml version="1.0"?><cp:coreProperties xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties">
               <dc:title>Grid Spec</dc:title><dc:creator>Gio</dc:creator>
               </cp:coreProperties>"#,
        )
        .unwrap();
        zip.finish().unwrap();

        let a = analyze_document(&path).unwrap();
        assert_eq!(a.kind, "docx");
        assert_eq!(a.title, "Grid Spec", "dc:title wins over body text");
        assert_eq!(a.metadata.get("creator").map(String::as_str), Some("Gio"));
        assert!(a.excerpt.contains("Semiotic Grid Handbook"));
        assert!(a.excerpt.contains("Chapter one."));
    }

    #[test]
    fn profile_decays_old_interests_and_normalizes() {
        let mut profile = InterestProfile::default();
        // Stub scorer: "rust" → compiler domain, "invoice" → finance domain.
        let scorer = |text: &str| -> Vec<(String, f32)> {
            let t = text.to_lowercase();
            let mut v = Vec::new();
            if t.contains("rust") {
                v.push(("compiler".into(), 1.0));
            }
            if t.contains("invoice") {
                v.push(("finance".into(), 1.0));
            }
            v
        };
        let items = vec![
            ObservedItem {
                at: days_ago(0),
                kind: ObservationKind::Paste,
                label: "today rust".into(),
                text: "rust release notes".into(),
                metadata: BTreeMap::new(),
            },
            ObservedItem {
                at: days_ago(0),
                kind: ObservationKind::Document,
                label: "inv".into(),
                text: "invoice 2026-08".into(),
                metadata: BTreeMap::new(),
            },
            ObservedItem {
                at: days_ago(120),
                kind: ObservationKind::Paste,
                label: "old rust".into(),
                text: "rust ancient blog".into(),
                metadata: BTreeMap::new(),
            },
        ];
        update_profile(&mut profile, &items, &scorer);

        assert_eq!(profile.total_observations, 3);
        assert_eq!(profile.entries.len(), 2);
        // Fresh rust + fresh invoice outweigh 120-day-old rust → compiler ≈ finance > 0.5 share.
        let compiler = profile
            .entries
            .iter()
            .find(|e| e.domain == "compiler")
            .unwrap();
        assert!(compiler.samples >= 2);
        let sum: f64 = profile.entries.iter().map(|e| e.score).sum();
        assert!((sum - 1.0).abs() < 1e-9, "normalized, got {sum}");

        // All-fresh world with rust observed twice vs finance once: ranking is
        // strict, and the old third observation would have flipped it.
        let mut fresh = InterestProfile::default();
        let mut recent = items[..2].to_vec();
        recent.push(ObservedItem {
            at: days_ago(1),
            kind: ObservationKind::Paste,
            label: "more rust".into(),
            text: "rust cargo tips".into(),
            metadata: BTreeMap::new(),
        });
        update_profile(&mut fresh, &recent, &scorer);
        assert_eq!(fresh.entries[0].domain, "compiler");
        assert!(fresh.entries[0].score > fresh.entries[1].score);
    }

    #[test]
    fn unclassifiable_items_are_counted_but_not_scored() {
        let mut profile = InterestProfile::default();
        let scorer = |_: &str| Vec::new();
        let items = vec![ObservedItem {
            at: Utc::now(),
            kind: ObservationKind::Paste,
            label: "noise".into(),
            text: "asdf jkl".into(),
            metadata: BTreeMap::new(),
        }];
        update_profile(&mut profile, &items, &scorer);
        assert_eq!(profile.total_observations, 1);
        assert!(profile.entries.is_empty());
    }

    #[test]
    fn events_round_trip_and_cap() {
        let db = sled::Config::new().temporary(true).open().unwrap();
        let cap = ClipboardCapture::capture(Utc::now(), "hello world");
        let item = ObservedItem::from_clipboard(&cap);

        append_items(&db, "alice", vec![item.clone()]).unwrap();
        append_items(&db, "bob", vec![item.clone()]).unwrap();
        assert_eq!(load_items(&db, "alice").len(), 1, "tenant isolation");
        assert_eq!(load_items(&db, "bob").len(), 1);

        let flood: Vec<ObservedItem> = (0..EVENTS_CAP).map(|_| item.clone()).collect();
        append_items(&db, "alice", flood).unwrap();
        assert_eq!(load_items(&db, "alice").len(), EVENTS_CAP);
    }
}
