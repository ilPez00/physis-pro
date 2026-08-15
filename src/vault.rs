//! Vault importers (Phase 7B) — Obsidian/markdown notes, plain text, and `git log`.
//!
//! Each source is reduced to [`VaultDoc`]s; callers register them as labeled core
//! nodes with the **shared** embedder so a knowledge vault is recallable by
//! content in the wiki — the same "source → embed → register labeled node"
//! connector shape as scan-labelling and watch-mode.
//!
//! CLI: `physis-pro vault <dir>` — web: `POST /api/v1/vault/import`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::scanner::FileInfo;

/// A single retrievable unit pulled out of a vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultDoc {
    /// Where the doc came from (file path, or `git <short-hash>` for commits).
    pub source: String,
    /// Human title: first H1, Obsidian `title:` frontmatter, or the file stem.
    pub title: String,
    /// Full body text (the recallable content).
    pub body: String,
    /// Markdown headings (`h1:Title`, `h2:Sub`, …) extracted from the body.
    pub headings: Vec<String>,
}

impl VaultDoc {
    /// Browsable content labels: the title plus one per heading
    /// (`title → h1:…`). Callers register these as labeled core nodes, matching
    /// how `FileInfo::labels()` feeds scan/watch.
    pub fn labels(&self) -> Vec<String> {
        let mut labels: Vec<String> = self
            .headings
            .iter()
            .map(|h| format!("{} → {}", self.title, h))
            .collect();
        labels.push(self.title.clone());
        labels
    }
}

/// First non-empty H1, Obsidian `title:` frontmatter, or the file stem.
fn title_of(path: &Path, body: &str) -> String {
    // Obsidian frontmatter: a leading `---\n` block may carry `title: "X"`.
    let stripped = body.trim_start();
    if let Some(rest) = stripped.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---") {
            let front = &rest[..end];
            for line in front.lines() {
                let line = line.trim();
                if let Some(t) = line.strip_prefix("title:") {
                    let t = t.trim().trim_matches('"').trim_matches('\'').to_string();
                    if !t.is_empty() {
                        return t;
                    }
                }
            }
        }
    }
    // First H1.
    for line in body.lines() {
        let line = line.trim();
        if let Some(t) = line.strip_prefix("# ") {
            let t = t.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    // Fallback: file stem.
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "untitled".to_string())
}

/// Extract `h1:`…`h3:` headings (same scheme the scanner uses for markdown).
fn headings_of(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        let hash_len = trimmed.chars().take_while(|c| *c == '#').count();
        if hash_len == 0 {
            continue;
        }
        let rest = trimmed[hash_len..].trim();
        if rest.is_empty() {
            continue;
        }
        let level = hash_len.min(3);
        out.push(format!("h{level}:{rest}"));
        if out.len() >= 20 {
            break;
        }
    }
    out
}

/// Scan a directory (Obsidian vault or plain notes) for markdown/text docs.
/// Reuses the scanner's file walk + text detection so both agree on what counts.
pub fn scan_vault(dir: &Path) -> Vec<VaultDoc> {
    let mut files = crate::scanner::scan_project(dir, None);
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let mut docs = Vec::new();
    for f in files {
        // Scanner already embeds images-as-text; vault import is text notes only.
        if !f.ext.eq_ignore_ascii_case(".md")
            && !f.ext.eq_ignore_ascii_case(".mdx")
            && !f.ext.eq_ignore_ascii_case(".txt")
            && !f.ext.eq_ignore_ascii_case(".rst")
            && !f.ext.eq_ignore_ascii_case(".adoc")
            && !f.ext.eq_ignore_ascii_case(".asciidoc")
            && !f.ext.eq_ignore_ascii_case(".org")
        {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(&f.abs_path) else { continue };
        let title = title_of(&PathBuf::from(&f.path), &body);
        let headings = headings_of(&body);
        let doc = VaultDoc {
            source: f.path.clone(),
            title,
            body,
            headings,
        };
        // Skip near-empty notes (e.g. a bare scaffold) — nothing to recall.
        if doc.body.trim().len() < 8 {
            continue;
        }
        docs.push(doc);
    }
    docs
}

/// Parse `git log --format=%H%x1f%s%x1f%b%x1e` output into commit docs.
/// Exposed separately so tests can feed canned output without a git binary.
pub fn parse_git_log(output: &str) -> Vec<VaultDoc> {
    let mut docs = Vec::new();
    for record in output.split('\x1e') {
        let record = record.trim();
        if record.is_empty() {
            continue;
        }
        let mut fields = record.splitn(3, '\x1f');
        let hash = fields.next().unwrap_or_default().trim();
        let subject = fields.next().unwrap_or_default().trim();
        let body = fields.next().unwrap_or_default().trim();
        if hash.is_empty() && subject.is_empty() {
            continue;
        }
        let short = hash.get(..7).unwrap_or(hash);
        let mut full = subject.to_string();
        if !body.is_empty() {
            full.push_str("\n\n");
            full.push_str(body);
        }
        docs.push(VaultDoc {
            source: format!("git {short}"),
            title: if subject.is_empty() { short.to_string() } else { subject.to_string() },
            body: full,
            headings: vec![],
        });
    }
    docs
}

/// Run `git log` in `repo` and reduce the most recent `max` commits to docs.
/// Returns an empty vec when git is missing or the dir is not a repository.
pub fn scan_git_log(repo: &Path, max: usize) -> Vec<VaultDoc> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["log", "--format=%H%x1f%s%x1f%b%x1e", &format!("-n{max}")])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            parse_git_log(&text)
        }
        _ => vec![],
    }
}

/// Reduce a whole vault to labeled core nodes, returning them (label + embeddable
/// text) so a caller with the shared embedder can register them. Kept pure so CLI
/// and web share the exact same ingestion contract.
pub fn collect_labels(docs: &[VaultDoc]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for d in docs {
        for label in d.labels() {
            out.push((label, d.body.clone()));
        }
    }
    out
}

// ── Reuse the scanner's FileInfo path for CLI output parity ────────────────

impl VaultDoc {
    /// Project this doc onto the scanner's label scheme (`path → heading`,
    /// plus the structural path), so a vault import and a watch/scan of the same
    /// dir produce overlapping, deduped nodes instead of divergent ones.
    pub fn as_file_info(&self) -> FileInfo {
        FileInfo {
            path: self.source.clone(),
            abs_path: self.source.clone(),
            ext: ".md".to_string(),
            size: self.body.len() as u64,
            mtime: 0.0,
            hash: String::new(),
            structural_lines: self.headings.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(body: &str) -> VaultDoc {
        VaultDoc {
            source: "notes/a.md".to_string(),
            title: title_of(Path::new("notes/a.md"), body),
            body: body.to_string(),
            headings: headings_of(body),
        }
    }

    #[test]
    fn test_title_from_h1_and_frontmatter() {
        assert_eq!(md("# My Note\n\nbody").title, "My Note");
        assert_eq!(
            md("---\ntitle: \"Vendor Report\"\n---\n# Ignored\n\nbody").title,
            "Vendor Report"
        );
        // No H1 / frontmatter → file stem fallback happens in scan_vault, but
        // title_of falls back to stem too when given a path.
        assert_eq!(title_of(Path::new("notes/a.md"), "just some body"), "a");
    }

    #[test]
    fn test_headings_extracted() {
        let d = md("# Top\n\n## Sub A\n\n### Sub B\n\nplain text\n");
        assert_eq!(d.headings, vec!["h1:Top", "h2:Sub A", "h3:Sub B"]);
        // Non-heading lines are ignored.
        assert_eq!(md("body only\nmore\n").headings.len(), 0);
    }

    #[test]
    fn test_labels_cover_title_and_headings() {
        let d = md("# Release Notes\n\n## Fixes\n\nstuff");
        let labels = d.labels();
        assert!(labels.contains(&"Release Notes".to_string()));
        assert!(labels.contains(&"Release Notes → h2:Fixes".to_string()));
    }

    #[test]
    fn test_parse_git_log() {
        let canned = "abc1234\x1fAdd autoclave cycle log\x1fLonger body here\x1e\
                      def5678\x1fFix bearing offset\x1f\x1e";
        let docs = parse_git_log(canned);
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].source, "git abc1234");
        assert_eq!(docs[0].title, "Add autoclave cycle log");
        assert!(docs[0].body.contains("Longer body here"));
        assert_eq!(docs[1].title, "Fix bearing offset");
        // Empty subjects fall back to the short hash.
        let lone = parse_git_log("feed0000\x1f\x1f\x1e");
        assert_eq!(lone[0].title, "feed000");
    }

    #[test]
    fn test_scan_vault_reads_notes_and_skips_binaries() {
        let dir = std::env::temp_dir().join(format!("physis_vault_test_{}", std::process::id()));
        let notes = dir.join("notes");
        std::fs::create_dir_all(&notes).unwrap();
        std::fs::write(notes.join("one.md"), "# First Note\n\nbody here").unwrap();
        std::fs::write(notes.join("two.txt"), "plain title\nsome content").unwrap();
        std::fs::write(notes.join("binary.bin"), [0u8, 159, 146, 150]).unwrap();

        let docs = scan_vault(&dir);
        let titles: Vec<&str> = docs.iter().map(|d| d.title.as_str()).collect();
        assert!(titles.contains(&"First Note"));
        assert!(titles.contains(&"two"), "txt file picked up with stem title");
        assert_eq!(docs.len(), 2, "binary is not a note");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_collect_labels_pairs_label_with_body() {
        let docs = vec![md("# Alpha\n\ncontent")];
        let pairs = collect_labels(&docs);
        // "# Alpha" is both the title AND an h1 heading → two labels.
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].0, "Alpha → h1:Alpha");
        assert_eq!(pairs[0].1, "# Alpha\n\ncontent");
        assert_eq!(pairs[1].0, "Alpha");
    }
}
