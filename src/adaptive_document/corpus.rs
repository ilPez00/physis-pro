//! The corpus: what the documents in this store say to each other.
//!
//! Two kinds of relation, and they are different in kind.
//!
//! **Declared.** A block can contain a hyperlink — `[label](target)` — which is
//! a person saying "this points there". Parsing one is exact, and following it
//! either resolves to a document in this store or it does not.
//!
//! **Observed.** A block that a model has read carries its vector. Two blocks
//! in different documents with a high cosine are similar, which is a statement
//! about the model's representation, not about anyone's intent. That is why
//! this is a *relation to inspect* and never an edge that propagates anything.
//!
//! The corpus is whatever stores exist under the root. A document that has
//! never been read by a model has no vectors and so contributes nothing to the
//! observed half — the scan counts it rather than inventing an entry for it.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::store::Store;
use super::BlockId;

/// Where a hyperlink points, as far as its text says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkTarget {
    /// A relative path that ends in a document extension.
    Document,
    /// http or https.
    Url,
    /// A `#fragment` within the same document.
    Anchor,
    /// Something else — a mailto, a wiki-style name, a typo.
    Other,
    /// `![alt](src)`. Recorded so it can be reported, never followed: an image
    /// is not a place to go.
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hyperlink {
    pub label: String,
    pub target: String,
    pub kind: LinkTarget,
}

impl Hyperlink {
    /// The path part of the target, without a `#fragment`.
    pub fn path(&self) -> &str {
        self.target.split('#').next().unwrap_or(&self.target)
    }

    /// The fragment, if the target carries one.
    pub fn fragment(&self) -> Option<&str> {
        self.target.split_once('#').map(|(_, f)| f)
    }

    /// Resolve a document link against the directory of the document holding
    /// it. Returns `None` for anything that is not a relative document path.
    pub fn resolve(&self, base_dir: &Path) -> Option<PathBuf> {
        if self.kind != LinkTarget::Document {
            return None;
        }
        let p = self.path();
        if p.is_empty() {
            return None;
        }
        let joined = if Path::new(p).is_absolute() {
            PathBuf::from(p)
        } else {
            base_dir.join(p)
        };
        Some(joined.canonicalize().unwrap_or(joined))
    }
}

/// Reference definitions — `[ref]: target` — keyed lowercased.
///
/// Collected in a first pass because a reference link can precede its
/// definition, which is the whole point of the form.
fn reference_definitions(text: &str) -> std::collections::BTreeMap<String, String> {
    let mut defs = std::collections::BTreeMap::new();
    for line in text.lines() {
        let line = line.trim_start();
        if !line.starts_with('[') {
            continue;
        }
        let Some(close) = line.find("]:") else { continue };
        let key = line[1..close].trim().to_ascii_lowercase();
        let target = line[close + 2..].trim();
        // Strip an optional title and the angle brackets of an autolink form.
        let target = target
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches('<')
            .trim_end_matches('>');
        if !key.is_empty() && !target.is_empty() {
            defs.insert(key, target.to_string());
        }
    }
    defs
}

fn push(out: &mut Vec<Hyperlink>, label: String, target: String, image: bool) {
    if label.is_empty() || target.is_empty() {
        return;
    }
    let kind = if image {
        LinkTarget::Image
    } else {
        classify_target(&target)
    };
    out.push(Hyperlink { label, kind, target });
}

/// Every hyperlink in `text`, in order: inline `[label](target)`, reference
/// `[label][ref]` resolved against the document's own `[ref]: target` lines,
/// images `![alt](src)`, and autolinks `<https://…>`.
///
/// Deliberately not a Markdown parser. It finds the forms and reports what they
/// say; anything it does not understand stays text, and a shortcut reference
/// `[ref]` with no definition is not a link because there is nothing to resolve.
pub fn hyperlinks(text: &str) -> Vec<Hyperlink> {
    let defs = reference_definitions(text);
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        // Autolink.
        if chars[i] == '<' {
            if let Some(end) = (i + 1..chars.len()).find(|&j| chars[j] == '>') {
                let t: String = chars[i + 1..end].iter().collect();
                if t.starts_with("http://") || t.starts_with("https://") {
                    out.push(Hyperlink {
                        label: t.clone(),
                        target: t,
                        kind: LinkTarget::Url,
                    });
                    i = end + 1;
                    continue;
                }
            }
        }
        let image = chars[i] == '!' && chars.get(i + 1) == Some(&'[');
        if chars[i] != '[' && !image {
            i += 1;
            continue;
        }
        let open = if image { i + 1 } else { i };
        let Some(close) = (open + 1..chars.len()).find(|&j| chars[j] == ']') else { break };
        let label: String = chars[open + 1..close].iter().collect();
        // Inline: [label](target)
        if chars.get(close + 1) == Some(&'(') {
            if let Some(end) = (close + 2..chars.len()).find(|&j| chars[j] == ')') {
                let target: String = chars[close + 2..end].iter().collect();
                push(&mut out, label, target, image);
                i = end + 1;
                continue;
            }
        }
        // Reference: [label][ref], or [label][] which is its own label.
        if chars.get(close + 1) == Some(&'[') {
            if let Some(rend) = (close + 2..chars.len()).find(|&j| chars[j] == ']') {
                let reference: String = chars[close + 2..rend].iter().collect();
                let key = if reference.trim().is_empty() {
                    label.clone()
                } else {
                    reference
                };
                if let Some(target) = defs.get(&key.trim().to_ascii_lowercase()) {
                    push(&mut out, label, target.clone(), image);
                    i = rend + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

fn classify_target(target: &str) -> LinkTarget {
    let lower = target.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return LinkTarget::Url;
    }
    if target.starts_with('#') {
        return LinkTarget::Anchor;
    }
    let path = target.split('#').next().unwrap_or(target);
    match Path::new(path).extension().and_then(|e| e.to_str()) {
        Some(ext) => match ext.to_ascii_lowercase().as_str() {
            "md" | "markdown" | "txt" | "text" => LinkTarget::Document,
            _ => LinkTarget::Other,
        },
        None => LinkTarget::Other,
    }
}

/// One block of one document, with whatever a model said about it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusEntry {
    /// Absolute path of the document, as the store records it.
    pub document: String,
    pub block_id: BlockId,
    pub text: String,
    pub cell: Option<(String, String)>,
    pub embedder: String,
    pub vector: Vec<f32>,
}

/// A scan of every store under a root.
#[derive(Debug, Clone, Default)]
pub struct Corpus {
    pub entries: Vec<CorpusEntry>,
    /// Stores that held a document.
    pub documents: usize,
    /// Documents with no vectors: never read by a model, so they contribute
    /// nothing to the observed half. Counted rather than silently dropped.
    pub unread: usize,
    pub truncated: bool,
}

impl Corpus {
    /// Read every store under `root`. Bounded, because a corpus is whatever
    /// happens to be on disk and this runs on a keystroke.
    pub fn scan(root: &Path, max_documents: usize, max_blocks: usize) -> Corpus {
        let mut corpus = Corpus::default();
        let Ok(read) = std::fs::read_dir(root) else { return corpus };
        let mut dirs: Vec<PathBuf> = read
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        for dir in dirs {
            if corpus.documents >= max_documents {
                corpus.truncated = true;
                break;
            }
            let store = Store::open(&dir);
            let Ok(Some(doc)) = store.load_document() else { continue };
            corpus.documents += 1;
            let observations = store.load_observations().unwrap_or_default();
            if observations.is_empty() {
                corpus.unread += 1;
                continue;
            }
            for o in observations {
                if corpus.entries.len() >= max_blocks {
                    corpus.truncated = true;
                    break;
                }
                if o.vector.is_empty() {
                    continue;
                }
                let Some(block) = doc.block(&o.block_id) else { continue };
                corpus.entries.push(CorpusEntry {
                    document: doc.id.clone(),
                    block_id: o.block_id.clone(),
                    text: block.text(&doc.raw).to_string(),
                    cell: o.cell.clone(),
                    embedder: o.embedder.clone(),
                    vector: o.vector.clone(),
                });
            }
        }
        corpus
    }

    /// Rank entries by cosine against a vector, best first. `exclude_document`
    /// drops one document's blocks, which is how an in-document neighbour list
    /// and a corpus neighbour list stay different questions.
    pub fn rank(&self, vector: &[f32], exclude_document: Option<&str>, k: usize) -> Vec<(usize, f32)> {
        let mut scored: Vec<(usize, f32)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| exclude_document != Some(e.document.as_str()))
            .map(|(i, e)| (i, cosine(vector, &e.vector)))
            .filter(|(_, s)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        scored
    }

    /// Blocks in *other* documents that resemble this vector.
    pub fn related(&self, vector: &[f32], document: &str, k: usize) -> Vec<(usize, f32)> {
        self.rank(vector, Some(document), k)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// A document path that has blocks in the corpus.
    pub fn document_path(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|e| e.document.as_str())
    }
}

/// One line describing what the scan found, for a status bar.
impl std::fmt::Display for Corpus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} document(s), {} embedded block(s){}",
            self.documents,
            self.entries.len(),
            if self.unread > 0 {
                format!(", {} never read by a model", self.unread)
            } else {
                String::new()
            }
        )?;
        if self.truncated {
            write!(f, " (scan cap reached)")?;
        }
        Ok(())
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..n {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adaptive_document::observation::Observation;
    use crate::adaptive_document::{parse_blocks, BlockKind, Link, LinkBehavior, LinkProvenance};

    #[test]
    fn hyperlinks_are_found_and_classified() {
        let text = "See [the report](report.md) and [the docs](https://example.com/x). \
                    Also [below](#section) and [mail](mailto:a@b.c).";
        let links = hyperlinks(text);
        assert_eq!(links.len(), 4);
        assert_eq!(links[0].label, "the report");
        assert_eq!(links[0].kind, LinkTarget::Document);
        assert_eq!(links[1].kind, LinkTarget::Url);
        assert_eq!(links[2].kind, LinkTarget::Anchor);
        assert_eq!(links[3].kind, LinkTarget::Other);
    }

    #[test]
    fn reference_links_resolve_against_the_documents_own_definitions() {
        let text = "See [the report][r] and [again][].\n\n[r]: report.md\n[again]: <other.md>\n";
        let links = hyperlinks(text);
        assert_eq!(links.len(), 2, "{links:?}");
        assert_eq!(links[0].label, "the report");
        assert_eq!(links[0].target, "report.md");
        assert_eq!(links[0].kind, LinkTarget::Document);
        assert_eq!(links[1].target, "other.md", "angle brackets are not part of the path");
    }

    #[test]
    fn a_reference_with_no_definition_is_not_a_link() {
        // A shortcut reference has nothing to resolve, so it stays text.
        assert!(hyperlinks("See [nowhere] for details.").is_empty());
        assert!(hyperlinks("See [nowhere][missing] for details.").is_empty());
        assert_eq!(hyperlinks("See [here][r].\n\n[r]: a.md\n").len(), 1);
    }

    #[test]
    fn autolinks_and_images_are_recognised() {
        let links = hyperlinks("Go to <https://example.com/x> or ![a chart](chart.png).");
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].kind, LinkTarget::Url);
        assert_eq!(links[0].target, "https://example.com/x");
        assert_eq!(links[1].kind, LinkTarget::Image);
        // An image is not a place to go.
        assert!(hyperlinks("![a](chart.png)")[0].resolve(Path::new("/tmp")).is_none());
        // Angle brackets that are not a URL stay text.
        assert!(hyperlinks("<not a link>").is_empty());
    }

    #[test]
    fn malformed_or_partial_links_are_left_as_text() {
        // The parser reports what it understands; anything else is not a link
        // and must not become one.
        assert!(hyperlinks("a [label] without a target").is_empty());
        assert!(hyperlinks("a [label](unclosed").is_empty());
        assert!(hyperlinks("[]()").is_empty(), "empty label and target");
        assert!(hyperlinks("plain text").is_empty());
        assert_eq!(hyperlinks("[a](b.md) and [c](d.md)").len(), 2);
    }

    #[test]
    fn a_document_link_resolves_against_the_holding_document() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("other.md"), "x").unwrap();
        let links = hyperlinks("[o](other.md#part)");
        let resolved = links[0].resolve(dir.path()).expect("resolves");
        assert!(resolved.ends_with("other.md"));
        assert!(resolved.exists());
        assert_eq!(links[0].fragment(), Some("part"));
        assert_eq!(links[0].path(), "other.md");
        // A URL is not a path and must not be resolved as one.
        assert!(hyperlinks("[x](https://a/b.md)")[0].resolve(dir.path()).is_none());
    }

    /// A store holding one document, with vectors on the blocks named by index.
    fn store_with(dir: &Path, key: &str, doc_id: &str, text: &str, vectors: Vec<(usize, Vec<f32>)>) {
        let store = Store::open(&dir.join(key));
        let doc = parse_blocks(doc_id, 0, text);
        store.save_document(&doc).unwrap();
        let observations: Vec<Observation> = vectors
            .into_iter()
            .map(|(i, vector)| Observation {
                block_id: doc.blocks[i].id.clone(),
                revision: 0,
                content_hash: 0,
                embedder: "test".into(),
                cell: Some(("A".into(), "B".into())),
                score: 0.5,
                entries: Vec::new(),
                neighbours: Vec::new(),
                isolated: false,
                vector,
            })
            .collect();
        store.save_observations(&observations).unwrap();
    }

    #[test]
    fn a_scan_reads_what_has_vectors_and_counts_what_does_not() {
        let dir = tempfile::tempdir().unwrap();
        store_with(dir.path(), "a", "/one.md", "Alpha one.\n\nBeta two.\n", vec![(1, vec![1.0, 0.0])]);
        // A store whose document was never read by a model.
        let root = dir.path().join("b");
        let store = Store::open(&root);
        store
            .save_document(&parse_blocks("/two.md", 0, "Gamma three.\n"))
            .unwrap();

        let corpus = Corpus::scan(dir.path(), 16, 64);
        assert_eq!(corpus.documents, 2);
        assert_eq!(corpus.len(), 1, "only the read document contributes");
        assert_eq!(corpus.unread, 1);
        assert!(corpus.to_string().contains("never read by a model"));
    }

    #[test]
    fn related_excludes_the_document_asked_about() {
        let dir = tempfile::tempdir().unwrap();
        store_with(dir.path(), "a", "/one.md", "Alpha one.\n\nBeta two.\n", vec![(1, vec![1.0, 0.0])]);
        store_with(dir.path(), "b", "/two.md", "Alpha one.\n\nBeta two.\n", vec![(1, vec![0.9, 0.1])]);
        let corpus = Corpus::scan(dir.path(), 16, 64);
        assert_eq!(corpus.len(), 2);
        let hits = corpus.related(&[1.0, 0.0], "/one.md", 5);
        assert_eq!(hits.len(), 1, "the same document is not its own neighbour");
        assert_eq!(corpus.document_path(hits[0].0), Some("/two.md"));
        assert!(hits[0].1 > 0.9);
    }

    #[test]
    fn ranking_ignores_degenerate_vectors() {
        let corpus = Corpus {
            entries: vec![
                CorpusEntry {
                    document: "/a.md".into(),
                    block_id: "x".into(),
                    text: "t".into(),
                    cell: None,
                    embedder: "test".into(),
                    vector: vec![0.0, 0.0],
                },
                CorpusEntry {
                    document: "/b.md".into(),
                    block_id: "y".into(),
                    text: "t".into(),
                    cell: None,
                    embedder: "test".into(),
                    vector: vec![1.0, 0.0],
                },
            ],
            documents: 2,
            unread: 0,
            truncated: false,
        };
        let hits = corpus.rank(&[1.0, 0.0], None, 5);
        assert_eq!(hits.len(), 1, "a zero vector is not a match");
        assert_eq!(hits[0].0, 1);
    }

    #[test]
    fn declared_links_and_observed_similarity_are_separate_records() {
        // A hyperlink is intent; a cosine is representation. Both are kept, and
        // neither is turned into the other.
        let text = "See [other](other.md).\n";
        let doc = parse_blocks("/a.md", 0, text);
        let links = hyperlinks(doc.blocks[0].text(text));
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].kind, LinkTarget::Document);
        // The block-level dependency link is a different type entirely.
        let l = Link {
            id: "l1".into(),
            source_block_id: doc.blocks[0].id.clone(),
            dependent_block_id: doc.blocks[0].id.clone(),
            behavior: LinkBehavior::Review,
            transform_id: None,
            provenance: LinkProvenance::HumanConfirmed,
            source_revision_at_confirmation: 0,
            active: true,
        };
        assert_eq!(l.behavior, LinkBehavior::Review);
        assert_eq!(doc.blocks[0].kind, BlockKind::Paragraph);
    }
}
