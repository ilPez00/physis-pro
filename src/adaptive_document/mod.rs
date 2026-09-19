//! Phase 26 adaptive document: blocks with exact byte spans, confirmed
//! dependency links, observed change events, and revision-safe patches.
//!
//! Contracts follow `PLAN.md` Phase 26 §26.4. The module is deliberately
//! self-contained: it does not depend on `dependency`, `staleness`, or
//! `repair_regions`, whose current state is documented as not ready for
//! integration (Phase 26 §26.2). Persistence reuses [`crate::atomic`].

pub mod proposal;
pub mod store;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable opaque identity for a block.
pub type BlockId = String;
/// Stable opaque identity for a link.
pub type LinkId = String;
/// Identity of a registered pure transform.
pub type TransformId = String;

/// Supported block kinds (Phase 26 §26.4). Unsupported Markdown forms stay
/// inside `Other` blocks rather than being silently reinterpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    Heading,
    Paragraph,
    Code,
    Other,
}

/// A block spans exact UTF-8 bytes of the document at a named revision.
/// Concatenating all block bytes in span order reproduces the document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: BlockId,
    pub kind: BlockKind,
    /// `[utf8_byte_start, utf8_byte_end)` at the document revision.
    pub span: (usize, usize),
    pub content_hash: u64,
    /// Explicit metadata, never inferred from proximity (Phase 26 §26.4).
    pub locked: bool,
    pub authored_by: String,
}

impl Block {
    pub fn text<'a>(&self, raw: &'a str) -> &'a str {
        &raw[self.span.0..self.span.1]
    }
}

/// Direction is source → dependent. Only `human_confirmed` links produce
/// automatic replacement candidates; `review` links only produce a reason to
/// inspect (Phase 26 §26.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub id: LinkId,
    pub source_block_id: BlockId,
    pub dependent_block_id: BlockId,
    pub behavior: LinkBehavior,
    /// Registered pure transform for calculation/copy; never arbitrary code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform_id: Option<TransformId>,
    /// Human-confirmed links may propagate automatically; parser-proven links
    /// are advisory and never auto-propagate.
    pub provenance: LinkProvenance,
    pub source_revision_at_confirmation: u64,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkBehavior {
    /// Dependent text quotes the source span verbatim.
    Copy,
    /// Dependent text is computed from source spans by a registered transform.
    Calculate,
    /// A reason to inspect; invents no replacement prose.
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkProvenance {
    HumanConfirmed,
    ParserProven,
}

/// A parsed, hashed document at a named revision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub revision: u64,
    pub raw: String,
    pub blocks: Vec<Block>,
    pub links: Vec<Link>,
    /// Monotonic revision of the link graph, checked by patch application.
    pub graph_revision: u64,
    #[serde(default)]
    pub actor: String,
}

/// FNV-1a: exact, dependency-free, stable across processes.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Parse a document into blocks over a documented Markdown subset: blank-line
/// separated paragraphs, ATX headings, and fenced code blocks. Blank lines
/// inside fences are not paragraph boundaries (Phase 26 §26.4). Block spans
/// cover *content only*: the blank-line separators between blocks are gap
/// bytes, preserved verbatim by any splice, so outside-byte equality holds.
///
/// A paragraph whose text contains `<!-- locked -->` is marked locked; the
/// marker stays in the raw bytes so outside-byte equality is preserved.
pub fn parse_blocks(id: &str, revision: u64, raw: &str) -> Document {
    let bytes = raw.as_bytes();
    let mut blocks: Vec<Block> = Vec::new();
    let mut i = 0usize;
    let mut occurrence: BTreeMap<(BlockKind, u64), u32> = BTreeMap::new();

    fn line_end_at(bytes: &[u8], from: usize) -> usize {
        bytes[from..]
            .iter()
            .position(|&b| b == b'\n')
            .map(|p| from + p + 1)
            .unwrap_or(bytes.len())
    }

    let push = |blocks: &mut Vec<Block>,
                kind: BlockKind,
                span: (usize, usize),
                occurrence: &mut BTreeMap<(BlockKind, u64), u32>,
                locked: bool| {
        let text = &bytes[span.0..span.1];
        let hash = fnv1a(text);
        let n = occurrence.entry((kind, hash)).or_insert(0);
        *n += 1;
        blocks.push(Block {
            id: format!("{kind:?}-{hash:016x}-{n}").to_lowercase(),
            kind,
            span,
            content_hash: hash,
            locked,
            authored_by: "human".into(),
        });
    };

    while i < bytes.len() {
        // Separator newlines are gap bytes between blocks; skip them.
        if bytes[i] == b'\n' || bytes[i] == b'\r' {
            i += 1;
            continue;
        }
        let line_end = line_end_at(bytes, i);
        let line = raw[i..line_end].trim_end_matches(['\n', '\r']);
        let line_content_end = i + line.len();

        if line.starts_with("```") {
            // Fenced code: consume to the closing fence; interior blank lines
            // are content, not separators. Span ends at the closing fence.
            let fence = line;
            let mut j = line_end;
            let mut end = line_content_end;
            while j < bytes.len() {
                let le = line_end_at(bytes, j);
                let l = raw[j..le].trim_end_matches(['\n', '\r']);
                end = j + l.len();
                if l.starts_with(fence) {
                    j = le;
                    break;
                }
                j = le;
            }
            push(
                &mut blocks,
                BlockKind::Code,
                (i, end),
                &mut occurrence,
                false,
            );
            i = j;
        } else if line.starts_with('#') {
            push(
                &mut blocks,
                BlockKind::Heading,
                (i, line_content_end),
                &mut occurrence,
                false,
            );
            i = line_end;
        } else {
            // Paragraph: consume lines until a blank line, heading, or fence.
            let mut j = line_end;
            let mut end = line_content_end;
            while j < bytes.len() {
                let le = line_end_at(bytes, j);
                let l = raw[j..le].trim_end_matches(['\n', '\r']);
                if l.trim().is_empty() || l.starts_with('#') || l.starts_with("```") {
                    break;
                }
                end = j + l.len();
                j = le;
            }
            let locked = raw[i..end].contains("<!-- locked -->");
            push(
                &mut blocks,
                BlockKind::Paragraph,
                (i, end),
                &mut occurrence,
                locked,
            );
            i = j;
        }
    }

    Document {
        id: id.to_string(),
        revision,
        raw: raw.to_string(),
        blocks,
        links: Vec::new(),
        graph_revision: 0,
        actor: "system".into(),
    }
}
impl Document {
    pub fn block(&self, id: &str) -> Option<&Block> {
        self.blocks.iter().find(|b| b.id == id)
    }

    /// Replace one block's bytes in-place, returning the new document text and
    /// updated block. Byte-offset arithmetic is done on UTF-8 bytes; browser
    /// UTF-16 indices must be converted at the API boundary (Phase 26 §26.4).
    pub fn splice_block(&self, id: &str, new_bytes: &str) -> Option<(Document, Block)> {
        let old = self.block(id)?;
        let mut doc = self.clone();
        doc.revision += 1;
        doc.raw.replace_range(old.span.0..old.span.1, new_bytes);
        let delta = new_bytes.len() as i64 - (old.span.1 - old.span.0) as i64;
        for b in doc.blocks.iter_mut() {
            if b.id == id {
                b.span.1 = (b.span.1 as i64 + delta) as usize;
                b.content_hash = fnv1a(new_bytes.as_bytes());
            } else if b.span.0 >= old.span.1 {
                b.span.0 = (b.span.0 as i64 + delta) as usize;
                b.span.1 = (b.span.1 as i64 + delta) as usize;
            } else if b.span.1 > old.span.0 {
                // Overlapping span: parse invariants violated.
                return None;
            }
        }
        let updated = doc.block(id)?.clone();
        Some((doc, updated))
    }

    /// Blocks reachable from `start` over active, human-confirmed links,
    /// including `start` itself. Deduplicated, stable order, capped.
    /// Returns the order plus whether the cap truncated traversal (AD13).
    pub fn reachable_dependents(&self, start: &str, max_nodes: usize) -> (Vec<BlockId>, bool) {
        let mut order = vec![start.to_string()];
        let mut seen: std::collections::BTreeSet<BlockId> = order.iter().cloned().collect();
        let mut queue = vec![start.to_string()];
        let mut truncated = false;
        while let Some(current) = queue.pop() {
            for link in &self.links {
                if !link.active || link.provenance != LinkProvenance::HumanConfirmed {
                    continue;
                }
                if link.source_block_id == current && !seen.contains(&link.dependent_block_id) {
                    if order.len() >= max_nodes {
                        truncated = true;
                        continue;
                    }
                    seen.insert(link.dependent_block_id.clone());
                    order.push(link.dependent_block_id.clone());
                    queue.push(link.dependent_block_id.clone());
                }
            }
        }
        (order, truncated)
    }

    /// Topological order of the calculation subgraph restricted to `ids`.
    /// Returns `Err(cycle_members)` when a cycle exists (AD13).
    pub fn topological_calculation_order(
        &self,
        ids: &[BlockId],
    ) -> Result<Vec<BlockId>, Vec<BlockId>> {
        let set: std::collections::BTreeSet<&BlockId> = ids.iter().collect();
        let mut indegree: BTreeMap<&BlockId, usize> = BTreeMap::new();
        let mut edges: BTreeMap<&BlockId, Vec<&BlockId>> = BTreeMap::new();
        for id in &set {
            indegree.entry(id).or_insert(0);
        }
        for link in &self.links {
            if !link.active
                || link.behavior != LinkBehavior::Calculate
                || link.provenance != LinkProvenance::HumanConfirmed
            {
                continue;
            }
            if set.contains(&link.source_block_id) && set.contains(&link.dependent_block_id) {
                edges
                    .entry(&link.source_block_id)
                    .or_default()
                    .push(&link.dependent_block_id);
                *indegree.entry(&link.dependent_block_id).or_insert(0) += 1;
            }
        }
        let mut queue: Vec<&BlockId> = indegree
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut order = Vec::new();
        while let Some(id) = queue.pop() {
            order.push(id.clone());
            if let Some(nexts) = edges.get(id) {
                for n in nexts.clone() {
                    let d = indegree.get_mut(n).unwrap();
                    *d -= 1;
                    if *d == 0 {
                        queue.push(n);
                    }
                }
            }
        }
        if order.len() == set.len() {
            Ok(order)
        } else {
            let done: std::collections::BTreeSet<&BlockId> = order.iter().collect();
            Err(set.difference(&done).cloned().cloned().collect())
        }
    }
}

/// An observed change event (Phase 26 §26.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub event_id: String,
    pub document_id: String,
    pub block_id: BlockId,
    pub before_revision: u64,
    pub after_revision: u64,
    pub before_hash: u64,
    pub after_hash: u64,
    pub actor: String,
    pub operation: String,
    pub sequence: u64,
    pub recorded_at: u64,
}
/// A proposed or applied set of replacements plus review reasons.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    pub id: String,
    pub document_id: String,
    pub triggered_by_change_id: String,
    pub expected_document_revision: u64,
    pub expected_graph_revision: u64,
    pub replacements: Vec<Replacement>,
    pub review_only: Vec<ReviewNote>,
    pub status: PatchStatus,
    /// Document revision recorded at first application; replayed verbatim so a
    /// repeated idempotency key applies once (AD08).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_revision: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Replacement {
    pub block_id: BlockId,
    /// Block-local byte range `[start, end)` relative to the block's span.
    pub range: (usize, usize),
    pub expected_old_bytes: String,
    pub new_bytes: String,
    /// Links that justify this replacement (Phase 26 §26.4).
    pub reasons: Vec<LinkId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewNote {
    pub block_id: BlockId,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub link_ids: Vec<LinkId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchStatus {
    Proposed,
    Applied,
    Rejected,
    Conflicted,
    Failed,
    Reverted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyError {
    /// Document revision does not match the patch expectation (AD07).
    StaleDocumentRevision,
    /// Graph revision does not match; links changed since proposal (AD07).
    StaleGraphRevision,
    /// Expected old bytes do not match the current content (AD05/AD07).
    ContentMismatch,
    /// Replacements overlap each other (AD05).
    OverlappingRanges,
    /// The patch is not in the `Proposed` state (AD09).
    NotProposed,
    /// A replacement targets a block that no longer exists.
    MissingBlock,
    /// Undo target no longer matches the document it produced (AD10).
    UndoConflict,
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Registered pure transforms. `copy` yields the source bytes verbatim;
/// `scale:<n>` replaces the first integer in the *dependent* bytes with
/// `n *` the first integer in the *source* bytes, preserving surrounding
/// text. Registration is explicit; unknown transform ids return `None` —
/// no arbitrary code ever executes.
pub fn apply_transform(
    transform_id: &str,
    source_bytes: &str,
    dependent_bytes: &str,
) -> Option<String> {
    if transform_id == "copy" {
        return Some(source_bytes.to_string());
    }
    if let Some(rest) = transform_id.strip_prefix("scale:") {
        let factor: i64 = rest.parse().ok()?;
        let src: Vec<char> = source_bytes.chars().collect();
        let s = src.iter().position(|c| c.is_ascii_digit())?;
        let mut e = s;
        while e < src.len() && src[e].is_ascii_digit() {
            e += 1;
        }
        let value: i64 = src[s..e].iter().collect::<String>().parse().ok()?;
        let computed = (value * factor).to_string();
        // Replace the dependent's first integer with the computed value.
        let dep: Vec<char> = dependent_bytes.chars().collect();
        let d = dep.iter().position(|c| c.is_ascii_digit())?;
        let mut d_end = d;
        while d_end < dep.len() && dep[d_end].is_ascii_digit() {
            d_end += 1;
        }
        let mut out = String::new();
        out.extend(dep[..d].iter());
        out.push_str(&computed);
        out.extend(dep[d_end..].iter());
        return Some(out);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Order\n\nWe need 18 units of bolts.\n\nBudget is 36 euros.\n";

    #[test]
    fn blocks_and_gaps_reconstruct_the_document_exactly() {
        let doc = parse_blocks("d", 0, SAMPLE);
        let mut rebuilt = String::new();
        for (idx, block) in doc.blocks.iter().enumerate() {
            if idx > 0 {
                // Gap bytes between blocks are separators: whitespace only.
                let gap = &SAMPLE[doc.blocks[idx - 1].span.1..block.span.0];
                assert!(gap.bytes().all(|b| b == b'\n' || b == b'\r' || b == b' '));
                rebuilt.push_str(gap);
            }
            rebuilt.push_str(block.text(SAMPLE));
        }
        // Trailing gap after the last block (e.g. final newline).
        rebuilt.push_str(&SAMPLE[doc.blocks.last().unwrap().span.1..]);
        assert_eq!(rebuilt, SAMPLE);
    }

    #[test]
    fn fence_interior_blank_lines_are_content() {
        let raw = "```text\nline one\n\nline two\n```\n";
        let doc = parse_blocks("d", 0, raw);
        assert_eq!(doc.blocks.len(), 1);
        assert_eq!(doc.blocks[0].kind, BlockKind::Code);
    }

    #[test]
    fn locked_marker_is_explicit_metadata() {
        let raw = "A free paragraph.\n\n<!-- locked -->Do not change.\n";
        let doc = parse_blocks("d", 0, raw);
        assert!(!doc.blocks[0].locked);
        assert!(doc.blocks[1].locked);
    }

    #[test]
    fn splice_preserves_outside_bytes_and_shifts_spans() {
        let doc = parse_blocks("d", 0, SAMPLE);
        let target = doc.blocks[1].id.clone();
        assert_eq!(
            doc.block(&target).unwrap().text(SAMPLE),
            "We need 18 units of bolts."
        );
        let (next, _) = doc
            .splice_block(&target, "We need 3 units of bolts.")
            .unwrap();
        assert_eq!(
            next.raw,
            "# Order\n\nWe need 3 units of bolts.\n\nBudget is 36 euros.\n"
        );
        let last = next.blocks.last().unwrap();
        assert_eq!(last.text(&next.raw), "Budget is 36 euros.");
    }

    #[test]
    fn transforms_are_pure_and_text_preserving() {
        assert_eq!(
            apply_transform("copy", "3 units", "old text"),
            Some("3 units".into())
        );
        assert_eq!(
            apply_transform("scale:2", "3 units", "Budget is 36 euros."),
            Some("Budget is 6 euros.".into())
        );
        assert_eq!(apply_transform("eval_js", "1", "1"), None);
    }
}
