/// Dependents + intent + map rendering for the MVP loop.
use std::collections::BTreeSet;
use std::path::Path;

use crate::nav::{NavHit, NavWeights};

/// Recent changes: git log when available, else mtime order over the scan.
/// Read-only projection — no session recording. Shared by the REPL `history`
/// command and the HUD `H` view so both surfaces report the same past.
pub fn history_text(root: &Path) -> String {
    let out = std::process::Command::new("git")
        .args(["-C", &root.to_string_lossy(), "log", "--oneline", "-15", "--", "."])
        .output();
    let mut o = String::new();
    match out {
        Ok(res) if res.status.success() => {
            o.push_str(&String::from_utf8_lossy(&res.stdout));
        }
        _ => {
            o.push_str("(no git history here; filesystem snapshot is the record)\n");
            let files = crate::scanner::scan_project(root, None);
            let mut by_mtime = files.clone();
            by_mtime.sort_by(|a, b| {
                b.mtime
                    .partial_cmp(&a.mtime)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for f in by_mtime.iter().take(15) {
                o.push_str(&format!("  {}  {}\n", f.mtime as u64, f.path));
            }
        }
    }
    o
}
/// Dependents of `changed`: files that *reference* the changed symbols or module.
///
/// The rule is deliberately precision-first, and it was measured before it was
/// believed: a first version asked only "does this file contain the symbol
/// name", and on `physis-core` an append to one file produced **261** dependents
/// because names like `run`, `Report` and `Source` occur in prose and in every
/// benchmark result. A consequence panel that lists 261 things shows nothing.
///
/// So a mention counts only when it is *code-shaped* — a call, a path, an
/// import — and only for identifiers long enough to be an identifier rather
/// than a word. Prose files still qualify when they name the module
/// (`act-recall`, `act_recall`), because that is a real dependency of a doc on a
/// program.
pub fn affected_dependents(root: &Path, changed: &[String], symbols: &[String]) -> Vec<String> {
    if changed.is_empty() {
        return Vec::new();
    }
    let changed_set: BTreeSet<&str> = changed.iter().map(String::as_str).collect();
    // Module stems, with `-`/`_` folded: a doc saying `act-recall` names the
    // module `act_recall`.
    let stems: Vec<String> = changed
        .iter()
        .filter_map(|p| {
            Path::new(p)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.replace('-', "_").to_lowercase())
        })
        .collect();
    let code_syms: Vec<&String> = symbols.iter().filter(|s| s.len() >= 6).collect();

    let mut out: BTreeSet<String> = BTreeSet::new();
    for f in &crate::scanner::scan_project(root, None) {
        if changed_set.contains(f.path.as_str()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(&f.path)) else {
            continue;
        };

        let stem_hit = stems.iter().any(|s| {
            let lower = text.to_lowercase().replace('-', "_");
            lower.contains(s.as_str())
        });
        let sym_hit = code_syms.iter().any(|s| {
            // Two strengths of reference, and the weaker one is gated by how
            // distinctive the name is:
            //  * a path/import reference (`Type::sym`, `sym::`, `use … sym`)
            //    names the symbol exactly — always counted;
            //  * a bare call (`sym(`) is a coin flip for a plain lowercase
            //    word: `render`, `dimension` and `command` are every struct's
            //    method. Measured on `physis-core`, ungated bare calls alone
            //    produced most of the 98 false dependents. So a bare call only
            //    counts when the name could not be anything else —
            //    snake_case with an underscore, or CamelCase.
            let distinctive = s.contains('_') || s.chars().any(|c| c.is_ascii_uppercase());
            let mut pats = vec![format!("{s}::"), format!("::{s}"), format!("use {s}")];
            if distinctive {
                pats.push(format!("{s}("));
            }
            pats.iter().any(|p| text.contains(p.as_str()))
        });
        if stem_hit || sym_hit {
            out.insert(f.path.clone());
        }
    }
    out.into_iter().collect()
}

/// Symbol names **defined** in the changed files.
///
/// Only definitions, and that distinction was measured: `artifacts_of` also
/// records `impl Foo` shapes, so a first version reported `VectorEmbed` (a type
/// the changed file merely implements) and every file referencing that type —
/// 79 of them in `physis-core` — appeared as a dependent. A type a file
/// implements is a reference *out of* it, never a consequence *of* it.
pub fn changed_symbols(root: &Path, changed: &[String]) -> Vec<String> {
    let mut syms = Vec::new();
    for rel in changed {
        let Ok(text) = std::fs::read_to_string(root.join(rel)) else {
            continue;
        };
        for stub in super::model::artifacts_of(rel, &text) {
            if stub.structural.starts_with("document:") {
                continue;
            }
            // `impl X` names someone else's type; skip it.
            if stub.structural.starts_with("impl ") {
                continue;
            }
            if let Some(name) = stub.structural.split_whitespace().nth(1) {
                syms.push(name.to_string());
            }
        }
    }
    syms.sort();
    syms.dedup();
    syms
}

/// Cheap intent: keyword rules over the raw line, no architecture.
#[derive(Debug, Clone)]
pub struct Intent {
    pub raw: String,
    pub query: String,
    pub wants_action: bool,
    pub wants_plan: bool,
}

pub fn classify_intent(line: &str) -> Intent {
    let lower = line.to_lowercase();
    let wants_action = [
        "improve", "fix", "edit", "change", "refactor", "add", "remove", "update",
    ]
    .iter()
    .any(|w| lower.contains(w));
    let wants_plan = lower.contains("plan") || lower.contains("propos");
    let mut query = line.to_string();
    for verb in [
        "show dependencies of",
        "what changed recently",
        "what contradicts this",
        "what does this project do",
        "without changing its public api",
        "without changing",
        "where is",
        "where does",
        "inspect this",
        "inspect",
        "improve",
        "show",
        "what",
        "where",
    ] {
        query = query.replace(verb, " ");
    }
    let query = query.split_whitespace().collect::<Vec<_>>().join(" ");
    let query = if query.trim().is_empty() {
        line.to_string()
    } else {
        query
    };
    Intent {
        raw: line.to_string(),
        query,
        wants_action,
        wants_plan,
    }
}

/// Minimal tree/graph hybrid for the navigable map.
pub fn render_map(hits: &[NavHit], trail: &[String]) -> String {
    let mut o = String::new();
    o.push_str("PROJECT\n");
    let mut tree: std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, Vec<&NavHit>>,
    > = Default::default();
    for h in hits.iter().take(12) {
        let path = h
            .object
            .source
            .as_ref()
            .map(|s| s.path.clone())
            .unwrap_or_else(|| h.object.label.clone());
        let dir = Path::new(&path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".into());
        let file = Path::new(&path)
            .file_name()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(path.clone());
        tree.entry(dir)
            .or_default()
            .entry(file)
            .or_default()
            .push(h);
    }
    for (dir, files) in &tree {
        o.push_str(&format!(" └─ {dir}\n"));
        for (file, hs) in files {
            o.push_str(&format!("     └─ {file}\n"));
            for h in hs.iter().take(3) {
                o.push_str(&format!(
                    "         └─ {}  ↔ {:.2}  why: {}\n",
                    h.object.label,
                    h.score.total,
                    crate::nav::explain_hit(h, &NavWeights::default())
                ));
            }
        }
    }
    if !trail.is_empty() {
        o.push_str(&format!("\ntrail: {}\n", trail.join(" › ")));
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intent_keeps_the_topic_and_drops_the_verb() {
        let i = classify_intent("where is retrieval implemented?");
        assert!(!i.query.to_lowercase().contains("where is"));
        assert!(i.query.contains("retrieval"));
        assert!(!i.wants_action);
    }

    #[test]
    fn actionable_requests_are_recognised() {
        let i = classify_intent("improve retrieval without changing its public API");
        assert!(i.wants_action);
        assert!(i.query.contains("retrieval"));
        assert!(!i.query.contains("without changing"));
        assert!(classify_intent("draft a plan for caching").wants_plan);
    }

    #[test]
    fn a_pure_verb_still_yields_a_nonempty_query() {
        let i = classify_intent("show");
        assert!(!i.query.trim().is_empty(), "retrieval needs a query");
    }

    /// Dependents come from real file text, not from a guess: a file that
    /// *calls or imports* the changed symbol is reported, a merely prose-y or
    /// unrelated one is not.
    #[test]
    fn dependents_are_found_from_code_shaped_references() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src/rag.rs"),
            "pub fn compile_context() {}\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("src/notebook.rs"),
            "use crate::rag;\nfn use_it() { compile_context(); }\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("src/unrelated.rs"), "fn other() {}\n").unwrap();

        let syms = changed_symbols(dir.path(), &["src/rag.rs".to_string()]);
        assert!(syms.contains(&"compile_context".to_string()));
        let deps = affected_dependents(dir.path(), &["src/rag.rs".to_string()], &syms);
        assert!(
            deps.contains(&"src/notebook.rs".to_string()),
            "got {deps:?}"
        );
        assert!(!deps.contains(&"src/unrelated.rs".to_string()));
    }

    /// The precision rule, measured on `physis-core` where the loose version
    /// returned 261 dependents: a common short identifier mentioned in prose is
    /// not a dependency, and a bare prose mention of a long identifier is not
    /// one either.
    #[test]
    fn common_identifiers_and_prose_mentions_are_not_dependents() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src/target.rs"),
            "pub fn run() {}\npub fn long_helper_name() {}\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("docs.md"),
            "The report describes how to run the thing. long_helper_name is nice.\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("src/caller.rs"),
            "fn go() { long_helper_name(); }\n",
        )
        .unwrap();
        let syms = changed_symbols(dir.path(), &["src/target.rs".to_string()]);
        assert!(
            syms.contains(&"run".to_string()),
            "shapes are still extracted"
        );
        let deps = affected_dependents(dir.path(), &["src/target.rs".to_string()], &syms);
        assert!(deps.contains(&"src/caller.rs".to_string()), "got {deps:?}");
        assert!(
            !deps.contains(&"docs.md".to_string()),
            "prose is not a call: {deps:?}"
        );
    }

    #[test]
    fn no_change_means_no_dependents_scanned() {
        let dir = tempfile::tempdir().unwrap();
        assert!(affected_dependents(dir.path(), &[], &[]).is_empty());
    }
}
