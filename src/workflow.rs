//! workflow — the workflow scanner. The audit layer that answers "who did what
//! where" over a directory tree, focused on *documentation*: which docs exist
//! for AI agents vs humans, when they were last touched, and who touched them.
//!
//! It differs from the base scanner in three ways:
//!
//! 1. **It walks the harness directories** (`.claude`, `.opencode`, `.codex`,
//!    `.hermes`, …) that `scanner::DEFAULT_EXCLUDE_DIRS` deliberately skips —
//!    those are exactly the agent documentation this audit exists to find.
//! 2. Every file is classified by **audience** (`Agent` / `Human` / `Both` /
//!    `Other`) from its path and name, so a repo's doc coverage for each
//!    reader can be checked ("do we have an AGENTS.md AND a README?").
//! 3. Where a git repo is present, each doc is attributed to its **last
//!    author and timestamp** (`git log --name-only`), so the report is a
//!    who-did-what-where table, not just a file list.
//!
//! Output projects onto the shared `OperationalEvent` stream (kind `Workflow`,
//! connector `workflow`), so a workflow scan and a shop-floor reading land in
//! the same timeline/ledger without mixing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::operational::{
    OperationalEvent, OperationalKind, OperationalProvenance, OperationalStatus,
};
use crate::study::Tally;

/// Who a documentation file is written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocAudience {
    /// Written for an AI agent (harness config, AGENTS.md, skills, …).
    Agent,
    /// Written for a human reader (README, docs/, CHANGELOG, …).
    Human,
    /// Read by both (AGENTS.md doubles as onboarding, llms.txt).
    Both,
    /// Not documentation (source, data, binary, …).
    Other,
}

impl DocAudience {
    pub fn as_str(self) -> &'static str {
        match self {
            DocAudience::Agent => "agent",
            DocAudience::Human => "human",
            DocAudience::Both => "both",
            DocAudience::Other => "other",
        }
    }
}

/// Directories the workflow scanner *includes* that the base scanner excludes:
/// they hold AI-agent documentation. `scan_workflow` walks these; the base
/// `scan_project` (ontology mapping) still skips them.
pub const HARNESS_DIRS: &[&str] = &[
    ".claude",
    ".opencode",
    ".codex",
    ".commandcode",
    ".autoresearch",
    ".cursor",
    ".agents",
    ".hermes",
    ".gemini",
    ".mcp",
];

/// Directories the workflow scanner still skips (noise, vendored trees, VCS).
/// Note: deliberately does NOT include the harness dirs above.
pub const WORKFLOW_EXCLUDE_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "__pycache__",
    "node_modules",
    ".venv",
    "venv",
    ".tox",
    ".eggs",
    "eggs",
    "build",
    "dist",
    "target",
    ".gradle",
    "out",
    ".index",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".terraform",
    ".serverless",
    "aura_venv",
    // Heavy harness-state trees: not documentation, and walking them makes a
    // home/ or drive-root audit unbounded (`.hermes/node` is a Node install).
    ".cache",
    ".npm",
    ".cargo",
    ".rustup",
    ".local",
    "cache",
    "logs",
    "node",
    "sandboxes",
    "file-history",
    "sessions",
    "shell-snapshots",
    "paste-cache",
    "downloads",
    "images",
    "audio_cache",
    "image_cache",
    "plugins",
    "session-env",
    "state",
    "terminal-sessions",
    "migration",
    "platforms",
];

/// Doc-file extensions considered documentation for humans.
pub const HUMAN_DOC_EXTS: &[&str] = &[
    "md", "mdx", "txt", "rst", "adoc", "asciidoc", "org", "html", "htm", "mdc",
];

/// Names (lowercased) that mark a file as AI-agent documentation regardless of
/// the directory it lives in.
pub const AGENT_DOC_NAMES: &[&str] = &[
    "agents.md",
    "ag.txt",
    "claude.md",
    "opencode.json",
    "opencode.jsonc",
    ".mcp.json",
    "soul.md",
    "soğul.md",
    "settings.local.json",
    "settings.json",
    ".cursorrules",
    "llms.txt",
];

/// Names (lowercased) that mark a file as human documentation regardless of
/// the directory it lives in.
pub const HUMAN_DOC_NAMES: &[&str] = &[
    "readme",
    "readme.md",
    "readme.txt",
    "changelog",
    "changelog.md",
    "contributing",
    "contributing.md",
    "license",
    "license.txt",
    "license.md",
    "security.md",
    "code_of_conduct.md",
    "governance.md",
];

/// Names that are explicitly documentation for *both* readers.
pub const BOTH_DOC_NAMES: &[&str] = &["llms.txt"];

/// Doc-oriented directory names (human docs live under these).
pub const HUMAN_DOC_DIRS: &[&str] = &["docs", "doc", "documentation", "help", "man"];

/// Which harness (if any) a path lives under. Matches against any path
/// component, so `foo/.claude/settings.json` and `.claude/settings.json` both
/// resolve. Returns the canonical harness name.
pub fn harness_of(path: &Path) -> Option<&'static str> {
    for (dir, name) in [
        (".claude", "claude"),
        (".opencode", "opencode"),
        (".codex", "codex"),
        (".commandcode", "commandcode"),
        (".autoresearch", "autoresearch"),
        (".cursor", "cursor"),
        (".agents", "agents"),
        (".hermes", "hermes"),
        (".gemini", "gemini"),
        (".mcp", "mcp"),
    ] {
        if path.components().any(|c| c.as_os_str() == dir) {
            return Some(name);
        }
    }
    None
}

/// Classify a file's audience from its path. Pure — the report builder and the
/// tests both call this directly.
pub fn classify_audience(rel: &Path) -> DocAudience {
    let name = rel
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let ext = rel
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let harness = harness_of(rel);

    // Agent-side signals: lives under a harness dir or is a known agent doc.
    let is_skills = rel.components().any(|c| c.as_os_str() == "skills");
    let agent = harness.is_some() || is_skills || AGENT_DOC_NAMES.contains(&name.as_str());

    // Human-side signals: known doc names, doc-oriented dirs, or a doc ext
    // outside harness dirs.
    let in_doc_dir = rel
        .parent()
        .map(|p| p.components().any(|c| {
            let s = c.as_os_str().to_string_lossy().to_lowercase();
            HUMAN_DOC_DIRS.contains(&s.as_str())
        }))
        .unwrap_or(false);
    let human = HUMAN_DOC_NAMES.contains(&name.as_str())
        || BOTH_DOC_NAMES.contains(&name.as_str())
        || (in_doc_dir && HUMAN_DOC_EXTS.contains(&ext.as_str()))
        || (HUMAN_DOC_EXTS.contains(&ext.as_str()) && !agent);

    match (agent, human) {
        (true, true) => DocAudience::Both,
        (true, false) => DocAudience::Agent,
        (false, true) => DocAudience::Human,
        (false, false) => DocAudience::Other,
    }
}

/// One documentation file with its full audit context: path, timestamps,
/// audience, harness, and (when available) the git author who last touched it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDoc {
    /// Path relative to the scan root.
    pub path: String,
    /// Absolute path on disk.
    pub abs_path: String,
    pub ext: String,
    pub size: u64,
    /// Last modification time (RFC3339).
    pub mtime: DateTime<Utc>,
    /// Creation time when the filesystem exposes it, else mtime.
    pub ctime: DateTime<Utc>,
    /// SHA-256 content hash.
    pub hash: String,
    pub audience: DocAudience,
    /// Harness this file belongs to, e.g. "claude", or None.
    pub harness: Option<String>,
    /// Git author of the most recent commit touching this file.
    pub author: Option<String>,
    /// Git commit time of that most recent commit.
    pub authored_at: Option<DateTime<Utc>>,
    /// Parent directory relative to the scan root ("where").
    pub dir: String,
}

impl WorkflowDoc {
    /// Human-facing label for the shared graph, mirroring `FileInfo::labels`.
    pub fn label(&self) -> String {
        self.path.replace('/', " → ")
    }
}

/// Aggregated workflow report — doc coverage per audience, harness, author and
/// directory, plus the individual docs for drill-down.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowReport {
    /// Canonical scan root.
    pub root: String,
    pub total: usize,
    /// `agent` / `human` / `both` / `other` counts, desc.
    pub by_audience: Vec<Tally>,
    /// Per-harness doc counts (claude, opencode, …), desc.
    pub by_harness: Vec<Tally>,
    /// Per-author doc counts, desc.
    pub by_author: Vec<Tally>,
    /// Per-directory doc counts, desc.
    pub by_dir: Vec<Tally>,
    /// The individual docs (newest mtime first).
    pub docs: Vec<WorkflowDoc>,
}

fn tally<I: IntoIterator<Item = String>>(items: I) -> Vec<Tally> {
    let mut map: HashMap<String, usize> = HashMap::new();
    for k in items {
        *map.entry(k).or_insert(0) += 1;
    }
    let mut v: Vec<Tally> = map
        .into_iter()
        .map(|(key, count)| Tally { key, count })
        .collect();
    v.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
    v
}

/// Build the report from scanned docs.
pub fn report(root: &Path, docs: Vec<WorkflowDoc>) -> WorkflowReport {
    let mut sorted = docs;
    sorted.sort_by_key(|d| std::cmp::Reverse(d.mtime));
    let total = sorted.len();
    let root_str = root.display().to_string();
    WorkflowReport {
        root: root_str,
        total,
        by_audience: tally(sorted.iter().map(|d| d.audience.as_str().to_string())),
        by_harness: tally(
            sorted
                .iter()
                .map(|d| d.harness.clone().unwrap_or_else(|| "none".to_string())),
        ),
        by_author: tally(
            sorted
                .iter()
                .map(|d| d.author.clone().unwrap_or_else(|| "unknown".to_string())),
        ),
        by_dir: tally(sorted.iter().map(|d| d.dir.clone())),
        docs: sorted,
    }
}

fn file_hash(path: &Path) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        use std::io::Read;
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

fn fs_times(path: &Path, fallback: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    let to_utc = |t: std::io::Result<std::time::SystemTime>| {
        t.ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| {
                Utc.timestamp_opt(d.as_secs() as i64, d.subsec_nanos())
                    .single()
            })
            .unwrap_or(fallback)
    };
    match std::fs::metadata(path) {
        Ok(meta) => {
            let modified = to_utc(meta.modified());
            let created = to_utc(meta.created().or_else(|_| meta.modified()));
            (created, modified)
        }
        Err(_) => (fallback, fallback),
    }
}

/// Parse `git log --format=%H%x1f%an%x1f%aI%x1e --name-only` output into a map
/// of path → (author, committed_at). Pure — tests feed canned output without a
/// git binary. Each commit prints its meta line (`hash\x1fauthor\x1fdate\x1e`)
/// followed by the files it touched; later (older) commits do not overwrite
/// earlier (newer) ones, so the map holds each path's most recent author.
pub fn parse_git_attribution(output: &str) -> HashMap<String, (String, DateTime<Utc>)> {
    let mut map = HashMap::new();
    let mut author: Option<String> = None;
    let mut at: Option<DateTime<Utc>> = None;
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut fields = line.splitn(3, '\x1f');
        let hash = fields.next().unwrap_or_default().trim();
        let who = fields.next().unwrap_or_default().trim();
        let date = fields.next().unwrap_or_default().trim();
        if !hash.is_empty() && !who.is_empty() && !date.is_empty() {
            // Commit meta line: hash\x1fauthor\x1fdate\x1e — strip the trailing
            // \x1e record terminator git appends via the format string.
            let date = date.split('\x1e').next().unwrap_or_default().trim();
            author = Some(who.to_string());
            at = DateTime::parse_from_rfc3339(date)
                .ok()
                .map(|d| d.with_timezone(&Utc));
        } else if let (Some(au), Some(t)) = (&author, &at) {
            // A file path this commit touched.
            map.entry(line.to_string())
                .or_insert_with(|| (au.clone(), *t));
        }
    }
    map
}

/// Run `git log --name-only` in `root` (best effort) and attribute each doc to
/// its most recent author. One subprocess for the whole tree, not one per file.
pub fn git_attribution(root: &Path) -> HashMap<String, (String, DateTime<Utc>)> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "log",
            "--format=%H%x1f%an%x1f%aI%x1e",
            "--name-only",
            "-n",
            "500",
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            parse_git_attribution(&text)
        }
        _ => HashMap::new(),
    }
}

/// Walk a tree (including harness dirs) into audited docs. `extra_exclude`
/// augments [`WORKFLOW_EXCLUDE_DIRS`]; git attribution is best-effort.
pub fn scan_workflow(root: &Path, extra_exclude: Option<&std::collections::HashSet<String>>) -> Vec<WorkflowDoc> {
    let root = root
        .canonicalize()
        .unwrap_or_else(|_| root.to_path_buf());
    if !root.is_dir() {
        return vec![];
    }

    let exclude: std::collections::HashSet<String> = WORKFLOW_EXCLUDE_DIRS
        .iter()
        .map(|d| d.to_string())
        .chain(extra_exclude.into_iter().flatten().cloned())
        .collect();
    let attribution = git_attribution(&root);
    let now = Utc::now();

    let mut docs: Vec<WorkflowDoc> = walkdir::WalkDir::new(&root)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                !exclude.contains(name.as_ref())
            } else {
                true
            }
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let path = e.path();
            if !crate::scanner::is_text_file(path) {
                return None;
            }
            let rel = path.strip_prefix(&root).unwrap_or(path);
            let rel_str = rel.to_string_lossy().to_string();
            let ext = rel
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or_default()
                .to_lowercase();
            let (ctime, mtime) = fs_times(path, now);
            let (author, authored_at) = attribution.get(&rel_str).cloned().unwrap_or_default();
            let has_author = !author.is_empty();
            let dir = rel
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| ".".to_string());
            Some(WorkflowDoc {
                path: rel_str,
                abs_path: path.to_string_lossy().to_string(),
                ext,
                size: e.metadata().map(|m| m.len()).unwrap_or(0),
                mtime,
                ctime,
                hash: file_hash(path).unwrap_or_default(),
                audience: classify_audience(rel),
                harness: harness_of(rel).map(String::from),
                author: has_author.then_some(author),
                authored_at: has_author.then_some(authored_at),
                dir,
            })
        })
        .collect();

    docs.sort_by(|a, b| a.path.cmp(&b.path));
    docs
}

/// Project scanned docs onto the shared `OperationalEvent` stream. One event
/// per doc, kind `Workflow`, connector `workflow`, so a workflow scan joins the
/// timeline/ledger every other connector feeds without mixing with them.
pub fn to_op_events(docs: &[WorkflowDoc]) -> Vec<OperationalEvent> {
    docs.iter()
        .map(|d| {
            let status = match d.audience {
                DocAudience::Both => OperationalStatus::Success,
                DocAudience::Agent | DocAudience::Human => OperationalStatus::Info,
                DocAudience::Other => OperationalStatus::Inert,
            };
            let mut provenance = OperationalProvenance::new("workflow", "scan");
            provenance.path = Some(PathBuf::from(&d.abs_path));
            provenance.source_id = d.harness.clone();
            let mut ev = OperationalEvent::new(
                OperationalKind::Workflow,
                status,
                d.mtime,
                d.path.clone(),
                provenance,
            );
            ev.actor = d.author.clone().or_else(|| d.harness.clone());
            ev.project = Some(d.dir.clone());
            ev.properties.insert("audience".to_string(), d.audience.as_str().to_string());
            if let Some(h) = &d.harness {
                ev.properties.insert("harness".to_string(), h.clone());
            }
            ev.properties.insert("ext".to_string(), d.ext.clone());
            ev.properties.insert("size".to_string(), d.size.to_string());
            ev.rekey();
            ev
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_detection_matches_any_component() {
        assert_eq!(harness_of(Path::new(".claude/settings.json")), Some("claude"));
        assert_eq!(harness_of(Path::new("a/.opencode/opencode.json")), Some("opencode"));
        assert_eq!(harness_of(Path::new("src/.hermes/config.yaml")), Some("hermes"));
        assert_eq!(harness_of(Path::new("README.md")), None);
        assert_eq!(harness_of(Path::new(".git/config")), None);
    }

    #[test]
    fn audience_agent_harness_doc() {
        assert_eq!(
            classify_audience(Path::new(".claude/settings.json")),
            DocAudience::Agent
        );
        assert_eq!(
            classify_audience(Path::new(".opencode/AGENTS.md")),
            DocAudience::Agent
        );
        assert_eq!(
            classify_audience(Path::new("skills/foo/SKILL.md")),
            DocAudience::Agent
        );
    }

    #[test]
    fn audience_human_docs() {
        assert_eq!(
            classify_audience(Path::new("README.md")),
            DocAudience::Human
        );
        assert_eq!(
            classify_audience(Path::new("docs/architecture.md")),
            DocAudience::Human
        );
        assert_eq!(
            classify_audience(Path::new("CHANGELOG.md")),
            DocAudience::Human
        );
    }

    #[test]
    fn audience_both_and_other() {
        assert_eq!(
            classify_audience(Path::new("llms.txt")),
            DocAudience::Both
        );
        assert_eq!(
            classify_audience(Path::new("src/main.rs")),
            DocAudience::Other
        );
    }

    #[test]
    fn parse_git_attribution_keeps_newest() {
        let canned = concat!(
            "aaaa1111\x1fAlice\x1f2026-08-30T10:00:00Z\x1e\n",
            "src/a.rs\n",
            "README.md\n",
            "bbbb2222\x1fBob\x1f2026-08-29T09:00:00Z\x1e\n",
            "src/a.rs\n",
            "docs/old.md\n",
        );
        let map = parse_git_attribution(canned);
        assert_eq!(map.len(), 3);
        // Newest commit wins for src/a.rs.
        let (author, _) = &map["src/a.rs"];
        assert_eq!(author, "Alice");
        assert_eq!(map["README.md"].0, "Alice");
        assert_eq!(map["docs/old.md"].0, "Bob");
    }

    #[test]
    fn scan_workflow_finds_agent_and_human_docs() {
        let dir = std::env::temp_dir().join(format!("physis_wf_{}", std::process::id()));
        let _ = std::fs::create_dir_all(dir.join(".claude"));
        let _ = std::fs::create_dir_all(dir.join("docs"));
        let _ = std::fs::create_dir_all(dir.join("src"));
        std::fs::write(dir.join("README.md"), "# Proj\n").unwrap();
        std::fs::write(dir.join("AGENTS.md"), "# agent\n").unwrap();
        std::fs::write(dir.join(".claude/settings.local.json"), "{}").unwrap();
        std::fs::write(dir.join("docs/architecture.md"), "# Arch\n").unwrap();
        std::fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();

        let docs = scan_workflow(&dir, None);
        let by_audience: Vec<(String, usize)> = report(&dir, docs.clone())
            .by_audience
            .iter()
            .map(|t| (t.key.clone(), t.count))
            .collect();
        assert!(by_audience.contains(&("agent".to_string(), 2)), "{by_audience:?}");
        assert!(by_audience.contains(&("human".to_string(), 2)), "{by_audience:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn op_events_carry_workflow_provenance() {
        let dir = std::env::temp_dir().join(format!("physis_wf_ev_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("README.md"), "# X\n").unwrap();
        let docs = scan_workflow(&dir, None);
        let events = to_op_events(&docs);
        assert!(!events.is_empty());
        for e in &events {
            assert_eq!(e.kind, OperationalKind::Workflow);
            assert_eq!(e.provenance.connector, "workflow");
            assert!(!e.event_key.is_empty());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}