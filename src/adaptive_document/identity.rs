//! Block identity across edits (Phase 26, extension).
//!
//! A block's id is derived from its bytes, so editing that block gives it a new
//! one and every link pointing at it stops resolving. That is correct for a
//! fresh parse and wrong for a document that is being edited: the block is the
//! same block, its text changed.
//!
//! [`reidentify`] fixes that without guessing. It diffs the old bytes against
//! the new ones and carries an id forward in exactly two situations:
//!
//! 1. every line of the block that the diff matched lands in the same new
//!    block — that block is this passage, moved or edited, and inherits the id;
//! 2. the block has no matched line at all, and its changed region holds exactly
//!    one old block and one new block — an in-place replacement.
//!
//! Everything else is reported, never guessed:
//!
//! - matched lines landing in *several* new blocks is a split: the id names
//!   more than one passage now, so it is dropped and both halves are minted;
//! - two old blocks landing on one new block is a merge: the earlier passage
//!   keeps the id and the later one is orphaned;
//! - a changed region with a different number of blocks on each side is
//!   ambiguous, and nothing is claimed;
//! - a changed region with blocks on only one side is a deletion or an
//!   insertion, which is not an ambiguity.
//!
//! The transition is content-addressed. [`Transition::diff_hash`] hashes the
//! canonical edit script, so a stored transition can be checked against the
//! pair of documents it claims to connect ([`transition_matches`]).
//!
//! Nothing here is fuzzy and nothing here is learned. Similarity would put a
//! model version into the file format, and ids appear in link endpoints, in
//! patch targets, in patch reasons and in the graph-revision conflict check —
//! all of which have to be reproducible from bytes alone.

use super::{fnv1a, BlockId, Document};
use physis_core::relation::RelationType;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Cap on the edit distance the line diff will compute. Above it the diff gives
/// up and every block is treated as new, which is reported rather than guessed.
/// The trace is O(D²) memory, so this is also what bounds the cost.
pub const MAX_DIFF_EDITS: usize = 512;

/// One changed region: old line range and new line range, both half-open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hunk {
    pub old: (usize, usize),
    pub new: (usize, usize),
    /// Hash of the new lines this hunk introduces, so the record can be
    /// checked against the document it produced.
    pub new_hash: u64,
}

/// A new block that appeared where an old one was.
///
/// This is the causal record of a *minted* block. A carried block inherits its
/// history; a minted one has none, so its origin has to be derived from the
/// transition that produced it. That is the asymmetry: the interesting part of
/// a new block is not its id, it is where it came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub to: BlockId,
    /// Words the old and new text literally share. Zero is a real answer: the
    /// block took the other's place without sharing any of its text.
    pub shared_tokens: usize,
    pub new_tokens: usize,
    /// The hunk paired exactly one old block with exactly one new block, so the
    /// succession is positional even when the text has nothing in common.
    pub positional: bool,
}

/// An old block that no longer exists, and what now stands where it stood.
///
/// Offered, never applied: this is the same information that was refused as
/// identity evidence, kept in the form where a human can act on it. The block
/// gets a fresh id — the *link* may be re-anchored, by a person, using this.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Displacement {
    pub from: BlockId,
    /// The relation this succession asserts. Deliberately the weakest true one:
    /// the old text came before this one. Nothing epistemic is claimed, because
    /// nothing epistemic was observed.
    pub relation: RelationType,
    /// New blocks in the same changed region, best evidence first.
    pub candidates: Vec<Candidate>,
}

impl Displacement {
    /// The strongest candidate, if any.
    pub fn best(&self) -> Option<&Candidate> {
        self.candidates.first()
    }

    /// Whether this succession is a positional fact rather than an inference.
    pub fn is_exact(&self) -> bool {
        self.candidates.len() == 1 && self.candidates[0].positional
    }
}

/// The observed passage from one revision to the next, and what it did to block
/// identity. Written to the store so a reload can replay the reasoning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    pub document_id: String,
    pub from_revision: u64,
    pub to_revision: u64,
    pub before_hash: u64,
    pub after_hash: u64,
    /// Hash of the canonical edit script. Two documents with the same edit
    /// script hash made the same transition.
    pub diff_hash: u64,
    pub hunks: Vec<Hunk>,
    /// Blocks that kept the id they had.
    pub carried: usize,
    /// How strong the weakest carries were: words shared between the old and
    /// new text of the blocks carried on word evidence, out of the words in
    /// the new text. A carry on a single shared word is the weakest claim this
    /// makes, and this is where that is visible.
    pub carried_tokens: usize,
    pub carried_tokens_total: usize,
    /// Blocks that are new, including any whose match was ambiguous.
    pub minted: usize,
    /// Ids that no longer name a block in the document.
    pub orphaned: Vec<BlockId>,
    /// The subset of `orphaned` the diff could not decide about. Empty means
    /// every orphan was a genuine deletion.
    pub ambiguous: Vec<BlockId>,
    /// For each block that is gone, what now stands where it stood. This is the
    /// causal record of the minted blocks, and what a dangling link is
    /// re-anchored from. Defaulted: records written before this existed load
    /// with none, which is exactly what they knew.
    #[serde(default)]
    pub displacements: Vec<Displacement>,
    /// The diff hit [`MAX_DIFF_EDITS`]; the block mapping is not trustworthy.
    pub diff_truncated: bool,
}

impl Transition {
    /// What now stands where `old_id` stood, best evidence first.
    pub fn candidates_for(&self, old_id: &str) -> &[Candidate] {
        self.displacements
            .iter()
            .find(|d| d.from == old_id)
            .map(|d| d.candidates.as_slice())
            .unwrap_or(&[])
    }

    /// The successions to record causally: `(what is there now, what was
    /// there before)`, one pair per displacement.
    ///
    /// This is what makes the record queryable rather than merely stored. Fed
    /// to a [`crate::ledger::CausalLedger`] as `Mechanism::Corrected` events,
    /// `lineage()` walks backwards through a chain of replacements and
    /// `descendants()` walks forwards from a block that no longer exists.
    pub fn successions(&self) -> Vec<(BlockId, BlockId)> {
        self.displacements
            .iter()
            .filter_map(|d| d.best().map(|c| (c.to.clone(), d.from.clone())))
            .collect()
    }

    /// Whether this passage is complex enough to be worth a model.
    ///
    /// Deliberately narrow. A single block edited in place, or a single block
    /// inserted, is already accounted for by the diff — calling a model there
    /// would be paying for an opinion about something already known. What
    /// warrants one is exactly where the exact evidence ran out: a region the
    /// diff could not pair up (ambiguous), or several blocks appearing at once,
    /// which means the passage was restructured rather than edited.
    pub fn warrants_model(&self) -> bool {
        !self.ambiguous.is_empty() || self.minted >= 2
    }

    /// Why the model was or was not called, in one line.
    pub fn complexity(&self) -> String {
        format!(
            "{} hunk(s), {} minted, {} ambiguous, {} replaced, {} carried{}",
            self.hunks.len(),
            self.minted,
            self.ambiguous.len(),
            self.displacements.len(),
            self.carried,
            if self.warrants_model() { " — warrants a model" } else { "" }
        )
    }

    /// A one-line summary for a log or a status bar.
    pub fn summary(&self) -> String {
        let mut s = format!(
            "rev {} → {}: {} carried, {} minted",
            self.from_revision, self.to_revision, self.carried, self.minted
        );
        if self.carried_tokens_total > 0 {
            s.push_str(&format!(
                " ({} of {} words matched in the edited blocks)",
                self.carried_tokens, self.carried_tokens_total
            ));
        }
        if !self.orphaned.is_empty() {
            s.push_str(&format!(", {} orphaned", self.orphaned.len()));
        }
        if !self.ambiguous.is_empty() {
            s.push_str(&format!(", {} ambiguous", self.ambiguous.len()));
        }
        if !self.displacements.is_empty() {
            s.push_str(&format!(", {} replaced", self.displacements.len()));
        }
        if self.diff_truncated {
            s.push_str(" (diff cap reached: identity not trustworthy)");
        }
        s
    }
}

/// Byte range of every line, including its trailing newline. A final line
/// without a newline is still a line.
fn line_spans(s: &str) -> Vec<(usize, usize)> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            out.push((start, i + 1));
            start = i + 1;
        }
        i += 1;
    }
    if start < bytes.len() {
        out.push((start, bytes.len()));
    }
    out
}

/// Index of the line containing `byte`.
fn line_of(spans: &[(usize, usize)], byte: usize) -> Option<usize> {
    let i = spans.partition_point(|&(start, _)| start <= byte);
    i.checked_sub(1)
}

fn hash_lines(lines: &[&str]) -> u64 {
    let mut joined = String::new();
    for l in lines {
        joined.push_str(l);
    }
    fnv1a(joined.as_bytes())
}

/// The longest common subsequence of lines, as `(old_index, new_index)` pairs.
///
/// Myers' O(ND) algorithm. `None` when the edit distance exceeds `max_d`, which
/// the caller must report rather than paper over.
fn diff_matching(a: &[&str], b: &[&str], max_d: usize) -> Option<Vec<(usize, usize)>> {
    let n = a.len() as isize;
    let m = b.len() as isize;
    let max = ((n + m) as usize).min(max_d) as isize;
    let offset = max + 1;
    let mut v = vec![0isize; (2 * max + 3) as usize];
    let mut trace: Vec<Vec<isize>> = Vec::new();
    let mut found: Option<isize> = None;

    'outer: for d in 0..=max {
        trace.push(v.clone());
        let mut k = -d;
        while k <= d {
            let mut x = if k == -d
                || (k != d && v[(k - 1 + offset) as usize] < v[(k + 1 + offset) as usize])
            {
                v[(k + 1 + offset) as usize]
            } else {
                v[(k - 1 + offset) as usize] + 1
            };
            let mut y = x - k;
            while x < n && y < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[(k + offset) as usize] = x;
            if x >= n && y >= m {
                found = Some(d);
                break 'outer;
            }
            k += 2;
        }
    }

    let d_final = found?;
    let mut equal: Vec<(usize, usize)> = Vec::new();
    let (mut x, mut y) = (n, m);
    for d in (0..=d_final).rev() {
        let vv = &trace[d as usize];
        let k = x - y;
        let prev_k = if k == -d
            || (k != d && vv[(k - 1 + offset) as usize] < vv[(k + 1 + offset) as usize])
        {
            k + 1
        } else {
            k - 1
        };
        let prev_x = vv[(prev_k + offset) as usize];
        let prev_y = prev_x - prev_k;
        while x > prev_x && y > prev_y {
            equal.push(((x - 1) as usize, (y - 1) as usize));
            x -= 1;
            y -= 1;
        }
        x = prev_x;
        y = prev_y;
    }
    equal.reverse();
    Some(equal)
}

/// The changed regions implied by a matching.
fn hunks_of(matching: &[(usize, usize)], a: &[&str], b: &[&str]) -> Vec<Hunk> {
    let mut out = Vec::new();
    let (mut ai, mut bi) = (0usize, 0usize);
    for &(ea, eb) in matching {
        if ea > ai || eb > bi {
            out.push(Hunk {
                old: (ai, ea),
                new: (bi, eb),
                new_hash: hash_lines(&b[bi..eb]),
            });
        }
        ai = ea + 1;
        bi = eb + 1;
    }
    if ai < a.len() || bi < b.len() {
        out.push(Hunk {
            old: (ai, a.len()),
            new: (bi, b.len()),
            new_hash: hash_lines(&b[bi..]),
        });
    }
    out
}

/// Canonical hash of a transition: the document hashes plus every hunk's
/// coordinates and the hash of the lines it introduces.
fn diff_hash(before: u64, after: u64, hunks: &[Hunk]) -> u64 {
    let mut s = format!("{before:016x}:{after:016x}");
    for h in hunks {
        s.push_str(&format!(
            ";{},{},{},{},{:016x}",
            h.old.0, h.old.1, h.new.0, h.new.1, h.new_hash
        ));
    }
    fnv1a(s.as_bytes())
}

/// Does a stored transition actually connect these two documents?
///
/// Recomputes the diff and compares the hash. This proves the record matches
/// the bytes; it does not prove the record was ever true, and it is not offered
/// as if it did.
pub fn transition_matches(prev_raw: &str, new_raw: &str, t: &Transition) -> bool {
    let a: Vec<&str> = line_texts(prev_raw);
    let b: Vec<&str> = line_texts(new_raw);
    let Some(matching) = diff_matching(&a, &b, MAX_DIFF_EDITS) else {
        return t.diff_truncated;
    };
    let hunks = hunks_of(&matching, &a, &b);
    diff_hash(fnv1a(prev_raw.as_bytes()), fnv1a(new_raw.as_bytes()), &hunks) == t.diff_hash
}

fn line_texts(s: &str) -> Vec<&str> {
    let spans = line_spans(s);
    spans.iter().map(|&(a, b)| &s[a..b]).collect()
}

/// Alphanumeric runs, which is the granularity identity evidence is read at.
///
/// Line granularity is too coarse: "Budget is 36 euros." and "Budget is 9
/// euros." share no line, so a line diff cannot tell that the block survived an
/// edit — and that is the most common edit there is. Words can.
fn tokens(s: &str) -> Vec<&str> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect()
}

/// Cap on the token count of a pair we will compare. Above it the evidence is
/// declined rather than approximated.
const MAX_EVIDENCE_TOKENS: usize = 4000;

/// Longest common subsequence of two token lists, compared case-insensitively.
/// This is the evidence that two blocks are the same passage: not a similarity
/// score, a count of words that are literally present in both.
fn matched_tokens(a: &str, b: &str) -> usize {
    let a = tokens(a);
    let b = tokens(b);
    if a.is_empty() || b.is_empty() || a.len() > MAX_EVIDENCE_TOKENS || b.len() > MAX_EVIDENCE_TOKENS {
        return 0;
    }
    let (short, long) = if a.len() <= b.len() { (&a, &b) } else { (&b, &a) };
    let mut prev = vec![0usize; short.len() + 1];
    let mut cur = vec![0usize; short.len() + 1];
    for lt in long.iter() {
        for (j, st) in short.iter().enumerate() {
            cur[j + 1] = if st.eq_ignore_ascii_case(lt) {
                prev[j] + 1
            } else {
                prev[j + 1].max(cur[j])
            };
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[short.len()]
}

/// Re-parse `raw` and carry forward the identity of every block the diff can
/// account for. Returns the new document and the transition record.
pub fn reidentify(prev: &Document, raw: &str) -> (Document, Transition) {
    let mut next = super::parse_blocks(&prev.id, prev.revision + 1, raw);
    next.links = prev.links.clone();
    next.graph_revision = prev.graph_revision;

    let a_texts = line_texts(&prev.raw);
    let b_texts = line_texts(raw);
    let a_spans = line_spans(&prev.raw);
    let b_spans = line_spans(raw);

    let matching = diff_matching(&a_texts, &b_texts, MAX_DIFF_EDITS);
    let truncated = matching.is_none();
    let matching = matching.unwrap_or_default();
    let hunks = hunks_of(&matching, &a_texts, &b_texts);

    // old line -> new line, for the lines the diff matched exactly.
    let equal: BTreeMap<usize, usize> = matching.iter().copied().collect();

    // Blocks fully inside each hunk, for the wholesale-replacement case.
    let old_ranges: Vec<Option<(usize, usize)>> = prev
        .blocks
        .iter()
        .map(|b| block_line_range(&a_spans, b.span))
        .collect();
    let new_ranges: Vec<Option<(usize, usize)>> = next
        .blocks
        .iter()
        .map(|b| block_line_range(&b_spans, b.span))
        .collect();

    // new block index -> the old id it inherits.
    let mut carried_at: BTreeMap<usize, BlockId> = BTreeMap::new();
    let mut ambiguous: Vec<BlockId> = Vec::new();
    let mut claimed: BTreeSet<usize> = BTreeSet::new();
    // A split is the one case where the pieces are named by the anchors rather
    // than by the changed region: a blank line inserted between two paragraphs
    // is a hunk that contains no block at all.
    let mut split_targets: BTreeMap<BlockId, Vec<usize>> = BTreeMap::new();

    // 1. Follow the lines the diff matched. A block whose matched lines all
    //    land in one new block is that block, and keeps its id. If they land in
    //    several, the block was split: the id names more than one passage now,
    //    so it is dropped and both halves are minted. If another old block
    //    already claimed that new block, the two were merged and the earlier
    //    passage keeps the identity — the later one is simply gone.
    for (oi, old_block) in prev.blocks.iter().enumerate() {
        let Some((first, last)) = old_ranges[oi] else { continue };
        let mut targets: BTreeSet<usize> = BTreeSet::new();
        for l in first..=last {
            let Some(&new_line) = equal.get(&l) else { continue };
            let Some(byte) = b_spans.get(new_line).map(|&(s, _)| s) else { continue };
            if let Some(ni) = new_block_at(&next, byte) {
                targets.insert(ni);
            }
        }
        match targets.len() {
            0 => {}
            1 => {
                let ni = *targets.iter().next().unwrap();
                if claimed.insert(ni) {
                    carried_at.insert(ni, old_block.id.clone());
                }
            }
            _ => {
                ambiguous.push(old_block.id.clone());
                split_targets.insert(old_block.id.clone(), targets.iter().copied().collect());
            }
        }
    }

    // 2. A block whose lines were all rewritten has no line to follow. If its
    //    hunk holds exactly one old block and one new block, ask the words:
    //    text present in both is evidence that this is the same passage
    //    edited, and no shared word at all means the passage was replaced.
    //    Anything else in the hunk is a split or a merge of different shapes,
    //    and is reported rather than guessed.
    let mut anchored: BTreeSet<BlockId> = carried_at.values().cloned().collect();
    let mut evidence_matched = 0usize;
    let mut evidence_total = 0usize;
    for h in &hunks {
        let old_here: Vec<usize> = (0..prev.blocks.len())
            .filter(|&i| {
                !anchored.contains(&prev.blocks[i].id)
                    && old_ranges[i]
                        .map(|r| r.0 >= h.old.0 && r.1 <= h.old.1)
                        .unwrap_or(false)
            })
            .collect();
        let new_here: Vec<usize> = (0..next.blocks.len())
            .filter(|&i| {
                !claimed.contains(&i)
                    && new_ranges[i]
                        .map(|r| r.0 >= h.new.0 && r.1 <= h.new.1)
                        .unwrap_or(false)
            })
            .collect();
        if old_here.len() == 1 && new_here.len() == 1 {
            let (oi, ni) = (old_here[0], new_here[0]);
            let before = prev.blocks[oi].text(&prev.raw);
            let after = next.blocks[ni].text(raw);
            let matched = matched_tokens(before, after);
            if matched > 0 {
                carried_at.insert(ni, prev.blocks[oi].id.clone());
                anchored.insert(prev.blocks[oi].id.clone());
                claimed.insert(ni);
                evidence_matched += matched;
                evidence_total += tokens(after).len();
            }
            // No shared word: the passage is gone. That is a decision, not an
            // uncertainty, so the old block is orphaned and not ambiguous.
        } else if !old_here.is_empty() && !new_here.is_empty() {
            // Both sides have blocks and they do not pair up. Neither side is
            // declared to be the other. A hunk with only one side is a
            // deletion or an insertion, which is not an ambiguity.
            for &oi in &old_here {
                ambiguous.push(prev.blocks[oi].id.clone());
            }
        }
    }

    // 3. For every old block that is gone, record what now stands where it
    //    stood. This is derived cause, not identity: the block gets a fresh id
    //    regardless, and the candidate list is an offer for a person to act on.
    let mut displacements: Vec<Displacement> = Vec::new();
    for (oi, old_block) in prev.blocks.iter().enumerate() {
        if anchored.contains(&old_block.id) {
            continue;
        }
        let before = old_block.text(&prev.raw);
        let mut candidates: Vec<Candidate> = if let Some(nis) = split_targets.get(&old_block.id) {
            // Split: the pieces are exactly the blocks that inherited the
            // matched lines. Neither is the old block, but both are where its
            // text went.
            nis.iter()
                .map(|&ni| {
                    let after = next.blocks[ni].text(raw);
                    Candidate {
                        to: next.blocks[ni].id.clone(),
                        shared_tokens: matched_tokens(before, after),
                        new_tokens: tokens(after).len(),
                        positional: false,
                    }
                })
                .collect()
        } else {
            let Some((first, _)) = old_ranges[oi] else { continue };
            let Some(h) = hunks.iter().find(|h| first >= h.old.0 && first < h.old.1) else {
                continue;
            };
            if h.new.1 <= h.new.0 {
                continue; // a deletion: nothing took its place
            }
            let Some(&(nstart, _)) = b_spans.get(h.new.0) else { continue };
            let nend = b_spans
                .get(h.new.1 - 1)
                .map(|&(_, e)| e)
                .unwrap_or_else(|| raw.len());
            // Positional means the region paired exactly one old block with
            // exactly one new block: a fact about the diff, not a guess.
            let old_here = old_ranges
                .iter()
                .filter(|r| r.map(|r| r.0 >= h.old.0 && r.1 <= h.old.1).unwrap_or(false))
                .count();
            let positional = old_here == 1;
            next.blocks
                .iter()
                .filter(|b| b.span.0 >= nstart && b.span.1 <= nend)
                .map(|b| {
                    let after = b.text(raw);
                    Candidate {
                        to: b.id.clone(),
                        shared_tokens: matched_tokens(before, after),
                        new_tokens: tokens(after).len(),
                        positional,
                    }
                })
                .collect()
        };
        candidates.sort_by(|a, b| {
            b.shared_tokens
                .cmp(&a.shared_tokens)
                .then(b.positional.cmp(&a.positional))
                .then(a.to.cmp(&b.to))
        });
        if candidates.is_empty() {
            continue;
        }
        displacements.push(Displacement {
            from: old_block.id.clone(),
            relation: RelationType::Precedes,
            candidates,
        });
    }

    // 4. Apply the mapping, then make ids unique.
    let mut seen: BTreeSet<BlockId> = BTreeSet::new();
    let mut minted = 0usize;
    for (i, b) in next.blocks.iter_mut().enumerate() {
        if let Some(old_id) = carried_at.get(&i) {
            if !seen.contains(old_id) {
                b.id = old_id.clone();
                seen.insert(b.id.clone());
                continue;
            }
        }
        if seen.contains(&b.id) {
            let base = b.id.clone();
            let mut n = 2u32;
            while seen.contains(&format!("{base}-{n}")) {
                n += 1;
            }
            b.id = format!("{base}-{n}");
        }
        seen.insert(b.id.clone());
        minted += 1;
    }

    let carried = carried_at.len();
    let orphaned: Vec<BlockId> = prev
        .blocks
        .iter()
        .filter(|b| !seen.contains(&b.id))
        .map(|b| b.id.clone())
        .collect();

    let before = fnv1a(prev.raw.as_bytes());
    let after = fnv1a(raw.as_bytes());
    let transition = Transition {
        document_id: next.id.clone(),
        from_revision: prev.revision,
        to_revision: next.revision,
        before_hash: before,
        after_hash: after,
        diff_hash: diff_hash(before, after, &hunks),
        hunks,
        carried,
        carried_tokens: evidence_matched,
        carried_tokens_total: evidence_total,
        minted,
        orphaned,
        ambiguous,
        displacements,
        diff_truncated: truncated,
    };
    (next, transition)
}

fn block_line_range(spans: &[(usize, usize)], span: (usize, usize)) -> Option<(usize, usize)> {
    if span.1 <= span.0 {
        return None;
    }
    let first = line_of(spans, span.0)?;
    let last = line_of(spans, span.1 - 1)?;
    Some((first, last))
}

fn new_block_at(doc: &Document, byte: usize) -> Option<usize> {
    doc.blocks
        .iter()
        .position(|b| b.span.0 <= byte && byte < b.span.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adaptive_document::{Link, LinkBehavior, LinkProvenance, parse_blocks};

    fn doc_with_link(raw: &str) -> Document {
        let mut d = parse_blocks("t", 0, raw);
        let source = d.blocks[1].id.clone();
        let dependent = d.blocks[2].id.clone();
        d.links = vec![Link {
            id: "l1".into(),
            source_block_id: source,
            dependent_block_id: dependent,
            behavior: LinkBehavior::Calculate,
            transform_id: Some("scale:2".into()),
            provenance: LinkProvenance::HumanConfirmed,
            source_revision_at_confirmation: 0,
            active: true,
        }];
        d.graph_revision = 1;
        d
    }

    const BASE: &str = "# Report\n\nWe ordered 18 units.\n\nBudget is 36 euros.\n\nCheck terms.\n";

    #[test]
    fn a_block_edited_in_place_keeps_its_id() {
        // The whole point: "Budget is 36 euros." becomes "Budget is 9 euros."
        // and the link that names that block still names it.
        let prev = doc_with_link(BASE);
        let budget_before = prev.blocks[2].id.clone();
        let edited = "# Report\n\nWe ordered 18 units.\n\nBudget is 9 euros.\n\nCheck terms.\n";
        let (next, t) = reidentify(&prev, edited);
        assert_eq!(next.blocks[2].text(&next.raw), "Budget is 9 euros.");
        assert_eq!(next.blocks[2].id, budget_before, "the block kept its identity");
        assert_eq!(t.carried, 4);
        assert_eq!(t.minted, 0);
        assert!(t.orphaned.is_empty());
        assert!(next.links[0].dependent_block_id == budget_before);
        assert!(next.block(&next.links[0].dependent_block_id).is_some());
    }

    #[test]
    fn inserting_a_block_above_does_not_renumber_the_rest() {
        let prev = doc_with_link(BASE);
        let ids: Vec<BlockId> = prev.blocks.iter().map(|b| b.id.clone()).collect();
        let inserted = "# Report\n\nA new first paragraph.\n\nWe ordered 18 units.\n\nBudget is 36 euros.\n\nCheck terms.\n";
        let (next, t) = reidentify(&prev, inserted);
        assert_eq!(t.carried, ids.len());
        assert_eq!(t.minted, 1);
        for id in &ids {
            assert!(next.block(id).is_some(), "old block {id} survived an insert");
        }
    }

    #[test]
    fn deleting_a_linked_block_orphans_exactly_that_block() {
        let prev = doc_with_link(BASE);
        let budget = prev.blocks[2].id.clone();
        let deleted = "# Report\n\nWe ordered 18 units.\n\nCheck terms.\n";
        let (next, t) = reidentify(&prev, deleted);
        assert!(next.block(&budget).is_none());
        assert_eq!(t.orphaned, vec![budget.clone()]);
        assert!(t.ambiguous.is_empty(), "a deletion is not an ambiguity");
        assert!(t.carried >= 3);
    }

    #[test]
    fn absorbing_the_next_block_orphans_only_that_block() {
        // A paragraph swallows the one after it. The absorbed block is gone;
        // the absorbing block keeps its id, because its own first line is
        // exactly where the diff says it is.
        let raw = "Alpha one.\n\nBeta two.\n\nGamma three.\n";
        let prev = parse_blocks("t", 0, raw);
        let alpha = prev.blocks[0].id.clone();
        let beta = prev.blocks[1].id.clone();
        let merged = "Alpha one.\nBeta two.\n\nGamma three.\n";
        let (next, t) = reidentify(&prev, merged);
        assert_eq!(t.carried, 2, "Alpha and Gamma keep their ids");
        assert_eq!(t.orphaned, vec![beta.clone()]);
        assert!(t.ambiguous.is_empty(), "a deletion is not an ambiguity");
        assert_eq!(next.blocks[0].id, alpha);
        assert!(next.block(&beta).is_none());
        let ids: BTreeSet<&str> = next.blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids.len(), next.blocks.len(), "ids stay unique");
    }

    #[test]
    fn splitting_one_block_drops_the_id_and_mints_both_halves() {
        // Neither half is the old block, so neither inherits its id.
        let raw = "Alpha one.\nBeta two.\n";
        let prev = parse_blocks("t", 0, raw);
        let original = prev.blocks[0].id.clone();
        let split = "Alpha one.\n\nBeta two.\n";
        let (next, t) = reidentify(&prev, split);
        assert_eq!(t.carried, 0);
        assert_eq!(t.ambiguous, vec![original.clone()]);
        assert_eq!(t.minted, 2);
        assert_eq!(t.orphaned, vec![original]);
        let ids: BTreeSet<&str> = next.blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids.len(), next.blocks.len(), "ids stay unique");
    }

    #[test]
    fn a_block_replaced_by_unrelated_text_is_not_carried() {
        // One old block, one new block, nothing in common. The hunk pairs them
        // 1:1, but there is no evidence they are the same passage, so the id
        // is not reused. This is the case a bare 1:1 rule gets wrong.
        let raw = "Alpha one.\n\nBeta two.\n";
        let prev = parse_blocks("t", 0, raw);
        let alpha = prev.blocks[0].id.clone();
        let (next, t) = reidentify(&prev, "Something else entirely.\n\nBeta two.\n");
        assert_eq!(t.carried, 1, "only Beta, which is untouched");
        assert!(t.ambiguous.is_empty(), "a replacement is a decision, not a doubt");
        assert_eq!(t.orphaned, vec![alpha.clone()]);
        assert!(next.block(&alpha).is_none());
        assert_eq!(t.minted, 1);
    }

    #[test]
    fn a_value_edit_inside_a_block_is_carried_on_word_evidence() {
        // No line survives, so this is exactly the case the line diff cannot
        // decide and the word evidence must.
        let prev = doc_with_link(BASE);
        let budget = prev.blocks[2].id.clone();
        let (next, t) = reidentify(&prev, "# Report\n\nWe ordered 18 units.\n\nBudget is 9 euros.\n\nCheck terms.\n");
        assert_eq!(next.blocks[2].id, budget, "the edited block kept its identity");
        assert_eq!(t.carried, 4);
        assert!(t.carried_tokens >= 3, "Budget / is / euros are shared");
        assert!(t.carried_tokens_total >= 4);
        assert!(t.orphaned.is_empty());
    }

    #[test]
    fn a_replaced_block_records_what_took_its_place() {
        // A minted block's origin is derived, not inherited: the diff says
        // which region it appeared in and what stood there before.
        let raw = "Alpha one.\n\nBeta two.\n";
        let prev = parse_blocks("t", 0, raw);
        let alpha = prev.blocks[0].id.clone();
        let (next, t) = reidentify(&prev, "Something else entirely.\n\nBeta two.\n");
        assert_eq!(t.displacements.len(), 1, "Alpha was replaced");
        let d = &t.displacements[0];
        assert_eq!(d.from, alpha);
        assert_eq!(d.candidates.len(), 1);
        assert_eq!(d.candidates[0].to, next.blocks[0].id);
        assert_eq!(d.candidates[0].shared_tokens, 0, "nothing in common, and that is said");
        assert!(d.candidates[0].positional, "one old block, one new block");
        assert!(d.is_exact(), "positional and single: a fact, not an inference");
        assert_eq!(t.candidates_for(&alpha).len(), 1);
    }

    #[test]
    fn a_deleted_block_has_no_replacement_to_offer() {
        let raw = "Alpha one.\n\nBeta two.\n";
        let prev = parse_blocks("t", 0, raw);
        let alpha = prev.blocks[0].id.clone();
        let (_, t) = reidentify(&prev, "Beta two.\n");
        assert!(t.displacements.is_empty(), "nothing took its place");
        assert!(t.candidates_for(&alpha).is_empty());
    }

    #[test]
    fn a_split_offers_both_halves_ranked_by_shared_words() {
        // Neither half is the old block, so identity is not carried — but a
        // dangling link can be pointed at either, best evidence first.
        let raw = "Alpha one.\nBeta two.\n";
        let prev = parse_blocks("t", 0, raw);
        let original = prev.blocks[0].id.clone();
        let split = "Alpha one.\n\nBeta two.\n";
        let (_, t) = reidentify(&prev, split);
        assert_eq!(t.carried, 0, "the id names more than one passage now");
        let d = t
            .displacements
            .iter()
            .find(|d| d.from == original)
            .expect("a displacement for the split source");
        assert_eq!(d.candidates.len(), 2, "both halves are offered");
        assert!(
            d.candidates[0].shared_tokens >= d.candidates[1].shared_tokens,
            "best evidence first"
        );
        assert!(!d.is_exact(), "two candidates is not a positional fact");
    }

    #[test]
    fn successions_name_what_replaced_what() {
        let raw = "Alpha one.\n\nBeta two.\n";
        let prev = parse_blocks("t", 0, raw);
        let alpha = prev.blocks[0].id.clone();
        let (next, t) = reidentify(&prev, "Something else entirely.\n\nBeta two.\n");
        let pairs = t.successions();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, next.blocks[0].id, "what is there now");
        assert_eq!(pairs[0].1, alpha, "what was there before");
    }

    #[test]
    fn a_simple_edit_does_not_warrant_a_model() {
        // The diff already accounted for it; a model would add nothing.
        let prev = doc_with_link(BASE);
        let (_, t) = reidentify(&prev, "# Report\n\nWe ordered 18 units.\n\nBudget is 9 euros.\n\nCheck terms.\n");
        assert_eq!(t.minted, 0);
        assert!(t.ambiguous.is_empty());
        assert!(!t.warrants_model(), "{}", t.complexity());
    }

    #[test]
    fn an_inserted_block_does_not_warrant_a_model_but_a_restructure_does() {
        let prev = parse_blocks("t", 0, "Alpha one.\n\nGamma three.\n");
        let (_, one) = reidentify(&prev, "Alpha one.\n\nBeta two.\n\nGamma three.\n");
        assert_eq!(one.minted, 1);
        assert!(!one.warrants_model(), "one insertion is not a restructure");

        let (_, two) = reidentify(&prev, "Alpha one.\n\nBeta two.\n\nDelta four.\n\nGamma three.\n");
        assert_eq!(two.minted, 2);
        assert!(two.warrants_model(), "{}", two.complexity());
    }

    #[test]
    fn an_ambiguous_region_warrants_a_model() {
        let prev = parse_blocks("t", 0, "Alpha one.\nBeta two.\n");
        let (_, t) = reidentify(&prev, "Something else.\n\nAnd another.\n");
        assert!(!t.ambiguous.is_empty());
        assert!(t.warrants_model(), "{}", t.complexity());
    }

    #[test]
    fn a_transition_without_displacements_still_loads() {
        // Records written before displacements existed must keep loading.
        let prev = doc_with_link(BASE);
        let (_, t) = reidentify(&prev, BASE);
        let mut v = serde_json::to_value(&t).unwrap();
        v.as_object_mut().unwrap().remove("displacements");
        let back: Transition = serde_json::from_value(v).unwrap();
        assert!(back.displacements.is_empty());
        assert!(back.candidates_for("anything").is_empty());
    }

    #[test]
    fn one_block_replaced_by_two_is_ambiguous() {
        // Nothing matched, and the changed region holds one old block against
        // two new ones. No pairing is claimed; the old id is reported.
        let raw = "Alpha one.\nBeta two.\n";
        let prev = parse_blocks("t", 0, raw);
        let original = prev.blocks[0].id.clone();
        let replaced = "Something else.\n\nAnd another.\n";
        let (next, t) = reidentify(&prev, replaced);
        assert_eq!(t.carried, 0);
        assert_eq!(t.ambiguous, vec![original.clone()]);
        assert_eq!(t.minted, 2);
        assert_eq!(t.orphaned, vec![original]);
        let ids: BTreeSet<&str> = next.blocks.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids.len(), next.blocks.len());
    }

    #[test]
    fn deleting_two_blocks_is_a_deletion_not_an_ambiguity() {
        let raw = "Alpha one.\n\nBeta two.\n\nGamma three.\n";
        let prev = parse_blocks("t", 0, raw);
        let alpha = prev.blocks[0].id.clone();
        let beta = prev.blocks[1].id.clone();
        let (next, t) = reidentify(&prev, "Something else entirely.\n\nGamma three.\n");
        assert_eq!(t.carried, 1, "Gamma is untouched");
        assert!(t.ambiguous.is_empty(), "nothing matched, so nothing was split");
        assert_eq!(t.orphaned.len(), 2);
        assert!(t.orphaned.contains(&alpha) && t.orphaned.contains(&beta));
        assert_eq!(t.minted, 1);
        assert!(next.block(&alpha).is_none());
    }

    #[test]
    fn the_diff_hash_is_stable_and_depends_on_the_pair() {
        let prev = doc_with_link(BASE);
        let a = "# Report\n\nWe ordered 18 units.\n\nBudget is 9 euros.\n\nCheck terms.\n";
        let b = "# Report\n\nWe ordered 18 units.\n\nBudget is 7 euros.\n\nCheck terms.\n";
        let (_, t1) = reidentify(&prev, a);
        let (_, t2) = reidentify(&prev, a);
        let (_, t3) = reidentify(&prev, b);
        assert_eq!(t1.diff_hash, t2.diff_hash, "same transition, same hash");
        assert_ne!(t1.diff_hash, t3.diff_hash, "different transition, different hash");
        assert!(transition_matches(&prev.raw, a, &t1));
        assert!(!transition_matches(&prev.raw, b, &t1), "a mismatched record is caught");
    }

    #[test]
    fn carried_identity_survives_a_second_edit() {
        // Identity has to be stable across a chain, not just one hop.
        let prev = doc_with_link(BASE);
        let budget = prev.blocks[2].id.clone();
        let (mid, _) = reidentify(&prev, "# Report\n\nWe ordered 18 units.\n\nBudget is 9 euros.\n\nCheck terms.\n");
        let (next, t) = reidentify(&mid, "# Report\n\nWe ordered 18 units.\n\nBudget is 4 euros.\n\nCheck terms.\n");
        assert_eq!(next.blocks[2].id, budget);
        assert_eq!(t.carried, 4);
        assert!(next.block(&budget).is_some());
    }

    #[test]
    fn an_unchanged_reparse_reports_every_block_carried() {
        let prev = doc_with_link(BASE);
        let (next, t) = reidentify(&prev, BASE);
        assert_eq!(t.hunks.len(), 0, "no hunks for identical bytes");
        assert_eq!(t.carried, prev.blocks.len());
        assert_eq!(t.minted, 0);
        assert_eq!(t.orphaned.len(), 0);
        assert_eq!(next.revision, prev.revision + 1);
    }

    #[test]
    fn block_ranges_are_line_ranges() {
        let raw = "one\ntwo\n\nthree\n";
        let spans = line_spans(raw);
        assert_eq!(spans.len(), 4);
        assert_eq!(line_of(&spans, 0), Some(0));
        assert_eq!(line_of(&spans, 4), Some(1));
        assert_eq!(line_of(&spans, 9), Some(3));
    }
}
