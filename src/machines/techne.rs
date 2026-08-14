//! techne — the codebase machine.
//!
//! Perceive a scanned repo (file paths + structural lines); reduce to
//! **capability** primitives (what a file is *for*, not where it lives) and a
//! module **dependency** graph; elaborate by tracing impact ("what breaks if
//! this changes"). Pairs with `/api/v1/scan` (which already yields the file +
//! structural-line nodes) — techne adds meaning + relations on top.
//!
//! Pure + testable: the reducers take plain data; `register_*` is the only core
//! touch. A capability is the elaborable unit (a single file isn't — it's a leaf).
//!
//! Standalone twin: `physis-nous` (Python). It overlaps techne's code domain but
//! adds two distinct primitives not yet ported here — **design-pattern**
//! recognition (control-flow / signature shapes) and **refactoring** as the
//! elaboration step (extract-method, introduce-parameter, replace-conditional).
//! Fold those in here rather than adding a separate `nous` module.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use crate::core::PhysisCore;
use crate::embed::VectorEmbed;

/// A scanned source file: its path and its structural lines (signatures, imports).
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: String,
    pub lines: Vec<String>,
}

/// A capability: a cluster of files that serve one purpose.
#[derive(Debug, Clone, PartialEq)]
pub struct Capability {
    pub name: String,
    pub files: Vec<String>,
}

/// Ordered keyword → capability rules. First match on the lowercased path wins.
const CAP_RULES: &[(&str, &str)] = &[
    ("test", "testing"),
    ("auth", "authentication"),
    ("/embed", "embedding"),
    ("classif", "classification"),
    ("graph", "graph"),
    ("dream", "dreaming"),
    ("scan", "ingestion"),
    ("spanner", "persistence"),
    ("server", "http-api"),
    ("route", "http-api"),
    ("ontolog", "ontology"),
    ("machine", "machines"),
    ("config", "config"),
];

fn capability_of(path: &str) -> &'static str {
    let p = path.to_lowercase();
    for (kw, cap) in CAP_RULES {
        if p.contains(kw) {
            return cap;
        }
    }
    "misc"
}

/// The module name a path defines. `src/embed/clip.rs` → `clip`. A module-root
/// file (`mod.rs` / `__init__.py` / `index.ts`) names its **parent directory**
/// (`src/embed/mod.rs` → `embed`), matching how the language resolves it.
fn module_of(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let file = parts.last().copied().unwrap_or(path);
    let stem = file
        .trim_end_matches(".rs")
        .trim_end_matches(".py")
        .trim_end_matches(".ts");
    if matches!(stem, "mod" | "__init__" | "index") && parts.len() >= 2 {
        return parts[parts.len() - 2].to_string();
    }
    stem.to_string()
}

/// Cluster files into capabilities (sorted, stable).
pub fn classify_capabilities(files: &[SourceFile]) -> Vec<Capability> {
    let mut buckets: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for f in files {
        buckets.entry(capability_of(&f.path)).or_default().push(f.path.clone());
    }
    buckets
        .into_iter()
        .map(|(name, mut files)| {
            files.sort();
            Capability { name: name.to_string(), files }
        })
        .collect()
}

/// Module-level dependency edges `(from_module, to_module)` parsed from import
/// lines (`use crate::X`, `import X`, `from X import`). Only edges whose target
/// is a known module (some file's stem) are kept — external deps are dropped.
pub fn dependency_edges(files: &[SourceFile]) -> Vec<(String, String)> {
    let known: HashSet<String> = files.iter().map(|f| module_of(&f.path)).collect();
    let mut edges = Vec::new();
    let mut seen = HashSet::new();
    for f in files {
        let from = module_of(&f.path);
        for line in &f.lines {
            for target in referenced_modules(line) {
                if target != from && known.contains(&target) {
                    let key = (from.clone(), target.clone());
                    if seen.insert(key.clone()) {
                        edges.push(key);
                    }
                }
            }
        }
    }
    edges
}

/// Pull candidate module names out of an import-ish line.
fn referenced_modules(line: &str) -> Vec<String> {
    let l = line.trim();
    let mut out = Vec::new();
    if let Some(rest) = l.strip_prefix("use crate::") {
        // use crate::a::b::C  → first segment `a`, and the last type-ish `b`
        for seg in rest.trim_end_matches(';').split("::") {
            let s = seg.trim().trim_matches(|c| c == '{' || c == '}' || c == ' ');
            if !s.is_empty() && s.chars().next().unwrap().is_lowercase() {
                out.push(s.to_string());
            }
        }
    } else if let Some(rest) = l.strip_prefix("from ") {
        if let Some(m) = rest.split_whitespace().next() {
            out.push(m.trim_start_matches('.').split('.').next().unwrap_or(m).to_string());
        }
    } else if let Some(rest) = l.strip_prefix("import ") {
        if let Some(m) = rest.split_whitespace().next() {
            out.push(m.split('.').next().unwrap_or(m).to_string());
        }
    }
    out
}

/// Transitive dependents of `target` — "what breaks if this changes". BFS over
/// reversed dependency edges.
pub fn impact_of(edges: &[(String, String)], target: &str) -> Vec<String> {
    let mut reverse: HashMap<&str, Vec<&str>> = HashMap::new();
    for (from, to) in edges {
        reverse.entry(to.as_str()).or_default().push(from.as_str());
    }
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut queue: VecDeque<&str> = VecDeque::new();
    queue.push_back(target);
    seen.insert(target.to_string());
    while let Some(m) = queue.pop_front() {
        if let Some(deps) = reverse.get(m) {
            for &d in deps {
                if seen.insert(d.to_string()) {
                    out.push(d.to_string());
                    queue.push_back(d);
                }
            }
        }
    }
    out.sort();
    out
}

/// Adapter: register each capability as a labeled, bridged coherence node, then
/// add dependency edges between the module nodes. Returns capability node ids.
pub fn register_capabilities(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    files: &[SourceFile],
) -> Vec<String> {
    let caps = classify_capabilities(files);
    let mut ids = Vec::new();
    for c in &caps {
        let text = format!("capability: {} ({} files)", c.name, c.files.len());
        let id = core.register_node_from_text(&text, embedder);
        core.bridge_node(&id);
        ids.push(id);
    }
    // Dependency edges between module nodes (registered on demand).
    for (from, to) in dependency_edges(files) {
        let a = core.register_node_from_text(&format!("module: {from}"), embedder);
        let b = core.register_node_from_text(&format!("module: {to}"), embedder);
        core.add_edge_between(&a, &b, 1.0, 1);
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(path: &str, lines: &[&str]) -> SourceFile {
        SourceFile { path: path.to_string(), lines: lines.iter().map(|s| s.to_string()).collect() }
    }

    #[test]
    fn classifies_files_into_capabilities() {
        let files = vec![
            f("src/auth/login.rs", &[]),
            f("src/embed/clip.rs", &[]),
            f("src/embed/jina.rs", &[]),
            f("src/random_widget.rs", &[]),
        ];
        let caps = classify_capabilities(&files);
        let names: Vec<&str> = caps.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"authentication"));
        assert!(names.contains(&"embedding"));
        assert!(names.contains(&"misc"));
        let embedding = caps.iter().find(|c| c.name == "embedding").unwrap();
        assert_eq!(embedding.files.len(), 2);
    }

    #[test]
    fn module_root_files_name_their_parent_dir() {
        assert_eq!(module_of("src/embed/mod.rs"), "embed");
        assert_eq!(module_of("src/embed/clip.rs"), "clip");
        assert_eq!(module_of("pkg/embed/__init__.py"), "embed");
        assert_eq!(module_of("src/core.rs"), "core");
    }

    #[test]
    fn builds_dependency_edges_and_impact() {
        let files = vec![
            f("src/core.rs", &["use crate::trie::Trie;", "use crate::embed::X;"]),
            f("src/trie.rs", &[]),
            f("src/embed.rs", &["use crate::trie::Trie;"]),
            f("src/api.rs", &["use crate::core::PhysisCore;"]),
        ];
        let edges = dependency_edges(&files);
        // core→trie, core→embed, embed→trie, api→core
        assert!(edges.contains(&("core".into(), "trie".into())));
        assert!(edges.contains(&("embed".into(), "trie".into())));
        assert!(edges.contains(&("api".into(), "core".into())));
        // What breaks if trie changes? core, embed, and (transitively) api.
        let impact = impact_of(&edges, "trie");
        assert_eq!(impact, vec!["api".to_string(), "core".to_string(), "embed".to_string()]);
    }
}
