//! Structural similarity — compare data structures (JSON, DB schemas, class
//! hierarchies) node-by-node in the shared embedding space.
//!
//! A structure is flattened into a list of addressable [`StructNode`]s (one per
//! field/branch). Each node's "signature" (key + type + sample value + nesting)
//! is embedded, then two structures are matched by cosine similarity between
//! their node vectors — so you can ask "which fields of schema A correspond to
//! schema B", "what does B have that A lacks", and "which of these 50 schemas
//! is shaped most like this one". This is the missing internal-structure layer
//! on top of the file/theme vector graph: the graph compares whole files, this
//! compares the *shape* of their contents field by field.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::embed::VectorEmbed;
use crate::models::cosine_sim;

/// One addressable node in a flattened structure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StructNode {
    /// Dotted/array path from the root, e.g. `user.address.zip` or `items[0].sku`.
    pub path: String,
    /// The last path segment (the field name).
    pub key: String,
    /// `object` | `array` | `leaf` — what kind of node this is.
    pub kind: String,
    /// For leaves: the JSON type (`string`/`number`/`bool`/`null`).
    /// For objects: `#obj`. For arrays: the element type hint (`[]string`).
    pub type_hint: String,
    /// Short, embeddable signature fragment (value sample for leaves, child
    /// count / element type for branches).
    pub sample: String,
}

/// A matched pair of nodes across two structures (above the similarity threshold).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructMatch {
    pub a_path: String,
    pub b_path: String,
    /// Cosine similarity of the two node signatures (0..1).
    pub score: f32,
}

/// The result of comparing two structures node-by-node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureGraph {
    pub nodes_a: Vec<StructNode>,
    pub nodes_b: Vec<StructNode>,
    /// Matched node pairs with cosine >= `threshold`.
    pub matches: Vec<StructMatch>,
    /// Best cosine achieved for each node in A (to any node in B). Used to
    /// compute `similarity`.
    pub best_a: Vec<f32>,
    /// Mean of `best_a` — a continuous 0..1 overall structural similarity even
    /// for sub-threshold matches.
    pub similarity: f32,
    /// Nodes present in A with no B match >= threshold (renamed/missing fields).
    pub missing_in_b: Vec<String>,
    /// Nodes present in B with no A match >= threshold (extra fields).
    pub extra_in_b: Vec<String>,
}

/// Flatten a JSON value into ordered [`StructNode`]s.
///
/// The root container itself is *not* emitted (only its children are), so a
/// top-level `{...}` or `[...]` doesn't produce a spurious empty-path node.
/// Arrays are represented by a single node plus a recursive visit of their
/// *first* element (so nested shape is captured without exploding on repeated
/// rows). Objects recurse into each child. Leaves carry their value sample.
pub fn flatten_json(value: &Value) -> Vec<StructNode> {
    let mut out = Vec::new();
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for k in keys {
                flatten_json_impl(&map[k], k.clone(), &mut out, 0);
            }
        }
        Value::Array(arr) => {
            if let Some(first) = arr.first() {
                flatten_json_impl(first, "[0]".into(), &mut out, 0);
            }
        }
        other => flatten_json_impl(other, String::new(), &mut out, 0),
    }
    out
}

fn flatten_json_impl(value: &Value, path: String, out: &mut Vec<StructNode>, depth: usize) {
    // Guard against pathological depth (e.g. a hand-rolled linked list).
    if depth > 32 {
        return;
    }
    match value {
        Value::Object(map) => {
            let key = leaf_key(&path);
            out.push(StructNode {
                path: path.clone(),
                key,
                kind: "object".into(),
                type_hint: "#obj".into(),
                sample: format!("{} fields", map.len()),
            });
            // Stable key order for deterministic output.
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for k in keys {
                let child_path = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                flatten_json_impl(&map[k], child_path, out, depth + 1);
            }
        }
        Value::Array(arr) => {
            let key = leaf_key(&path);
            let elem_hint = arr.first().map(type_name).unwrap_or_else(|| "empty".into());
            out.push(StructNode {
                path: path.clone(),
                key,
                kind: "array".into(),
                type_hint: format!("[]{elem_hint}"),
                sample: format!("{} elems of {}", arr.len(), elem_hint),
            });
            // Visit only the first element to capture nested element shape.
            if let Some(first) = arr.first() {
                let elem_path = if path.is_empty() {
                    "[0]".into()
                } else {
                    format!("{path}[0]")
                };
                flatten_json_impl(first, elem_path, out, depth + 1);
            }
        }
        other => {
            let key = leaf_key(&path);
            out.push(StructNode {
                path: path.clone(),
                key,
                kind: "leaf".into(),
                type_hint: type_name(other),
                sample: value_sample(other),
            });
        }
    }
}

/// Flatten an indentation-structured text (e.g. a class/struct/schema dump)
/// where each non-empty line is `key: Type` or `Type key`, and 2-space indents
/// denote nesting. Falls back gracefully: a flat list of fields still compares
/// well by key/type.
pub fn flatten_struct_text(text: &str) -> Vec<StructNode> {
    let mut out = Vec::new();
    let mut stack: Vec<(String, usize)> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") || line.starts_with("#") {
            continue;
        }
        let indent = raw.len() - raw.trim_start().len();
        let level = indent / 2;
        while stack.last().map(|&(_, l)| l >= level).unwrap_or(false) {
            stack.pop();
        }
        let parent = stack.last().map(|(p, _)| p.clone()).unwrap_or_default();
        let (key, type_hint) = parse_field_line(line);
        if key.is_empty() {
            continue;
        }
        let path = if parent.is_empty() {
            key.clone()
        } else {
            format!("{parent}.{key}")
        };
        out.push(StructNode {
            path: path.clone(),
            key: key.clone(),
            kind: "leaf".into(),
            type_hint: type_hint.clone(),
            sample: format!("type {type_hint}"),
        });
        stack.push((path, level));
    }
    out
}

/// Parse a field line into (key, type). Accepts `key: Type`, `key Type`,
/// `Type key`, and `key` (type unknown).
fn parse_field_line(line: &str) -> (String, String) {
    let line = line.trim_end_matches(['{', '}', ';', ',']).trim();
    if let Some((k, t)) = line.split_once(':') {
        let k = k.trim().to_string();
        let t = t.trim().to_string();
        if !k.is_empty() {
            return (k, if t.is_empty() { "unknown".into() } else { t });
        }
    }
    let parts: Vec<&str> = line.split_whitespace().collect();
    match parts.len() {
        0 => (String::new(), "unknown".into()),
        1 => (parts[0].to_string(), "unknown".into()),
        _ => {
            // Heuristic: if last token looks like an identifier and first like a
            // type, use "Type key"; else "key Type".
            let first = parts[0];
            let last = parts[parts.len() - 1];
            if last
                .chars()
                .next()
                .map(|c| c.is_lowercase())
                .unwrap_or(false)
            {
                (last.to_string(), first.to_string())
            } else {
                (first.to_string(), parts[1..].join(" "))
            }
        }
    }
}

fn leaf_key(path: &str) -> String {
    match path.rfind(['.', ']']) {
        Some(i) => {
            let start = if path.as_bytes().get(i) == Some(&b']') {
                // path like "items[0]" → key "items"
                i
            } else {
                i + 1
            };
            path[start..].to_string()
        }
        None => path.to_string(),
    }
}

fn type_name(v: &Value) -> String {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
    .into()
}

fn value_sample(v: &Value) -> String {
    match v {
        Value::String(s) => {
            let s = s.trim();
            if s.len() > 24 {
                format!("\"{}…\"", &s[..24])
            } else {
                format!("\"{s}\"")
            }
        }
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".into(),
        _ => type_name(v),
    }
}

/// The text embedded for a node — key + type + sample. The embedder decides how
/// much weight each part gets; semantic keys (e.g. "zipcode" vs "postal_code")
/// land close despite different strings.
fn node_signature(n: &StructNode) -> String {
    format!("field {} : {} = {}", n.key, n.type_hint, n.sample)
}

/// Embed a node list, returning nodes and their vectors (in matching order).
fn embed_nodes(nodes: &[StructNode], embedder: &dyn VectorEmbed) -> Vec<Vec<f32>> {
    if nodes.is_empty() {
        return Vec::new();
    }
    let sig_strings: Vec<String> = nodes.iter().map(node_signature).collect();
    let sigs: Vec<&str> = sig_strings.iter().map(|s| s.as_str()).collect();
    embedder.embed_batch(&sigs)
}

/// Compare two structures node-by-node via cosine similarity.
///
/// `threshold` is the minimum cosine for a node pair to count as a *match*;
/// `similarity` (mean best-cosine over A) is reported regardless.
pub fn compare_structures(
    a: &Value,
    b: &Value,
    embedder: &dyn VectorEmbed,
    threshold: f32,
) -> StructureGraph {
    let nodes_a = flatten_json(a);
    let nodes_b = flatten_json(b);
    compare_node_lists(&nodes_a, &nodes_b, embedder, threshold)
}

/// Compare two pre-flattened node lists (JSON or struct-text).
pub fn compare_node_lists(
    nodes_a: &[StructNode],
    nodes_b: &[StructNode],
    embedder: &dyn VectorEmbed,
    threshold: f32,
) -> StructureGraph {
    let emb_a = embed_nodes(nodes_a, embedder);
    let emb_b = embed_nodes(nodes_b, embedder);

    let mut best_a: Vec<f32> = Vec::with_capacity(nodes_a.len());
    let mut matches: Vec<StructMatch> = Vec::new();
    let mut matched_b: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for (i, na) in nodes_a.iter().enumerate() {
        let mut best = 0.0f32;
        let mut best_j = None;
        for (j, eb) in emb_b.iter().enumerate().take(nodes_b.len()) {
            let s = cosine_sim(&emb_a[i], eb);
            if s > best {
                best = s;
                best_j = Some(j);
            }
        }
        best_a.push(best);
        if best >= threshold {
            if let Some(j) = best_j {
                matched_b.insert(j);
                matches.push(StructMatch {
                    a_path: na.path.clone(),
                    b_path: nodes_b[j].path.clone(),
                    score: best,
                });
            }
        }
    }

    let similarity = if best_a.is_empty() {
        0.0
    } else {
        best_a.iter().sum::<f32>() / best_a.len() as f32
    };

    let missing_in_b: Vec<String> = nodes_a
        .iter()
        .enumerate()
        .filter(|(i, _)| best_a[*i] < threshold)
        .map(|(_, n)| n.path.clone())
        .collect();
    let extra_in_b: Vec<String> = nodes_b
        .iter()
        .enumerate()
        .filter(|(j, _)| !matched_b.contains(j))
        .map(|(_, n)| n.path.clone())
        .collect();

    StructureGraph {
        nodes_a: nodes_a.to_vec(),
        nodes_b: nodes_b.to_vec(),
        matches,
        best_a,
        similarity,
        missing_in_b,
        extra_in_b,
    }
}

/// Rank a corpus of structures by how similar each is to `query`.
/// Returns `(name, similarity)` pairs sorted by descending similarity.
pub fn search_structures(
    corpus: &[(String, Value)],
    query: &Value,
    embedder: &dyn VectorEmbed,
    threshold: f32,
) -> Vec<(String, f32)> {
    let q_nodes = flatten_json(query);
    let q_emb = embed_nodes(&q_nodes, embedder);
    let mut out = Vec::with_capacity(corpus.len());
    for (name, val) in corpus {
        let c_nodes = flatten_json(val);
        let c_emb = embed_nodes(&c_nodes, embedder);
        // Mean best-cosine of query nodes to corpus nodes.
        let mut sim_sum = 0.0f32;
        let mut sim_n = 0usize;
        for qe in &q_emb {
            let best = c_emb
                .iter()
                .map(|ce| cosine_sim(qe, ce))
                .fold(0.0f32, f32::max);
            sim_sum += best;
            sim_n += 1;
        }
        let sim = if sim_n == 0 {
            0.0
        } else {
            sim_sum / sim_n as f32
        };
        // Unused but kept for callers that want the threshold semantics.
        let _ = threshold;
        out.push((name.clone(), sim));
    }
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::RandomProjectionEmbedder;

    fn emb() -> RandomProjectionEmbedder {
        // RandomProjectionEmbedder is seeded, so embeddings are deterministic:
        // identical text → identical vector → cosine 1.0. This makes the
        // structural tests stable without any ONNX model.
        RandomProjectionEmbedder::new(64)
    }

    #[test]
    fn flat_json_fields() {
        let v = serde_json::json!({"name": "x", "age": 3});
        let nodes = flatten_json(&v);
        assert_eq!(nodes.len(), 2);
        assert!(nodes
            .iter()
            .any(|n| n.path == "name" && n.type_hint == "string"));
        assert!(nodes
            .iter()
            .any(|n| n.path == "age" && n.type_hint == "number"));
    }

    #[test]
    fn nested_and_array_shape() {
        let v = serde_json::json!({
            "user": {"address": {"zip": "12345"}},
            "items": [{"sku": "a", "qty": 2}]
        });
        let nodes = flatten_json(&v);
        let paths: Vec<&str> = nodes.iter().map(|n| n.path.as_str()).collect();
        assert!(paths.contains(&"user"));
        assert!(paths.contains(&"user.address"));
        assert!(paths.contains(&"user.address.zip"));
        assert!(paths.contains(&"items"));
        assert!(paths.contains(&"items[0].sku"));
        // Array element type captured as []object.
        assert!(nodes
            .iter()
            .any(|n| n.kind == "array" && n.type_hint == "[]object"));
    }

    #[test]
    fn identical_structures_match_perfectly() {
        let a = serde_json::json!({"id": 1, "name": "x", "tags": ["a"]});
        let b = a.clone();
        let g = compare_structures(&a, &b, &emb(), 0.0);
        assert!(
            (g.similarity - 1.0).abs() < 1e-5,
            "identical → similarity 1"
        );
        assert_eq!(g.missing_in_b.len(), 0);
        assert_eq!(g.extra_in_b.len(), 0);
    }

    #[test]
    fn field_absent_in_empty_structure_is_missing() {
        // A field present in A but in a B that has NO fields at all must be
        // reported missing — independent of embedder semantics.
        let a = serde_json::json!({"postal_code": "12345"});
        let b = serde_json::json!({});
        let g = compare_structures(&a, &b, &emb(), 0.5);
        assert!(g.missing_in_b.contains(&"postal_code".to_string()));
        assert!(g.extra_in_b.is_empty());
        assert!(g.matches.is_empty());
    }

    #[test]
    fn search_ranks_closest_first() {
        let target = serde_json::json!({"user": {"name": "a", "email": "b"}});
        let close = serde_json::json!({"user": {"name": "c", "email": "d"}});
        let far = serde_json::json!({"temperature": 99, "rpm": 12});
        let corpus = vec![("far".into(), far), ("close".into(), close)];
        let ranked = search_structures(&corpus, &target, &emb(), 0.0);
        assert_eq!(ranked[0].0, "close");
        assert!(ranked[0].1 > ranked[1].1);
    }

    #[test]
    fn struct_text_flatten() {
        let txt = "id: number\nname: string\naddress: object\n  street: string\n  city: string";
        let nodes = flatten_struct_text(txt);
        assert!(nodes
            .iter()
            .any(|n| n.path == "address.street" && n.type_hint == "string"));
        assert!(nodes
            .iter()
            .any(|n| n.path == "name" && n.type_hint == "string"));
    }

    #[test]
    fn graph_serializes() {
        let a = serde_json::json!({"x": 1});
        let b = serde_json::json!({"x": 2});
        let g = compare_structures(&a, &b, &emb(), 0.0);
        let json = serde_json::to_string(&g).unwrap();
        assert!(json.contains("similarity"));
        let back: StructureGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(back.nodes_a.len(), 1);
    }
}
