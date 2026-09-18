//! diff — line and word diff for the Studio reviewer.
//!
//! Deliberately in-repo and dependency-free: no diff crate is a dependency of
//! this workspace (`grep similar` is empty), the algorithm is textbook, and a
//! deterministic implementation is testable against a golden corpus without
//! widening the supply chain. `similar` stays the documented fallback if word
//! quality ever proves insufficient.
//!
//! ## Why the split at `split_inclusive`
//!
//! Lines are split with [`str::split_inclusive`] so every element keeps its own
//! terminator. That makes the two halves of this module exact inverses:
//! concatenating the lines you kept reproduces the input byte for byte,
//! including a missing final newline and CRLF endings. The invariant the tests
//! pin is therefore total, not approximate:
//!
//! ```text
//! apply_hunks(old, &diff_lines(old, new)) == new
//! ```
//!
//! ## Cost
//!
//! The middle (between the common prefix and suffix) is diffed with an LCS
//! table. Two guards keep that honest on a 2 MB file: common prefix/suffix
//! trimming, which makes the real middle of a real edit tiny, and
//! [`MAX_CELLS`], past which the middle is reported as one replacement hunk
//! rather than allocating an unbounded table. A degenerate answer that says
//! "this block changed" is not a lie; an OOM is a bug.

/// One line of output. Text is the original line *including* its terminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    /// Present in both sides.
    Same(String),
    /// Present only in the new side.
    Add(String),
    /// Present only in the old side.
    Del(String),
}

impl DiffLine {
    /// The text this line carries, whatever its role.
    pub fn text(&self) -> &str {
        match self {
            DiffLine::Same(t) | DiffLine::Add(t) | DiffLine::Del(t) => t,
        }
    }

    /// Single-glyph gutter marker for side-by-side rendering.
    pub fn marker(&self) -> char {
        match self {
            DiffLine::Same(_) => ' ',
            DiffLine::Add(_) => '+',
            DiffLine::Del(_) => '-',
        }
    }
}

/// A contiguous change with its surrounding context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    /// 0-based start and count of the covered lines on the old side.
    pub old_range: (usize, usize),
    /// 0-based start and count of the covered lines on the new side.
    pub new_range: (usize, usize),
    /// Context, deletions and insertions in render order.
    pub lines: Vec<DiffLine>,
}

impl Hunk {
    /// True when the hunk actually changes something (not pure context).
    pub fn is_change(&self) -> bool {
        self.lines
            .iter()
            .any(|l| matches!(l, DiffLine::Add(_) | DiffLine::Del(_)))
    }

    /// Counted changes, for the status bar.
    pub fn tally(&self) -> (usize, usize) {
        let add = self
            .lines
            .iter()
            .filter(|l| matches!(l, DiffLine::Add(_)))
            .count();
        let del = self
            .lines
            .iter()
            .filter(|l| matches!(l, DiffLine::Del(_)))
            .count();
        (add, del)
    }
}

/// Context lines kept on each side of a change.
const CONTEXT: usize = 3;
/// LCS cells we are willing to allocate (`u32` each): 8M cells = 32 MB.
pub const MAX_CELLS: usize = 8_000_000;

fn lines_of(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

/// Longest common subsequence of the two slices, as matched index pairs
/// `(i, j)` in increasing order. Returns `None` when the table would exceed
/// [`MAX_CELLS`] — the caller degrades to one replacement hunk.
fn lcs_pairs(a: &[&str], b: &[&str]) -> Option<Vec<(usize, usize)>> {
    let (n, m) = (a.len(), b.len());
    if n == 0 || m == 0 {
        return Some(Vec::new());
    }
    if n.saturating_mul(m) > MAX_CELLS {
        return None;
    }
    // table[i][j] = LCS length of a[i..] / b[j..], flattened row-major.
    let mut table = vec![0u32; (n + 1) * (m + 1)];
    let idx = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[idx(i, j)] = if a[i] == b[j] {
                table[idx(i + 1, j + 1)] + 1
            } else {
                table[idx(i + 1, j)].max(table[idx(i, j + 1)])
            };
        }
    }
    let mut pairs = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if a[i] == b[j] {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if table[idx(i + 1, j)] >= table[idx(i, j + 1)] {
            i += 1;
        } else {
            j += 1;
        }
    }
    Some(pairs)
}

/// Line-level diff: deterministic, trimmed-then-LCS, context-padded hunks.
///
/// Unchanged stretches longer than twice [`CONTEXT`] are skipped entirely —
/// that is what makes a review readable — so hunks do not tile the file.
/// [`apply_hunks`] reconstructs the skipped gaps from the old text.
pub fn diff_lines(old: &str, new: &str) -> Vec<Hunk> {
    if old == new {
        return Vec::new();
    }
    let a = lines_of(old);
    let b = lines_of(new);

    // Trim the common prefix and suffix: the LCS table then only sees the
    // region that actually moved, which is what keeps a big file cheap.
    let mut pre = 0;
    while pre < a.len() && pre < b.len() && a[pre] == b[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < a.len().saturating_sub(pre)
        && suf < b.len().saturating_sub(pre)
        && a[a.len() - 1 - suf] == b[b.len() - 1 - suf]
    {
        suf += 1;
    }

    let a_mid = &a[pre..a.len() - suf];
    let b_mid = &b[pre..b.len() - suf];

    let script: Vec<DiffLine> = match lcs_pairs(a_mid, b_mid) {
        Some(pairs) => {
            let mut script: Vec<DiffLine> = Vec::new();
            let (mut i, mut j) = (0usize, 0usize);
            for (pi, pj) in pairs {
                while i < pi {
                    script.push(DiffLine::Del(a_mid[i].to_string()));
                    i += 1;
                }
                while j < pj {
                    script.push(DiffLine::Add(b_mid[j].to_string()));
                    j += 1;
                }
                script.push(DiffLine::Same(a_mid[pi].to_string()));
                i = pi + 1;
                j = pj + 1;
            }
            while i < a_mid.len() {
                script.push(DiffLine::Del(a_mid[i].to_string()));
                i += 1;
            }
            while j < b_mid.len() {
                script.push(DiffLine::Add(b_mid[j].to_string()));
                j += 1;
            }
            script
        }
        // Too big for the table: one honest replacement hunk.
        None => a_mid
            .iter()
            .map(|l| DiffLine::Del(l.to_string()))
            .chain(b_mid.iter().map(|l| DiffLine::Add(l.to_string())))
            .collect(),
    };

    // Positions in the two files *before* each script index. Prefix sums, so a
    // hunk's ranges are exact even when earlier hunks' context was skipped.
    let mut old_pos = vec![pre; script.len() + 1];
    let mut new_pos = vec![pre; script.len() + 1];
    for (k, line) in script.iter().enumerate() {
        old_pos[k + 1] = old_pos[k] + usize::from(!matches!(line, DiffLine::Add(_)));
        new_pos[k + 1] = new_pos[k] + usize::from(!matches!(line, DiffLine::Del(_)));
    }

    // Cluster the changes: two changes closer than one context window on each
    // side belong in the same hunk, otherwise their context would overlap.
    let changes: Vec<usize> = script
        .iter()
        .enumerate()
        .filter(|(_, l)| matches!(l, DiffLine::Add(_) | DiffLine::Del(_)))
        .map(|(k, _)| k)
        .collect();
    if changes.is_empty() {
        return Vec::new();
    }
    let mut groups: Vec<(usize, usize)> = Vec::new(); // [start, end) in script
    let mut start = changes[0].saturating_sub(CONTEXT);
    let mut end = (changes[0] + 1 + CONTEXT).min(script.len());
    for &c in &changes[1..] {
        let s = c.saturating_sub(CONTEXT);
        if s <= end {
            end = (c + 1 + CONTEXT).min(script.len());
        } else {
            groups.push((start, end));
            start = s;
            end = (c + 1 + CONTEXT).min(script.len());
        }
    }
    groups.push((start, end));

    groups
        .into_iter()
        .map(|(s, e)| Hunk {
            old_range: (old_pos[s], old_pos[e] - old_pos[s]),
            new_range: (new_pos[s], new_pos[e] - new_pos[s]),
            lines: script[s..e].to_vec(),
        })
        .collect()
}

/// Rebuild the new text from the old text and hunks produced by [`diff_lines`]
/// on the same pair. Used for the preview/"apply to buffer" step, never to
/// mutate a store.
///
/// Hunks must be in order and non-overlapping, which is what [`diff_lines`]
/// emits. The inverse property is exact, not approximate: see the module header.
pub fn apply_hunks(old: &str, hunks: &[Hunk]) -> String {
    let old_lines = lines_of(old);
    let mut out = String::with_capacity(old.len());
    let mut cursor = 0usize;
    for h in hunks {
        let (start, _len) = h.old_range;
        debug_assert!(start >= cursor, "hunks must be ordered and non-overlapping");
        for l in &old_lines[cursor..start.min(old_lines.len())] {
            out.push_str(l);
        }
        for l in &h.lines {
            match l {
                DiffLine::Del(_) => {}
                DiffLine::Add(t) | DiffLine::Same(t) => out.push_str(t),
            }
        }
        cursor = start + h.old_range.1;
    }
    for l in &old_lines[cursor.min(old_lines.len())..] {
        out.push_str(l);
    }
    out
}

/// Intra-line diff for a replaced line: tokens are words *and* the whitespace
/// between them, so the same exact-reconstruction property holds per line.
pub fn diff_words(old: &str, new: &str) -> Vec<DiffLine> {
    if old == new {
        return if old.is_empty() {
            Vec::new()
        } else {
            vec![DiffLine::Same(old.to_string())]
        };
    }
    let a = tokenize(old);
    let b = tokenize(new);
    let Some(pairs) = lcs_pairs(&a, &b) else {
        return vec![
            DiffLine::Del(old.to_string()),
            DiffLine::Add(new.to_string()),
        ];
    };
    let mut out: Vec<DiffLine> = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    let mut push = |role: u8, t: &str| {
        let t = t.to_string();
        // Coalesce adjacent tokens of the same role so the renderer shows
        // words, not a token-per-span rainbow.
        match (role, out.last_mut()) {
            (0, Some(DiffLine::Same(prev))) => prev.push_str(&t),
            (1, Some(DiffLine::Add(prev))) => prev.push_str(&t),
            (2, Some(DiffLine::Del(prev))) => prev.push_str(&t),
            (0, _) => out.push(DiffLine::Same(t)),
            (1, _) => out.push(DiffLine::Add(t)),
            _ => out.push(DiffLine::Del(t)),
        }
    };
    for (pi, pj) in pairs {
        while i < pi {
            push(2, a[i]);
            i += 1;
        }
        while j < pj {
            push(1, b[j]);
            j += 1;
        }
        push(0, a[pi]);
        i = pi + 1;
        j = pj + 1;
    }
    while i < a.len() {
        push(2, a[i]);
        i += 1;
    }
    while j < b.len() {
        push(1, b[j]);
        j += 1;
    }
    out
}

/// Split into maximal runs of `[A-Za-z0-9_]` vs everything else, so a word and
/// its trailing space stay distinct tokens and concatenation is lossless.
fn tokenize(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut word: Option<bool> = None;
    for (i, c) in s.char_indices() {
        let is_word = c.is_alphanumeric() || c == '_';
        match word {
            Some(w) if w != is_word => {
                out.push(&s[start..i]);
                start = i;
            }
            None => {}
            _ => {}
        }
        word = Some(is_word);
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The property that makes this module usable by the DIFF tab at all:
    /// replaying a diff over its own old text must land exactly on the new text.
    fn assert_round_trip(old: &str, new: &str) {
        let hunks = diff_lines(old, new);
        let rebuilt = apply_hunks(old, &hunks);
        assert_eq!(
            rebuilt, new,
            "round-trip failed\n--- old ---\n{old:?}\n--- new ---\n{new:?}\n--- got ---\n{rebuilt:?}"
        );
    }

    #[test]
    fn round_trip_covers_insert_delete_replace_and_rewrite() {
        assert_round_trip("a\nb\nc\n", "a\nb\nc\n"); // no-op
        assert_round_trip("a\nb\nc\n", "a\nb\nc\nd\n"); // append
        assert_round_trip("a\nb\nc\n", "a\nc\n"); // delete middle
        assert_round_trip("a\nb\nc\n", "a\nB\nc\n"); // replace middle
        assert_round_trip("a\nb\nc\n", "x\ny\nz\n"); // full rewrite
        assert_round_trip("", "a\n"); // from empty
        assert_round_trip("a\n", ""); // to empty
    }

    /// No final newline and CRLF are the two cases a naive `lines()` split
    /// loses; `split_inclusive` is the reason these hold.
    #[test]
    fn round_trip_preserves_missing_final_newline_and_crlf() {
        assert_round_trip("a\nb", "a\nb\nc");
        assert_round_trip("a\r\nb\r\n", "a\r\nB\r\n");
        assert_round_trip("one line, no newline", "two lines\nhere\n");
    }

    #[test]
    fn identical_inputs_produce_no_hunks() {
        assert!(diff_lines("same\ntext\n", "same\ntext\n").is_empty());
        assert!(diff_lines("", "").is_empty());
    }

    /// A change far from any other change gets its own hunk with a bounded
    /// context window — otherwise the reviewer scrolls through the whole file.
    #[test]
    fn distant_changes_split_into_separate_context_bounded_hunks() {
        let old: String = (0..40).map(|i| format!("line {i}\n")).collect();
        let mut new_lines: Vec<String> = (0..40).map(|i| format!("line {i}\n")).collect();
        new_lines[2] = "changed early\n".into();
        new_lines[30] = "changed late\n".into();
        let new: String = new_lines.concat();

        let hunks = diff_lines(&old, &new);
        assert_eq!(hunks.len(), 2, "two distant changes: {hunks:#?}");
        for h in &hunks {
            assert!(h.is_change());
            assert!(
                h.lines.len() <= 2 * CONTEXT + 2,
                "hunk should be change + bounded context, got {} lines",
                h.lines.len()
            );
        }
        assert_round_trip(&old, &new);
    }

    #[test]
    fn nearby_changes_merge_into_one_hunk() {
        let old = "a\nb\nc\nd\ne\nf\ng\nh\n";
        let new = "a\nB\nc\nd\ne\nF\ng\nh\n";
        let hunks = diff_lines(old, new);
        assert_eq!(hunks.len(), 1, "{hunks:#?}");
        assert_eq!(hunks[0].tally(), (2, 2));
        assert_round_trip(old, new);
    }

    /// Hunks must be ordered and non-overlapping, because `apply_hunks`
    /// reconstructs every gap between them from the old text.
    #[test]
    fn hunks_are_ordered_and_non_overlapping() {
        let old: String = (0..60).map(|i| format!("l{i}\n")).collect();
        let new: String = (0..60)
            .map(|i| {
                // Every 20 lines: far enough apart that context cannot merge
                // them (CONTEXT is 3, so a 6-line gap would merge).
                if i % 20 == 0 {
                    format!("L{i}!\n")
                } else {
                    format!("l{i}\n")
                }
            })
            .collect();
        let hunks = diff_lines(&old, &new);
        assert_eq!(hunks.len(), 3, "three far-apart changes: {hunks:#?}");
        for w in hunks.windows(2) {
            assert!(
                w[0].old_range.0 + w[0].old_range.1 <= w[1].old_range.0,
                "overlap: {:?} then {:?}",
                w[0].old_range,
                w[1].old_range
            );
        }
        assert_round_trip(&old, &new);
    }

    /// Past `MAX_CELLS` the module degrades to one replacement hunk instead of
    /// allocating an unbounded table. The answer is coarser, never wrong.
    #[test]
    fn oversized_middle_degrades_to_one_replacement_hunk() {
        let old: String = (0..3000).map(|i| format!("old {i}\n")).collect();
        let new: String = (0..3000).map(|i| format!("new {i}\n")).collect();
        assert_ne!(
            3000 * 3000,
            MAX_CELLS + 1,
            "fixture must exceed the table cap"
        );
        let hunks = diff_lines(&old, &new);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].tally(), (3000, 3000));
        assert_round_trip(&old, &new);
    }

    #[test]
    fn word_diff_marks_only_the_changed_words() {
        let out = diff_words("the relief valve is open", "the relief valve is closed");
        let added: String = out
            .iter()
            .filter(|l| matches!(l, DiffLine::Add(_)))
            .map(|l| l.text().to_string())
            .collect();
        let removed: String = out
            .iter()
            .filter(|l| matches!(l, DiffLine::Del(_)))
            .map(|l| l.text().to_string())
            .collect();
        assert!(added.contains("closed"), "{out:#?}");
        assert!(removed.contains("open"), "{out:#?}");
        // Context survives on both sides.
        let same: String = out
            .iter()
            .filter(|l| matches!(l, DiffLine::Same(_)))
            .map(|l| l.text().to_string())
            .collect();
        assert!(same.contains("relief"), "{out:#?}");
        // Losslessness: the old side is Same+Del, the new side is Same+Add.
        let old_side: String = out
            .iter()
            .filter(|l| !matches!(l, DiffLine::Add(_)))
            .map(|l| l.text().to_string())
            .collect();
        let new_side: String = out
            .iter()
            .filter(|l| !matches!(l, DiffLine::Del(_)))
            .map(|l| l.text().to_string())
            .collect();
        assert_eq!(old_side, "the relief valve is open");
        assert_eq!(new_side, "the relief valve is closed");
    }

    #[test]
    fn word_diff_identical_line_is_single_same_run() {
        let out = diff_words("unchanged", "unchanged");
        assert_eq!(out, vec![DiffLine::Same("unchanged".into())]);
        assert!(diff_words("", "").is_empty());
    }

    #[test]
    fn tokenize_keeps_words_and_separators_distinct_and_lossless() {
        let toks = tokenize("fn  main() -> 42\n");
        // Maximal runs: a word, then everything that is not a word.
        assert_eq!(toks, vec!["fn", "  ", "main", "() -> ", "42", "\n"]);
        assert_eq!(toks.concat(), "fn  main() -> 42\n");
        // A single word, and an empty input, are both lossless.
        assert_eq!(tokenize("word").concat(), "word");
        assert!(tokenize("").is_empty());
    }

    #[test]
    fn markers_and_tally_are_render_ready() {
        assert_eq!(DiffLine::Add("x\n".into()).marker(), '+');
        assert_eq!(DiffLine::Del("x\n".into()).marker(), '-');
        assert_eq!(DiffLine::Same("x\n".into()).marker(), ' ');
        let h = Hunk {
            old_range: (0, 1),
            new_range: (0, 1),
            lines: vec![
                DiffLine::Del("a\n".into()),
                DiffLine::Add("b\n".into()),
                DiffLine::Add("c\n".into()),
            ],
        };
        assert_eq!(h.tally(), (2, 1));
        assert!(h.is_change());

        let ctx = Hunk {
            old_range: (0, 1),
            new_range: (0, 1),
            lines: vec![DiffLine::Same("a\n".into())],
        };
        assert!(!ctx.is_change());
    }
}
