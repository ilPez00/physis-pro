//! MVP world model: artifact view + scoped slice + loop delta.
//!
//! Artifact is a view over nav PhysisObject, not a new store.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::nav::{ObjectKind, PhysisObject};
/// One addressable thing in the repository world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub path: String,
    pub kind: String,
    pub file_type: String,
    pub span: Option<(usize, usize)>,
    pub parent: Option<String>,
    pub relations: Vec<String>,
    pub semantic_repr: String,
    pub provenance: String,
}

impl From<&PhysisObject> for Artifact {
    fn from(o: &PhysisObject) -> Self {
        let (path, span) = match &o.source {
            Some(s) => (
                s.path.clone(),
                match (s.start_line, s.end_line) {
                    (Some(a), Some(b)) => Some((a, b)),
                    _ => None,
                },
            ),
            None => (o.label.clone(), None),
        };
        let kind = match o.kind {
            ObjectKind::File => "file",
            ObjectKind::Passage => "section",
            ObjectKind::Directory => "directory",
            ObjectKind::Concept => "concept",
            ObjectKind::Note => "document",
            ObjectKind::Other => "chunk",
        }
        .to_string();
        Self {
            id: o.id.short(),
            path: path.clone(),
            kind,
            file_type: Path::new(&path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_string(),
            span,
            parent: Path::new(&path)
                .parent()
                .map(|p| p.to_string_lossy().to_string()),
            relations: Vec::new(),
            semantic_repr: o.text.chars().take(240).collect(),
            provenance: o
                .source
                .as_ref()
                .map(|s| match (s.start_line, s.end_line) {
                    (Some(a), Some(b)) => format!("{}:{a}-{b}", s.path),
                    _ => s.path.clone(),
                })
                .unwrap_or_else(|| o.label.clone()),
        }
    }
}

/// Minimal structural row before nav-object resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactStub {
    pub path: String,
    pub file_type: String,
    pub structural: String,
}

/// Parse structural artifacts from file text. Reuses scanner line shapes:
/// Rust fn/struct/enum/trait/impl/mod, Markdown h1-h3, JSON key:, TOML
/// table:/config:, generic text degrades to document:chunk.
pub fn artifacts_of(rel: &str, contents: &str) -> Vec<ArtifactStub> {
    let ext = Path::new(rel)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    match ext.as_str() {
        ".rs" => {
            for line in contents.lines().map(str::trim) {
                // Real Rust declares most items behind visibility and
                // qualifiers (`pub fn`, `pub(crate) struct`, `async fn`).
                // Strip them before matching, or only private top-level
                // items are ever seen — which is what the first version did.
                let mut rest = line;
                loop {
                    let before = rest;
                    for q in [
                        "pub(crate)",
                        "pub(super)",
                        "pub(self)",
                        "pub",
                        "async",
                        "unsafe",
                        "default",
                        "extern \"C\"",
                    ] {
                        if let Some(r) = rest.strip_prefix(q) {
                            if r.starts_with(' ') || r.starts_with('(') {
                                rest = r.trim_start();
                            }
                        }
                    }
                    if rest == before {
                        break;
                    }
                }
                for (pat, fmt) in [
                    ("fn ", "fn "),
                    ("struct ", "struct "),
                    ("enum ", "enum "),
                    ("trait ", "trait "),
                    ("impl ", "impl "),
                    ("mod ", "mod "),
                ] {
                    if let Some(body) = rest.strip_prefix(pat) {
                        let name: String = body
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect();
                        if !name.is_empty() {
                            lines.push(format!("{fmt}{name}"));
                        }
                        break;
                    }
                }
            }
        }
        ".md" => {
            for line in contents.lines() {
                let t = line.trim_start();
                let level = t.chars().take_while(|c| *c == '#').count();
                if (1..=3).contains(&level) && t[level..].starts_with(' ') {
                    lines.push(format!("h{level}:{}", t[level..].trim()));
                }
            }
        }
        ".json" => {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(contents) {
                if let Some(obj) = v.as_object() {
                    for k in obj.keys().take(30) {
                        lines.push(format!("key:{k}"));
                    }
                }
            }
        }
        ".toml" | ".yaml" | ".yml" | ".ini" | ".cfg" | ".conf" => {
            for line in contents.lines().map(str::trim) {
                if line.starts_with('[') {
                    lines.push(format!("table:{line}"));
                    continue;
                }
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some(eq) = line.find('=') {
                    lines.push(format!("config:{}", line[..eq].trim()));
                }
            }
        }
        _ => {}
    }
    if lines.is_empty() {
        lines.push("document:chunk".to_string());
    }
    lines
        .into_iter()
        .map(|structural| ArtifactStub {
            path: rel.to_string(),
            file_type: ext.trim_start_matches('.').to_string(),
            structural,
        })
        .collect()
}

/// MVP loop delta: pre/post filesystem + index state around one action.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SemanticDelta {
    pub changed_artifacts: Vec<String>,
    pub added_relations: Vec<String>,
    pub removed_relations: Vec<String>,
    pub affected_dependents: Vec<String>,
    pub invariant_results: Vec<String>,
    pub test_results: Vec<String>,
    pub objects_before: usize,
    pub objects_after: usize,
    pub objects_remapped: usize,
    pub changed_semantic_neighborhoods: usize,
    pub full_rescan: bool,
}

impl SemanticDelta {
    pub fn render(&self) -> String {
        let mut o = String::new();
        o.push_str("Δ\n\nCHANGED\n");
        if self.changed_artifacts.is_empty() {
            o.push_str("  (none)\n");
        }
        for c in &self.changed_artifacts {
            o.push_str(&format!("  {c}\n"));
        }
        o.push_str("\nAFFECTED\n");
        if self.affected_dependents.is_empty() {
            o.push_str("  (none)\n");
        }
        // Bounded on purpose: the consequence panel is for reading. The full
        // list is in the transition record, and the count says how many were
        // left out rather than implying this is all of them.
        for a in self.affected_dependents.iter().take(20) {
            o.push_str(&format!("  {a}\n"));
        }
        if self.affected_dependents.len() > 20 {
            o.push_str(&format!(
                "  … and {} more (full list in the transition record)\n",
                self.affected_dependents.len() - 20
            ));
        }
        o.push_str("\nSTRUCTURE\n");
        for r in &self.added_relations {
            o.push_str(&format!("  + relation: {r}\n"));
        }
        for r in &self.removed_relations {
            o.push_str(&format!("  - relation: {r}\n"));
        }
        if self.added_relations.is_empty() && self.removed_relations.is_empty() {
            o.push_str("  (no relation churn)\n");
        }
        o.push_str("\nVALIDATION\n");
        if self.test_results.is_empty() {
            o.push_str("  (no validation ran)\n");
        }
        for t in &self.test_results {
            o.push_str(&format!("  {t}\n"));
        }
        for i in &self.invariant_results {
            o.push_str(&format!("  {i}\n"));
        }
        o.push_str("\nMAP\n");
        o.push_str(&format!(
            "  {} objects remapped ({} → {}{})\n",
            self.objects_remapped,
            self.objects_before,
            self.objects_after,
            if self.full_rescan {
                ", full rescan"
            } else {
                ", incremental"
            }
        ));
        o.push_str(&format!(
            "  {} semantic neighborhood(s) changed\n",
            self.changed_semantic_neighborhoods
        ));
        o
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// File-type invariants: each format keeps its own structural shapes
    /// rather than being flattened into homogeneous text.
    #[test]
    fn rust_structural_shapes_are_preserved() {
        let stubs = artifacts_of(
            "src/rag.rs",
            "pub fn compile_context(\nstruct Foo {}\nimpl Foo {}\n",
        );
        assert!(stubs.iter().any(|s| s.structural == "fn compile_context"));
        assert!(stubs.iter().any(|s| s.structural == "struct Foo"));
        assert!(stubs.iter().any(|s| s.structural == "impl Foo"));
        assert!(stubs.iter().all(|s| s.file_type == "rs"));
    }

    #[test]
    fn markdown_sections_are_headings_not_rust() {
        let stubs = artifacts_of("README.md", "# Title\n\n## Section\n");
        assert!(stubs.iter().any(|s| s.structural == "h1:Title"));
        assert!(stubs.iter().any(|s| s.structural == "h2:Section"));
        assert!(stubs.iter().all(|s| !s.structural.starts_with("fn ")));
    }

    #[test]
    fn json_keys_and_toml_tables_keep_their_hierarchy() {
        let j = artifacts_of("config.json", r#"{"alpha": 1, "beta": {"g": 2}}"#);
        assert!(j.iter().any(|s| s.structural == "key:alpha"));
        assert!(j.iter().any(|s| s.structural == "key:beta"));
        let t = artifacts_of("Cargo.toml", "[package]\nname = \"x\"\n");
        assert!(t.iter().any(|s| s.structural == "table:[package]"));
        assert!(t.iter().any(|s| s.structural == "config:name"));
    }

    #[test]
    fn generic_text_degrades_to_document_chunk() {
        let stubs = artifacts_of("notes.txt", "just some words\n");
        assert_eq!(stubs.len(), 1);
        assert!(stubs[0].structural.starts_with("document:"));
    }

    #[test]
    fn delta_render_carries_every_required_section() {
        let d = SemanticDelta {
            changed_artifacts: vec!["src/rag.rs::compile_context".into()],
            affected_dependents: vec!["notebook".into()],
            added_relations: vec!["compile_context → FileTypePrior".into()],
            test_results: vec!["cargo test 17/17 relevant tests passed".into()],
            objects_remapped: 4,
            ..Default::default()
        };
        let r = d.render();
        for section in ["CHANGED", "AFFECTED", "STRUCTURE", "VALIDATION", "MAP"] {
            assert!(r.contains(section), "missing {section}:\n{r}");
        }
        assert!(r.contains("+ relation: compile_context"));
    }
}
