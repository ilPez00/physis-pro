// provenance: moved from physis-core src/act.rs (core HEAD d65858b)
// as part of the Core/Product split, 2026-09-14. One implementation; no fork.
//! Acting, recorded — and checked against what is already believed.
//!
//! ## The gap this closes
//!
//! The survey in `computer-remake-research/` found that acting and remembering
//! are never the same system. Systems that **act** (open-interpreter, ragfs
//! `.ops/`) accumulate no belief; systems that **remember** (recall,
//! localsearch, ActivityWatch) cannot act. ragfs comes closest — it acts through
//! `.ops/` and audits through `.safety/` — but its memory is an index, not a
//! state of belief.
//!
//! The consequence, stated in `gaps.md` as Gap 8:
//!
//! > **No system in the survey can notice that an action it took contradicted
//! > something it already held.**
//!
//! That is the specific failure the brief calls *catching agentic mistakes*, and
//! it requires action and belief to live in one substrate.
//!
//! ## What this does, and what it deliberately does not
//!
//! It is **not** a sandbox and not a permission system. `sh -c` already runs
//! commands; the contribution here is that running one:
//!
//! 1. records the intent as an observation *before* the command runs, so an
//!    action that hangs or kills the process is still in the record;
//! 2. records the outcome with its real duration;
//! 3. **recalls what is already believed about this** and puts it in front of
//!    the operator — especially claims already `Contradicted`, which is the
//!    machine saying *you established this does not work*.
//!
//! Step 3 is the whole point. Steps 1 and 2 are bookkeeping that make it
//! possible.
//!
//! ## Honest limits
//!
//! Relevance is cosine over the command text against claim statements. That is
//! a weak matcher and it is the same lexical-versus-semantic caveat as
//! everywhere else in this crate: under the random-projection fallback it
//! matches strings, not meaning. It surfaces candidates for a person to read; it
//! does not decide anything, and it will miss a contradiction phrased
//! differently from the command that triggers it.
//!
//! **How weak, measured.** The recall of this function has now been measured
//! directly — `act_recall.rs`, `physis-core act-recall`, artifact in
//! `benchmarks/results/act-recall.json`. Eight commands against a 48-claim
//! ledger of one age, each with exactly one claim that refutes it, scored
//! against two construction-matched nulls (k random claims; k random
//! *contradicted* claims):
//!
//! | embedder | arm | recall@5 | MRR@5 |
//! |---|---|---|---|
//! | bge-base-en-v1.5 | command shares the claim's words | 1.000 | 1.000 |
//! | bge-base-en-v1.5 | command paraphrases it | 0.875 | 0.583 |
//! | random-projection | command shares the claim's words | 0.750 | 0.667 |
//! | random-projection | command paraphrases it | **0.000** | 0.000 |
//!
//! So the limit stated above is real but it is **a property of the embedder,
//! not of this function**. On a semantic embedder the paraphrase case is found
//! seven times in eight and the cost is rank, not presence: the warning arrives
//! mid-list (MRR 0.583) rather than first, and at `--top 1` recall falls to
//! 0.375. On the random-projection fallback — the supported offline mode — the
//! paraphrase case is found *never*, and scores below both nulls.
//!
//! The earlier reading here, taken from the 2×2 of 2026-09-13
//! (`docs/plans/2026-09-13-2x2-first-run.md`, 0–1/7 against a ceiling of 5/7),
//! said this function does not find the contradiction it exists to surface in
//! the normal case. That does not survive the direct measurement and is
//! withdrawn: the 2×2 scored a different task on a different corpus.
//!
//! **The polarity result, which is worse than the recall result.** A second
//! ledger adds every target's *affirmed twin* — same subject, same words,
//! opposite verdict — and asks which of the pair ranks first. No topic
//! separates them, so the null is 0.500 by construction:
//!
//! | embedder | arm | refutation ranked first | reassured at `--top 1` |
//! |---|---|---|---|
//! | bge-base-en-v1.5 | shares the claim's words | **0.250** | 5 of 8 |
//! | bge-base-en-v1.5 | paraphrase | **0.125** | 3 of 8 |
//! | random-projection | shares the claim's words | 0.750 | 0 of 8 |
//! | random-projection | paraphrase | 0.125 | 0 of 8 |
//!
//! On the semantic embedder this is not a coin flip, it is **inverted**: the
//! endorsement outranks the refutation in six of eight pairs, and in seven of
//! eight when the command is paraphrased. `is_warning` reads `status`, so the
//! wrong one is surfaced wearing `Supported` — a reassurance about the exact
//! thing a `Contradicted` claim refutes.
//!
//! At `--top 5` the list rescues it: both claims are shown in almost every
//! pair, and a person reading five lines sees the refutation. At `--top 1` it
//! does not: in five of eight lexical pairs the only claim printed is the
//! endorsement. **That is the argument for the default being a list.** Do not
//! lower `--top` to 1 on the assumption that the best match is the right one.
//!
//! The random-projection column inverts the ordering of the whole table and is
//! the reason it is printed: the lexical hash keys on `fails`, `never` and `no`
//! — the tokens the semantic space smooths away — so it reads the verdict
//! better and the topic far worse (paraphrase recall 0.000). Neither embedder
//! dominates, and the hybrid that would is untested. Eight pairs, so 0.750 is
//! two pairs above chance and should not be leaned on; the semantic inversion
//! is the robust half, same sign on both arms at every `top`.
//!
//! The structural point stands and is unaffected by the number: `RESCOPE.md` §0
//! settles that the context compiler is the *commodity* half and the ledger is
//! the differentiator — and this, the sharpest thing the ledger does, reaches
//! its data through the compiler. Recorded as conceptual problem 1 in
//! `docs/plans/2026-09-13-four-conceptual-problems.md`. What the measurement
//! adds is which half of the dependency is load-bearing: **offline, Gap 8 is
//! open.**

use physis_core::embed::VectorEmbed;
use serde::{Deserialize, Serialize};

/// Something already believed that bears on what is about to be done.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bearing {
    pub id: String,
    pub status: String,
    pub statement: String,
    pub relevance: f32,
    /// Unresolved predictions riding on this claim.
    pub open_predictions: usize,
}

impl Bearing {
    /// Does this deserve to stop someone?
    ///
    /// A `Contradicted` claim is the machine saying *this was tried and it did
    /// not work*. An unresolved prediction is a promise that was never scored.
    /// Both are worth reading before acting; a merely-related Candidate is not.
    pub fn is_warning(&self) -> bool {
        self.status == "Contradicted" || self.status == "Failed" || self.open_predictions > 0
    }
}

/// What happened, and what was already known about it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Acted {
    pub command: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub stdout_lines: usize,
    pub stderr_tail: String,
    /// Claims that bear on this command, most relevant first.
    pub bearing: Vec<Bearing>,
    /// Sequence numbers of the observations this produced.
    pub observed: Vec<u64>,
}

impl Acted {
    pub fn warnings(&self) -> Vec<&Bearing> {
        self.bearing.iter().filter(|b| b.is_warning()).collect()
    }

    pub fn render(&self) -> String {
        let mut o = String::new();
        let w = self.warnings();
        if !w.is_empty() {
            o.push_str("── ALREADY ESTABLISHED ──\n");
            for b in &w {
                o.push_str(&format!(
                    "  [{}] {:<13} rel {:.3}{}\n      {}\n",
                    b.id,
                    b.status,
                    b.relevance,
                    if b.open_predictions > 0 {
                        format!("   {} unscored", b.open_predictions)
                    } else {
                        String::new()
                    },
                    b.statement.chars().take(88).collect::<String>()
                ));
            }
            o.push('\n');
        }
        o.push_str(&format!(
            "── RAN ──\n{}\nexit {} · {:.1}s · {} line(s) out\n",
            self.command,
            self.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "signal".into()),
            self.duration_ms as f64 / 1000.0,
            self.stdout_lines
        ));
        if !self.stderr_tail.trim().is_empty() {
            o.push_str(&format!("stderr: {}\n", self.stderr_tail.trim()));
        }
        o.push_str(&format!(
            "\nrecorded as observation(s) {}\n",
            self.observed
                .iter()
                .map(|s| format!("#{s}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        o
    }
}

/// Claims bearing on `command`, most relevant first.
pub fn bearing_on(
    core: &physis_core::core::PhysisCore,
    command: &str,
    embedder: &dyn VectorEmbed,
    top: usize,
) -> Vec<Bearing> {
    bearing_on_with(core, command, embedder, top, Selection::Relevance)
}

/// How the list is cut down to `top`.
///
/// ## Why this is a choice and not a constant
///
/// Truncating on relevance alone decides who survives *before* anyone looks at
/// status — and the only thing the consumer prints is warnings. The polarity
/// arm of `act_recall` measured what that costs: with a claim's affirmed twin
/// in the ledger, the endorsement outranks the refutation in six of eight
/// pairs, so at `--top 1` the surviving claim is a `Supported` reassurance
/// about the exact thing a `Contradicted` claim refutes, five times in eight.
///
/// [`Selection::WarningReserve`] keeps one slot for the most relevant
/// warning-eligible claim. That is not free — a ledger holding any refuted
/// claim would then always surface one — so the reserve is gated on a floor:
/// the warning must score at least `floor_ratio` of the top claim's relevance
/// to take the slot. The floor trades *reassured* against *false alarms*, and
/// `act_recall` measures both arms of that trade rather than assuming a value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Selection {
    /// Cut on relevance. The original behaviour, and what every number
    /// recorded before 2026-09-13 was measured on.
    Relevance,
    /// Keep one slot for the best warning-eligible claim, if it clears the
    /// floor. `floor_ratio` is a fraction of the top-ranked claim's relevance;
    /// 0.0 always reserves, 1.0 never does unless the warning already leads.
    WarningReserve { floor_ratio: f32 },
    /// Rank on `alpha · cosine + (1 − alpha) · BM25`, both min-max normalised
    /// over the candidate set.
    ///
    /// The two legs were measured failing in **opposite directions** on the
    /// same eight pairs: the semantic embedder finds the topic (paraphrase
    /// recall 0.875) and cannot read the verdict (polarity 0.250); the lexical
    /// hash reads the verdict (0.750) and cannot find the topic (0.000). This
    /// is the knob between them. `alpha = 1.0` is [`Selection::Relevance`] by
    /// another name and is the sweep's control.
    ///
    /// Normalisation is min-max **over the candidates of this query**, not
    /// global: BM25 and cosine live on unrelated scales, and a fixed conversion
    /// tuned on one embedder is meaningless on the next. The cost is that
    /// `relevance` on the returned [`Bearing`] is then a fused score, not a
    /// cosine — read it as a rank key and not as a similarity.
    Hybrid { alpha: f32 },
    /// Reciprocal rank fusion of the cosine and BM25 orders — the crate's own
    /// hybrid, [`physis_core::rag::fuse_rrf`], which has existed in `rag.rs` since G5
    /// and which nothing in the ledger has ever called. Rank-based, so it needs
    /// no normalisation and has no weight to tune; `k` damps the head.
    HybridRrf { k: f32 },
    /// The cascade, plus a floor: claims scoring below `floor` of the leader's
    /// cosine are dropped before anything is shown.
    ///
    /// `act` has never had a floor. It takes the top five by cosine and prints
    /// whichever are refuted, however unrelated — measured at **4 of 8**
    /// commands with nothing to warn about getting a warning anyway. On a real
    /// ledger that rises, because there is always something at rank five. It is
    /// the failure the survey attributes to Nepomuk and to every semantic
    /// desktop that died: the system is *felt*, constantly, for nothing.
    ///
    /// The floor is **absolute cosine**, and that is the whole finding. A
    /// leader-relative floor — the rule [`Selection::WarningReserve`] uses, and
    /// the first thing tried here — does nothing at any usable setting, because
    /// the leader is high for both classes of command. The two classes separate
    /// on the *absolute* score and not on the ratio:
    ///
    /// | bge-base-en-v1.5 | best claim's cosine |
    /// |---|---|
    /// | commands with a refutation in the ledger | min **0.717**, median 0.798 |
    /// | commands with nothing to warn about | median 0.625, max **0.643** |
    ///
    /// Disjoint, with 0.074 between them. Measured by `examples/floor_spread`.
    ///
    /// The price is that the number is embedder-specific and must be
    /// re-measured per embedder — on the random-projection fallback the same
    /// two distributions overlap completely (0.705–0.902 against 0.609–0.824)
    /// and **no floor exists at all**, which is one more thing the hash cannot
    /// do.
    CascadeFloor { pool: usize, floor: f32 },
    /// Two stages: cosine picks a pool of `pool` candidates, BM25 orders that
    /// pool, and the top of the ordered pool is returned.
    ///
    /// This exists because [`Selection::Hybrid`] failed, and failed
    /// informatively. Mixing the two scores linearly cannot use them: at a
    /// weight high enough to keep topic recall, the lexical term is too small
    /// to reorder a pair, and at a weight low enough to reorder it, recall has
    /// already collapsed (0.875 → 0.125 at `alpha = 0`). One score cannot do
    /// two jobs.
    ///
    /// Retrieval and ranking are different problems — the finding of arXiv
    /// 2609.01556, whose whole axis is items that are *retrieved but not
    /// ranked*. So: retrieve on the leg that finds the topic, rank on the leg
    /// that reads the verdict. `pool` is how much room the second stage gets.
    CascadeRerank { pool: usize },
}

/// Claims bearing on `command`, under an explicit selection policy.
pub fn bearing_on_with(
    core: &physis_core::core::PhysisCore,
    command: &str,
    embedder: &dyn VectorEmbed,
    top: usize,
    selection: Selection,
) -> Vec<Bearing> {
    let q = embedder.embed(command);
    let mut v: Vec<Bearing> = core
        .hypotheses
        .values()
        .map(|h| Bearing {
            id: h.id.chars().take(8).collect(),
            status: format!("{:?}", h.status),
            statement: h.statement.clone(),
            relevance: physis_core::models::cosine_sim(&q, &h.embedding),
            open_predictions: h.predictions.iter().filter(|p| p.observed_at.is_none()).count(),
        })
        .filter(|b| b.relevance.is_finite())
        .collect();
    // Tie-break on the statement, not the id. `id` is a UUID prefix, freshly
    // random per claim, so two equally relevant claims came back in a
    // different order in every process — `act` was not reproducible for ties,
    // and the benchmark's blind-ranker test flaked on exactly that. Statements
    // are the corpus, so this order is a property of the data.
    v.sort_by(|a, b| {
        b.relevance
            .partial_cmp(&a.relevance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.statement.cmp(&b.statement))
    });

    let floor_ratio = match selection {
        Selection::Relevance => {
            v.truncate(top);
            return v;
        }
        Selection::Hybrid { alpha } => {
            refuse_or_fuse(&mut v, command, alpha);
            v.truncate(top);
            return v;
        }
        Selection::HybridRrf { k } => {
            fuse_by_rank(&mut v, command, k);
            v.truncate(top);
            return v;
        }
        Selection::CascadeFloor { pool, floor } => {
            // The floor is applied to the COSINE score, before the rerank
            // rewrites `relevance` into a fused one. Filtering on the fused
            // score would filter on a number whose scale is an artefact of
            // this query's candidate set.
            v.retain(|b| b.relevance >= floor);
            v.truncate(pool.max(top));
            refuse_or_fuse(&mut v, command, 0.0);
            v.truncate(top);
            return v;
        }
        Selection::CascadeRerank { pool } => {
            // Stage one is the cosine order `v` is already in.
            v.truncate(pool.max(top));
            // Stage two orders the pool lexically. alpha = 0 is BM25 alone,
            // and it is applied to the pool rather than to the ledger, which
            // is the entire difference from `Hybrid { alpha: 0.0 }`.
            refuse_or_fuse(&mut v, command, 0.0);
            v.truncate(top);
            return v;
        }
        Selection::WarningReserve { floor_ratio } => floor_ratio,
    };
    if top == 0 || v.is_empty() {
        v.truncate(top);
        return v;
    }

    // The best warning-eligible claim, wherever it sits. If it already made the
    // cut there is nothing to reserve.
    let Some(w) = v.iter().position(|b| b.is_warning()) else {
        v.truncate(top);
        return v;
    };
    if w < top {
        v.truncate(top);
        return v;
    }
    // The floor is relative to the leader, not absolute: cosine scales differ
    // per embedder, and an absolute threshold tuned on one is meaningless on
    // the next. A non-positive leader carries no scale, so no reserve is made.
    let lead = v[0].relevance;
    // `lead.is_finite() && lead > 0.0` rather than a negated comparison: a NaN
    // leader carries no scale, and the floor must refuse rather than fire.
    if !(lead.is_finite() && lead > 0.0) || v[w].relevance < lead * floor_ratio {
        v.truncate(top);
        return v;
    }
    // Promote it into the last slot, and keep the list in relevance order so
    // the operator still reads it as a ranking.
    let warning = v.remove(w);
    v.truncate(top.saturating_sub(1));
    v.push(warning);
    v.sort_by(|a, b| {
        b.relevance
            .partial_cmp(&a.relevance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.statement.cmp(&b.statement))
    });
    v
}

/// Run `command`, recording intent before and outcome after.
///
/// The intent observation is written first and on purpose: a command that hangs,
/// is killed, or takes the machine down still leaves a record that it was
/// attempted. A log that only records completions cannot explain a crash.
pub fn run(
    command: &str,
    bearing: Vec<Bearing>,
    log: &std::path::Path,
) -> anyhow::Result<Acted> {
    let mut intent = physis_core::observe::Observation::new("action", command)
        .with_body("intent")
        .by("act");
    let (first, _) = physis_core::observe::append(log, std::slice::from_mut(&mut intent))?;

    let started = std::time::Instant::now();
    let out = std::process::Command::new("sh").arg("-c").arg(command).output()?;
    let duration_ms = started.elapsed().as_millis() as u64;

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let stderr_tail: String = stderr.lines().rev().take(3).collect::<Vec<_>>().join(" | ");

    let mut outcome = physis_core::observe::Observation::new("action", command)
        .with_body(format!(
            "exit {} · {} out / {} err line(s){}",
            out.status.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".into()),
            stdout.lines().count(),
            stderr.lines().count(),
            if stderr_tail.is_empty() { String::new() } else { format!(" · {stderr_tail}") }
        ))
        .by("act");
    outcome.duration_ms = Some(duration_ms);
    let (second, _) = physis_core::observe::append(log, std::slice::from_mut(&mut outcome))?;

    Ok(Acted {
        command: command.to_string(),
        exit_code: out.status.code(),
        duration_ms,
        stdout_lines: stdout.lines().count(),
        stderr_tail,
        bearing,
        observed: vec![first, second],
    })
}

/// Min-max normalise a slice in place, mapping a flat input to all-zero rather
/// than to a division by zero. A leg with no spread contributes nothing, which
/// is the correct reading: it ranked nothing.
fn minmax(xs: &mut [f32]) {
    let (lo, hi) = xs.iter().fold((f32::MAX, f32::MIN), |(l, h), &x| (l.min(x), h.max(x)));
    let span = hi - lo;
    if !(span.is_finite() && span > 0.0) {
        xs.iter_mut().for_each(|x| *x = 0.0);
        return;
    }
    xs.iter_mut().for_each(|x| *x = (*x - lo) / span);
}

/// Re-score `v` as `alpha · cosine + (1 − alpha) · BM25`, both normalised over
/// this candidate set, and re-sort. `v` must already be cosine-scored.
fn refuse_or_fuse(v: &mut [Bearing], command: &str, alpha: f32) {
    let alpha = alpha.clamp(0.0, 1.0);
    let texts: Vec<String> = v.iter().map(|b| b.statement.clone()).collect();
    let bm = physis_core::rag::Bm25Index::build(&texts);
    let terms = physis_core::rag::bm25_terms(command);
    let mut lex: Vec<f32> = (0..texts.len()).map(|i| bm.score(i, &terms)).collect();
    let mut sem: Vec<f32> = v.iter().map(|b| b.relevance).collect();
    minmax(&mut lex);
    minmax(&mut sem);
    for (i, b) in v.iter_mut().enumerate() {
        b.relevance = alpha * sem[i] + (1.0 - alpha) * lex[i];
    }
    v.sort_by(|a, b| {
        b.relevance
            .partial_cmp(&a.relevance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.statement.cmp(&b.statement))
    });
}

/// Re-order `v` by reciprocal rank fusion of its cosine order and its BM25
/// order, using the crate's existing [`physis_core::rag::fuse_rrf`].
fn fuse_by_rank(v: &mut Vec<Bearing>, command: &str, k: f32) {
    let texts: Vec<String> = v.iter().map(|b| b.statement.clone()).collect();
    let bm = physis_core::rag::Bm25Index::build(&texts);
    let terms = physis_core::rag::bm25_terms(command);
    // `v` is already in cosine order, so its indices are its cosine ranks.
    let cos_order: Vec<usize> = (0..v.len()).collect();
    let bm_order: Vec<usize> = bm.rank(&terms).into_iter().map(|(i, _)| i).collect();
    let fused = physis_core::rag::fuse_rrf(&[&cos_order, &bm_order], k);
    let mut out: Vec<Bearing> = Vec::with_capacity(v.len());
    for (idx, score) in fused {
        if let Some(b) = v.get(idx) {
            let mut b = b.clone();
            b.relevance = score;
            out.push(b);
        }
    }
    *v = out;
}

#[cfg(test)]
mod tests {
    use super::*;
    use physis_core::embed::RandomProjectionEmbedder;
    use physis_core::hypothesis::{Evidence, Hypothesis};

    fn tmp(n: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("physis-act-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("o.jsonl")
    }

    /// An action that hangs or kills the process must still leave a record that
    /// it was attempted. A log of completions only cannot explain a crash.
    #[test]
    fn intent_is_recorded_before_the_command_runs() {
        let log = tmp("intent");
        let a = run("true", vec![], &log).unwrap();
        let all = physis_core::observe::read(&log).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].body, "intent", "intent must be written first");
        assert!(all[1].body.starts_with("exit 0"));
        assert!(all[1].duration_ms.is_some(), "the outcome carries the runtime");
        assert_eq!(a.observed, vec![1, 2]);
    }

    /// The point of the module: a claim already refuted must be put in front of
    /// the operator, and a merely-related candidate must not.
    #[test]
    fn a_refuted_claim_is_a_warning_and_a_candidate_is_not() {
        let refuted = Bearing {
            id: "aaaaaaaa".into(),
            status: "Contradicted".into(),
            statement: "this approach fails".into(),
            relevance: 0.9,
            open_predictions: 0,
        };
        let candidate = Bearing {
            id: "bbbbbbbb".into(),
            status: "Candidate".into(),
            statement: "might work".into(),
            relevance: 0.9,
            open_predictions: 0,
        };
        let unscored = Bearing { open_predictions: 1, ..candidate.clone() };
        assert!(refuted.is_warning());
        assert!(!candidate.is_warning(), "relatedness alone must not interrupt");
        assert!(unscored.is_warning(), "an unkept promise is worth reading");
    }

    /// Relevance must actually rank: a claim about the command's subject should
    /// outrank an unrelated one.
    #[test]
    fn bearing_ranks_by_relevance() {
        let e = RandomProjectionEmbedder::new(128);
        let mut core = physis_core::core::PhysisCore::new();
        for text in ["cargo test fails on this crate", "the kitchen needs painting"] {
            let mut h = Hypothesis::new(text, e.embed(text));
            h.add_supporting_evidence(Evidence::supports("x", "y"));
            core.hypotheses.insert(h.id.clone(), h);
        }
        let b = bearing_on(&core, "cargo test", &e, 2);
        assert_eq!(b.len(), 2);
        assert!(
            b[0].statement.contains("cargo"),
            "the related claim must rank first, got {:?}",
            b[0].statement
        );
    }

    /// Warnings are a filter over bearing, not a separate list that can drift.
    #[test]
    fn warnings_are_a_subset_of_bearing() {
        let a = Acted {
            command: "x".into(),
            exit_code: Some(0),
            duration_ms: 1,
            stdout_lines: 0,
            stderr_tail: String::new(),
            bearing: vec![
                Bearing { id: "a".into(), status: "Contradicted".into(), statement: "s".into(), relevance: 0.5, open_predictions: 0 },
                Bearing { id: "b".into(), status: "Supported".into(), statement: "t".into(), relevance: 0.4, open_predictions: 0 },
            ],
            observed: vec![1, 2],
        };
        assert_eq!(a.warnings().len(), 1);
        assert!(a.render().contains("ALREADY ESTABLISHED"));
        assert!(a.render().contains("recorded as observation(s) #1, #2"));
    }
}
