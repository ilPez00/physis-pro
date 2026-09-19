// provenance: moved from physis-core src/act_recall.rs (core HEAD d65858b)
// as part of the Core/Product split, 2026-09-14. One implementation; no fork.
//! Does `act` actually find the contradiction it exists to surface?
//!
//! ## Why this module exists
//!
//! `act.rs` is the only implementation, in a twenty-one system survey, of
//! *acting while checking what is already believed* (`gaps.md` Gap 8). It
//! reaches that belief through one line — cosine over the command text against
//! claim statements — and that retrieval family scored **0–1/7 against a
//! whole-file ceiling of 5/7** in the 2×2 of 2026-09-13. So the differentiator
//! routes through the commodity, and the commodity was measured weak.
//!
//! Recorded as conceptual problem 1 in
//! `docs/plans/2026-09-13-four-conceptual-problems.md`, which asks for exactly
//! one thing: **measure `act`'s recall directly, against a construction-matched
//! null.** That is this module. Nothing here is an improvement to `act`; it is
//! the number that says whether an improvement is needed and, later, whether
//! one worked.
//!
//! ## The construction
//!
//! A seeded synthetic ledger of [`LEDGER_SIZE`] claims is built in one pass, so
//! every claim is the same age and age cannot be the thing that ranks. Eight of
//! them are `Contradicted` and each is the *one* claim a given command
//! contradicts; eight more are `Contradicted` and bear on nothing being run;
//! the rest are candidates and support. Ground truth is by construction — the
//! pairing is authored with the corpus, not judged afterwards.
//!
//! Two arms, because the failure mode is specific and an average over both
//! would hide it:
//!
//! | arm | the command | what it tests |
//! |---|---|---|
//! | `lexical` | shares the claim's own words (`cargo build --release`) | the best case a string matcher can have |
//! | `paraphrase` | same act, no shared content words (`produce an optimised binary for shipping`) | the normal case — `act.rs`'s own stated limit |
//!
//! ## The nulls
//!
//! Two, both matched to the same `top` and drawn from the same one-age pool, so
//! the only difference between arm and null is the ranking:
//!
//! - **uniform** — surface `top` claims at random from the whole ledger. This
//!   is the null the plan names.
//! - **status** — surface `top` claims at random *from the contradicted claims
//!   only*. This is the harder and fairer one: `Bearing::is_warning` already
//!   knows a claim's status, so a system that ignored the command entirely and
//!   listed refuted claims would score this without doing any retrieval. Beating
//!   uniform and losing to status means the cosine leg contributes nothing that
//!   a status filter does not already give for free.
//!
//! A margin must clear one query's worth of recall (`1/queries`) to count, so a
//! single lucky pairing cannot produce a verdict.
//!
//! ## What it measured, 2026-09-13
//!
//! `recall@top` over 8 commands, ledger of 48, bge-base-en-v1.5 (`onnx-explicit`):
//!
//! | arm | top=1 | top=3 | top=5 | MRR@5 | null unif@5 | null status@5 |
//! |---|---|---|---|---|---|---|
//! | `lexical` | 1.000 | 1.000 | **1.000** | 1.000 | 0.103 | 0.318 |
//! | `paraphrase` | 0.375 | 0.875 | **0.875** | 0.583 | 0.103 | 0.318 |
//!
//! Both arms clear both nulls at every `top`. **The pessimistic reading of the
//! 2×2 does not survive a direct measurement:** on a semantic embedder `act`
//! does find the contradicting claim, and on the list the CLI actually shows it
//! finds seven of eight even when the command shares no word with the claim.
//! What degrades is *rank*, not presence — MRR 1.000 → 0.583 — so on a
//! paraphrase the warning arrives mid-list rather than first, and at `top=1` the
//! paraphrase arm drops to 0.375.
//!
//! The same run on the random-projection fallback:
//!
//! | arm | recall@5 | MRR@5 | verdict |
//! |---|---|---|---|
//! | `lexical` | 0.750 | 0.667 | HOLDS |
//! | `paraphrase` | **0.000** | 0.000 | FAILS |
//!
//! That is the sharper finding. Offline — the mode this crate advertises as
//! supported, not as a failure — `act` surfaces the contradiction **never** when
//! the command is phrased differently from the claim, and scores below both
//! nulls. Gap 8 is closed on a semantic embedder and open on the fallback, and
//! nothing in the CLI says which one is running when `act` prints nothing.
//!
//! ## The polarity arm, and the prediction recorded before it ran
//!
//! Every distractor above differs from its target by **topic**. None differs by
//! **polarity**, so the two arms measure the easy discrimination and say nothing
//! about the hard one: of two claims about this command, tell the refutation
//! from the endorsement. [`AFFIRMED_TWINS`] adds each target's endorsement to a
//! second ledger and asks which of the pair ranks first. The null is **0.500 by
//! construction** — no topic separates them.
//!
//! Measured elsewhere and the reason for this arm: dense retrievers rank
//! "treatment works" and "treatment does not work" alike (arXiv 2603.17580,
//! negation is syntactic, the embedding space is not), and in 95.2–99.8% of
//! structural-retrieval misses the false positive is more lexically similar to
//! the query than the gold item is (arXiv 2609.01556).
//!
//! **Predicted, before the arm was run** (recorded in commit d415488, so the
//! record is checkable): recall stays near 0.875 while discrimination lands
//! *below* 0.500 — the endorsement outranking the refutation in most pairs.
//!
//! ### What it did, and it is worse than the prediction
//!
//! bge-base-en-v1.5, 56-claim ledger, `discrim` = pairs where the refutation
//! outranked its endorsement, null 0.500:
//!
//! | arm | discrim | target@5 | twin@5 | reassured@5 | reassured@1 |
//! |---|---|---|---|---|---|
//! | `lexical` | **0.250** | 8/8 | 8/8 | 0/8 | 5/8 |
//! | `paraphrase` | **0.125** | 6/8 | 8/8 | 2/8 | 3/8 |
//!
//! Not a coin flip — **inverted**. The endorsement outranks the refutation in
//! six of eight pairs on the lexical arm and seven of eight on the paraphrase
//! arm. The prediction was that polarity would be invisible; it is worse than
//! invisible, because the endorsement is systematically the *better* match for
//! a command that asks to do the thing.
//!
//! `reassured` is the cell that matters: the endorsement made the list and the
//! refutation did not. At `top 5` that is 0 and 2 of 8 — the operator still
//! sees both, so the list saves it. At `top 1` it is **5 of 8 and 3 of 8**: in
//! the majority of lexical pairs, `act --top 1` prints a Supported claim about
//! the exact thing a Contradicted claim refutes. That is not a miss. It is the
//! wrong answer delivered with the ledger's authority, and it is the strongest
//! argument in this crate for showing a list rather than a best match.
//!
//! ### And the two embedders fail in opposite directions
//!
//! The same arm on the random-projection fallback:
//!
//! | arm | discrim | verdict |
//! |---|---|---|
//! | `lexical` | **0.750** | HOLDS |
//! | `paraphrase` | 0.125 | INVERTED |
//!
//! The lexical hash *beats* the semantic embedder on polarity, 0.750 against
//! 0.250, because it keys on the literal tokens — `fails`, `never`, `no` — that
//! the semantic space smooths away. That is arXiv 2603.17580's finding
//! reproduced from the other side: negation is syntactic, so the model that
//! cannot read meaning is the one that can still see the "not".
//!
//! Neither embedder dominates. The semantic one finds the right *topic* and
//! cannot tell the verdict; the hash can tell the verdict and cannot find the
//! topic (paraphrase recall 0.000). Read as an instruction, that is an argument
//! for a hybrid, and it is untested.
//!
//! **Do not over-read 0.750.** Eight pairs means one flip is worth 0.125, so
//! that margin is two pairs. The inversion on the semantic embedder is the
//! robust half of this result: it is the same sign on both arms, at every
//! `top`, and it is what the literature predicts.
//!
//! ## The selection sweep — the fix that does not pay
//!
//! The polarity result argues for reserving a slot for the best
//! warning-eligible claim ([`crate::ported::act::Selection::WarningReserve`]). On the
//! benefit alone that looks free, and it is a tautology: the target *is* the
//! most relevant contradicted claim, so reserving a slot for it removes the
//! harm by construction. [`UNRELATED_COMMANDS`] is the cost arm — eight
//! commands nothing refutes, where every warning is a false alarm.
//!
//! Lexical arm, twin ledger, bge-base-en-v1.5:
//!
//! | top | policy | reassured | target@top | false alarms | removed/alarm |
//! |---|---|---|---|---|---|
//! | 1 | relevance | 5/8 | 2/8 | 0/8 | — |
//! | 1 | reserve@0.00–0.90 | 0/8 | 8/8 | **8/8** | 0.62 |
//! | 1 | reserve@0.95 | 0/8 | 8/8 | 5/8 | **1.00** |
//! | 1 | reserve@1.00 | 5/8 | 2/8 | 0/8 | — |
//! | 5 | relevance | 0/8 | 8/8 | **4/8** | — |
//! | 5 | reserve@0.95 | 0/8 | 8/8 | 5/8 | — |
//!
//! **The verdict is no.** At best the reserve removes one harm per false alarm
//! it creates, and below floor 0.95 it removes 0.62. At `top 5` — the default —
//! there is no harm left to remove, so the reserve is pure cost. `act` keeps
//! `Selection::Relevance` and the policy ships measured, off, and documented as
//! not worth turning on. The thing that actually fixes the polarity harm is the
//! default already being a list of five.
//!
//! `reserve@1.00` reproduces the relevance policy exactly, at both cuts. That is
//! the sweep's own control and it holds.
//!
//! ### What the cost arm found on the way
//!
//! Read the `top 5 · relevance` row again: **4 of 8** commands with nothing to
//! warn about already surface a warning today, with no reserve involved. `act`
//! has no relevance floor at all — it takes the top five by cosine and prints
//! whichever of them are refuted, however unrelated. On a ledger of any size
//! that is a warning on half of all unrelated commands, and it was never
//! measured because nothing asked what `act` does when the honest answer is
//! nothing. That is a separate defect from the polarity one, it is larger, and
//! it is not fixed here.
//!
//! ## The hybrid sweep, and the prediction recorded before it ran
//!
//! The polarity result says the two legs fail in opposite directions.
//! [`crate::ported::act::Selection::Hybrid`] is the knob between them —
//! `alpha · cosine + (1 − alpha) · BM25`, both min-max normalised over the
//! candidates — and [`ALPHAS`] sweeps it with **both metrics on every row**, so
//! a weight that buys polarity by losing the topic cannot look like a win.
//! `alpha = 1.00` is cosine only and is the control: it must reproduce the
//! recorded 0.875 paraphrase recall and 0.125 paraphrase polarity.
//!
//! The sweep also includes `rrf@60` — [`physis_core::rag::fuse_rrf`] over the cosine
//! and BM25 orders. That hybrid has existed in `rag.rs` since G5 and **the
//! ledger has never called it**, which is itself an instance of the pattern the
//! capability sweep exists to catch: the crate shipped a hybrid retriever and
//! its sharpest consumer kept using the cosine leg alone.
//!
//! **Predicted, before the sweep ran** (recorded in the commit that added it):
//! there is an interior `alpha` that beats both endpoints on `combined`,
//! because the two legs fail on *different* pairs rather than on the same ones
//! with different severity.
//!
//! ### The prediction was wrong about the mechanism and right about the legs
//!
//! | policy | recall(para) | polarity(lex) | polarity(para) | combined |
//! |---|---|---|---|---|
//! | `hybrid@0.00` (BM25 only) | 0.125 | 0.875 | 0.625 | 0.750 |
//! | `hybrid@0.50` | 0.625 | 0.375 | 0.125 | 0.750 |
//! | `hybrid@0.60`–`@1.00` | 0.875 | 0.250 | 0.125 | 1.000 |
//! | `rrf@60` | 0.625 | 0.375 | 0.125 | 0.750 |
//! | **`cascade@5`** | **0.875** | **0.875** | **0.625** | **1.500** |
//!
//! **No interior `alpha` exists.** From 0.60 to 1.00 the mixture is *identical*
//! to cosine alone — the lexical term is too small to reorder anything — and
//! below 0.60 topic recall collapses before polarity improves. The crate's own
//! `rrf@60` lands in the same dead zone. One score cannot do two jobs: at a
//! weight that keeps recall the lexical leg is inert, and at a weight that
//! reorders, recall is already gone.
//!
//! But the legs *are* complementary, which the endpoints show plainly: BM25
//! alone reads the verdict at 0.875/0.625 where cosine reads it at 0.250/0.125.
//! The mistake was mixing them into one score at all. Retrieval and ranking are
//! different problems — the entire axis of arXiv 2609.01556, whose finding is
//! items *retrieved but not ranked*.
//!
//! [`crate::ported::act::Selection::CascadeRerank`] is the two-stage form: cosine
//! retrieves the pool, BM25 orders it. At `pool = top` it is a **pure
//! reordering** — recall@5 is set membership and therefore unchanged by
//! construction, not by achievement — and polarity goes 0.250 → 0.875 and
//! 0.125 → 0.625, both far above the 0.500 null. Past `pool = top` the second
//! stage can push the target out of the returned list and recall falls
//! (0.875 → 0.625 → 0.375 at pools 10 and 20) while polarity gains nothing, so
//! the pool wants to be exactly the cut.
//!
//! Through the same cost arm as the reserve: at `top 5` every cost column is
//! identical to the cosine policy, because a reordering cannot change which
//! claims are available. At `top 1` it removes 4 of the 5 reassurances for 2
//! false alarms — **2.00 harm removed per alarm, twice the best the reserve
//! managed** — and raises `target_in_top` from 2/8 to 7/8. The CLI now uses it;
//! `act::bearing_on` keeps the cosine order as the library's frozen baseline.
//!
//! ## The relevance floor — item B, and it is absolute, not relative
//!
//! The cost arm found that **4 of 8** commands with nothing to warn about get a
//! warning today. `act` has never had a floor: it takes the top five by cosine
//! and prints whichever are refuted, however unrelated.
//!
//! The first floor tried was leader-relative, the rule
//! [`crate::ported::act::Selection::WarningReserve`] uses, and it did **nothing** at
//! 0.80, 0.90 or 0.95 — the leader is high for both classes of command, so the
//! ratio carries no signal. `examples/floor_spread` prints the number the sweep
//! could not, and the classes separate on the *absolute* score:
//!
//! | bge-base-en-v1.5 | best claim's cosine |
//! |---|---|
//! | command has a refutation | min **0.717**, median 0.798, max 0.913 |
//! | command has nothing to say | min 0.573, median 0.625, max **0.643** |
//!
//! Disjoint, 0.074 apart. Sweeping absolute floors through the same cost arm:
//!
//! | policy | reassured@1 | target@1 | alarms@1 | alarms@5 |
//! |---|---|---|---|---|
//! | `relevance` (today) | 5/8 | 2/8 | 0/8 | 4/8 |
//! | `cascade@5` | 1/8 | 7/8 | 2/8 | 4/8 |
//! | **`cascade@5+floor0.66`** | **1/8** | **7/8** | **0/8** | **0/8** |
//! | `cascade@5+floor0.70` | 2/8 | 6/8 | 0/8 | 0/8 |
//! | `cascade@5+floor0.75` | 2/8 | 5/8 | 0/8 | 0/8 |
//!
//! 0.66 is strictly better than the current default on every column, at both
//! cuts, and the two floors above it start dropping real warnings — the window
//! is exactly the measured gap. The CLI uses 0.66.
//!
//! **The price is that the number is embedder-specific.** On the
//! random-projection fallback the same two distributions overlap completely
//! (0.705–0.902 against 0.609–0.824) and **no floor exists**. The CLI applies
//! none there rather than guessing one, so offline `act` keeps its false alarms
//! — which is the honest outcome and one more entry in the list of things the
//! hash cannot do.
//!
//! ## What this measures and what it does not
//!
//! It measures whether `bearing_on` puts the *one* contradicting claim in the
//! `top` the CLI actually shows (default 5). It does not measure whether an
//! operator then reads it, and it says nothing about real ledgers, whose claims
//! are neither uniformly aged nor authored to be findable. The corpus was authored
//! so that one claim is the right answer; a real ledger has near-misses, and
//! this number is therefore a ceiling on a real one, not an estimate of it.
//!
//! Run it: `physis-core act-recall [--top N] [--json]`. Artifacts land in
//! `benchmarks/results/act-recall.json`.

use crate::ported::act::bearing_on;
use physis_core::core::PhysisCore;
use physis_core::embed::VectorEmbed;
use physis_core::hypothesis::{Evidence, Hypothesis};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

/// Claims in the synthetic ledger.
pub const LEDGER_SIZE: usize = 48;
/// Random draws averaged for each null. Enough that the null is stable to
/// ±0.01 at this ledger size, and cheap because no embedding is involved.
pub const NULL_TRIALS: usize = 2000;

/// One command, and the single claim it contradicts.
struct Pair {
    /// The command as it shares the claim's own vocabulary.
    lexical: &'static str,
    /// The same act, phrased with no content word in common with the claim.
    paraphrase: &'static str,
    /// The claim that command contradicts. Contradicted in the ledger.
    claim: &'static str,
}

/// The eight commands with a claim that already refutes them.
///
/// Authored together with the corpus: the ground truth is the pairing, and it
/// is fixed before any retrieval runs. The paraphrase column is deliberately
/// stripped of the claim's content words — that is the case `act.rs`'s "Honest
/// limits" says it will miss, and an arm that cannot fail is not an arm.
const PAIRS: [Pair; 8] = [
    Pair {
        lexical: "cargo test --all-features",
        paraphrase: "run the whole suite with every optional flag turned on",
        claim: "cargo test --all-features fails on this crate: the ort linker step never resolves",
    },
    Pair {
        lexical: "cargo build --release",
        paraphrase: "produce an optimised binary for shipping",
        claim: "the release build never finishes on this box — it runs out of memory at the LTO stage",
    },
    Pair {
        lexical: "pip install onnxruntime",
        paraphrase: "add the neural inference package to the interpreter environment",
        claim: "pip install onnxruntime fails on this Python: there is no manylinux wheel for 3.13",
    },
    Pair {
        lexical: "docker compose up -d",
        paraphrase: "bring the local service stack online in the background",
        claim: "docker compose up leaves postgres in a restart loop because the volume is owned by root",
    },
    Pair {
        lexical: "npm run build",
        paraphrase: "compile the frontend bundle for deployment",
        claim: "npm run build breaks on the CI image: node 18 cannot parse the new syntax",
    },
    Pair {
        lexical: "git push origin main",
        paraphrase: "publish the local commits to the shared upstream trunk",
        claim: "git push origin main is rejected — the branch is protected and requires a review",
    },
    Pair {
        lexical: "./scripts/migrate.sh",
        paraphrase: "apply the pending schema changes to the database",
        claim: "the migrate script drops the audit table when run twice; it is not idempotent",
    },
    Pair {
        lexical: "rsync -av ./data backup:/srv/data",
        paraphrase: "copy the dataset across to the offsite machine",
        claim: "rsync to the backup host times out over the VPN for anything larger than a gigabyte",
    },
];

/// The affirmed twin of each target in [`PAIRS`], same index.
///
/// ## Why these exist
///
/// Every distractor in the base ledger differs from its target **by topic**.
/// Not one differs by **polarity**. So the base arms measure the easy
/// discrimination — find the claim that is *about* this command — and say
/// nothing about the hard one: of two claims about this command, tell the
/// refutation from the endorsement.
///
/// That is the discrimination `act` actually needs. `Bearing::is_warning` reads
/// `status`, not the ranking, so a near-twin of the opposite polarity is not a
/// near-miss — it is a claim that will be surfaced with a *reassuring* status
/// while the refutation sits below the cut.
///
/// Measured elsewhere, 2603.17580: dense retrievers rank "treatment works" and
/// "treatment does not work" alike, because negation is syntactic and the
/// embedding space is not. Each twin here is written to keep the target's
/// vocabulary and invert only the verdict — a test asserts that overlap rather
/// than trusting the author.
const AFFIRMED_TWINS: [&str; 8] = [
    "cargo test --all-features passes on this crate: the ort linker step resolves every time",
    "the release build finishes on this box in four minutes — the LTO stage never runs out of memory",
    "pip install onnxruntime succeeds on this Python: there is a manylinux wheel for 3.13",
    "docker compose up leaves postgres healthy because the volume is owned by the right user, with no restart loop",
    "npm run build succeeds on the CI image: node 18 parses the new syntax without complaint",
    "git push origin main is accepted — the branch is not protected and requires no review",
    "the migrate script leaves the audit table alone when run twice; it is idempotent",
    "rsync to the backup host completes over the VPN for anything larger than a gigabyte",
];

/// Commands that nothing in the ledger refutes.
///
/// The cost arm. [`crate::ported::act::Selection::WarningReserve`] keeps a slot for the
/// best warning-eligible claim, so on its own it would look free: the polarity
/// arm's `reassured` cell goes to zero by construction, because the target *is*
/// the most relevant contradicted claim. That is a tautology, not a result.
///
/// These eight commands have no refutation in the ledger. Every warning
/// surfaced for them is a **false alarm** — the reserve paying out on noise —
/// and the floor is what trades one against the other. Without this arm the
/// reserve cannot fail and must not be believed.
const UNRELATED_COMMANDS: [&str; 8] = [
    "ls -la /var/log",
    "date --iso-8601=seconds",
    "whoami",
    "echo hello",
    "uname -sr",
    "df -h /",
    "hostname",
    "cat /etc/os-release",
];

/// A restatement of each target that **agrees** with it, same index.
///
/// The decisive negatives for `claim_identity`. Without them the twin ledger
/// cannot answer conceptual problem 2 at all: the eight contradiction pairs are
/// also, by construction, the eight most *similar* pairs in the corpus, so any
/// similarity threshold isolates them perfectly and cosine scores F1 1.000 for
/// a reason that has nothing to do with contradiction.
///
/// These sit at the same similarity and assert the *same* verdict. A method
/// that flags "two claims about one fact that disagree" must take the twins and
/// leave these; a method that flags "two claims about one fact" takes both and
/// its precision halves. That is the whole question, and it needs both halves
/// of the pair to be askable.
const PARAPHRASE_AGREEMENTS: [&str; 8] = [
    "running cargo test with all features on this crate does not get past the ort linker step",
    "an optimised build of this crate exhausts the memory on this box during link-time optimisation",
    "there is no prebuilt onnxruntime wheel for Python 3.13 on this platform, so the install stops",
    "the postgres container keeps restarting under compose because its data volume has the wrong owner",
    "the frontend bundle will not compile on the CI image because that node version predates the syntax",
    "pushing straight to the main branch upstream is refused until someone reviews the change",
    "running the migration a second time destroys the audit table, so it cannot be replayed safely",
    "sending the dataset to the offsite host over the VPN stalls once it passes a gigabyte",
];

/// Contradicted claims that no command in [`PAIRS`] is about.
///
/// They exist so the status null has something to be wrong about. Without them
/// every contradicted claim would be a target and "list the contradicted ones"
/// would score a perfect null, which would say more about the corpus than about
/// `act`.
const CONTRADICTED_DISTRACTORS: [&str; 8] = [
    "the nightly toolchain miscompiles the SIMD path and the results differ from stable",
    "raising the thread pool above sixteen workers makes the ingest slower, not faster",
    "the S3 lifecycle rule deleted the archived exports before anyone had read them",
    "the ARM builder cannot cross-compile the C shim; the linker picks the host libc",
    "caching the tokenizer between processes corrupts it — the offsets go stale",
    "the webhook retry policy double-charges when the receiver is slow rather than down",
    "the ONNX export loses the pooling layer, so the vectors come back unnormalised",
    "increasing the batch size past 64 exhausts the GPU before the epoch ends",
];

/// Neither refuted nor about anything being run — the bulk of a real ledger.
///
/// Thirty-two of these put the target at 1-in-48 for the uniform null, which is
/// the realistic shape: most of what is believed bears on nothing in particular.
const NEUTRAL_DISTRACTORS: [&str; 32] = [
    "the semiotic grid holds five domains and fourteen modes",
    "observations are appended, never rewritten, so the timeline replays",
    "the MMR default packed four of three thousand tokens before it was fixed",
    "a validity window can be narrowed when better evidence arrives",
    "the embedder falls back to random projection when no model is on disk",
    "the studio serves the same structural map the CLI prints",
    "n-gram tables are interchangeable with the model that reads them",
    "the ground-truth corpus carries thirty documents with known structure",
    "context compression measured thirty point six percent on the demo corpus",
    "the direction control truncates to the same length as the rewrite",
    "worst-covered retention refuses to reward copying",
    "the ledger keeps a revision every time a status changes",
    "provenance chains record which source supported which claim",
    "a claim below twenty training entries is unmeasured rather than weak",
    "the pre-flight stops when a class is too sparse to score",
    "coherence is computed over the certified branches only",
    "dream evaluation archives its results for later replay",
    "typed edges connect nodes, hypotheses and observations",
    "the tokenizer compatibility gate refuses a mismatched pair",
    "the licence gate verifies signatures even in development builds",
    "the terminal watcher and the agent watcher share one timeline",
    "structural machines carry a proof status alongside their observations",
    "the map hash is identical for identical corpora",
    "the mandelbrot document is the most different in the demo set",
    "an unscored prediction is worth reading before acting",
    "the oracle leg reports not-configured rather than inventing a number",
    "packs are dropped into the config directory rather than compiled in",
    "facets carry sub-domain and sub-mode without widening the grid",
    "the quality loop embeds failures against the grid and applies a penalty",
    "inventory forecasting uses a thirty-period moving average",
    "the web server keys session state by the tenant header",
    "voice, image and video reduce to vectors in one shared space",
];

/// The ledger the benchmark builds, for a caller that wants to probe it.
///
/// Exists so `examples/floor_spread.rs` measures the *same* corpus the
/// benchmark scores rather than a second one that drifts away from it.
pub fn demo_ledger(embedder: &dyn VectorEmbed) -> PhysisCore {
    build_ledger(embedder, true).0
}

/// The twin ledger as (statement, embedding) plus the index pairs that really
/// do contradict — target against its own affirmed twin.
///
/// Exists so `claim_identity` scores the corpus this module already documents
/// rather than a second one that drifts away from it. The truth is by
/// construction: pair `i` is `PAIRS[i].claim` against `AFFIRMED_TWINS[i]`, and
/// nothing else in the ledger is a contradiction of anything.
pub fn twin_pairs_corpus(
    embedder: &dyn VectorEmbed,
) -> (
    physis_core::claim_identity::ScoredClaims,
    physis_core::claim_identity::TruthPairs,
) {
    let (_, mut index) = build_ledger(embedder, true);
    // The agreeing restatements go in as negatives. Without them the eight
    // contradiction pairs are also the eight most similar pairs and any
    // threshold scores perfectly for the wrong reason.
    for a in PARAPHRASE_AGREEMENTS.iter() {
        index.push((format!("agree-{a:.8}"), a.to_string()));
    }
    let claims: Vec<(String, Vec<f32>)> = index
        .iter()
        .map(|(_, s)| (s.clone(), embedder.embed(s)))
        .collect();
    let find = |needle: &str| index.iter().position(|(_, s)| s == needle);
    let mut truth = Vec::new();
    for (i, p) in PAIRS.iter().enumerate() {
        if let (Some(a), Some(b)) = (find(p.claim), find(AFFIRMED_TWINS[i])) {
            truth.push((a, b));
        }
    }
    (claims, truth)
}

/// The commands that have a refutation in the ledger, lexical phrasing.
pub fn pair_commands() -> Vec<&'static str> {
    PAIRS.iter().map(|p| p.lexical).collect()
}

/// The commands that have nothing to warn about.
pub fn unrelated_commands() -> Vec<&'static str> {
    UNRELATED_COMMANDS.to_vec()
}

/// Which phrasing of the command is issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arm {
    /// The command shares the claim's own words.
    Lexical,
    /// The same act, with no content word in common with the claim.
    Paraphrase,
}

impl Arm {
    pub fn label(&self) -> &'static str {
        match self {
            Arm::Lexical => "lexical",
            Arm::Paraphrase => "paraphrase",
        }
    }
    fn command(&self, p: &Pair) -> &'static str {
        match self {
            Arm::Lexical => p.lexical,
            Arm::Paraphrase => p.paraphrase,
        }
    }
}

/// What one arm scored, with both nulls measured in the same pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmResult {
    pub arm: String,
    /// Commands issued — one per pair.
    pub queries: usize,
    /// How many claims `act` surfaces; the CLI default is 5.
    pub top: usize,
    /// Queries whose contradicting claim appeared in the top.
    pub hits: usize,
    pub recall: f32,
    /// Hits that would also have been *rendered* as a warning. Equal to `hits`
    /// unless `is_warning` and the corpus disagree, which would be a bug.
    pub warned: usize,
    pub warning_recall: f32,
    /// Mean reciprocal rank over the queries; 0 for a miss. Rank matters
    /// because the operator reads a list, and the fifth line is not the first.
    pub mrr: f32,
    /// `top` claims drawn at random from the whole ledger.
    pub null_uniform_recall: f32,
    /// `top` claims drawn at random from the contradicted claims only.
    pub null_status_recall: f32,
    pub margin_uniform: f32,
    pub margin_status: f32,
    pub verdict: String,
    /// Per-query detail, so a failure can be read rather than only counted.
    pub cases: Vec<CaseResult>,
}

/// One command's outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseResult {
    pub command: String,
    pub target: String,
    /// 1-based rank of the target within the surfaced list; `None` for a miss.
    pub rank: Option<usize>,
    /// Cosine the target scored, whether or not it made the cut.
    pub target_relevance: f32,
    /// The claim that outranked everything, which on a miss is the diagnosis.
    pub top_statement: String,
}

/// The whole run: both arms, the embedder that produced them, and provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallRun {
    pub ledger_size: usize,
    pub contradicted_claims: usize,
    pub top: usize,
    pub seed: u64,
    pub null_trials: usize,
    /// Which embedder ranked. A recall claim made on the lexical-hash fallback
    /// is not a recall claim; it is recorded, never assumed.
    pub embedder: String,
    pub physis_version: String,
    pub git_commit: String,
    pub arms: Vec<ArmResult>,
    /// Refutation versus its own endorsement, on a ledger that also holds the
    /// affirmed twin of every target. Null 0.500 by construction.
    pub polarity: Vec<PolarityResult>,
    pub polarity_ledger_size: usize,
    /// Selection policies, benefit and cost measured in the same pass.
    pub selection: Vec<SelectionResult>,
    /// The mixing weight between the semantic and lexical legs, both metrics
    /// on every row.
    pub hybrid: Vec<HybridPoint>,
}

impl RecallRun {
    pub fn arm(&self, label: &str) -> Option<&ArmResult> {
        self.arms.iter().find(|a| a.arm == label)
    }

    pub fn render(&self) -> String {
        let mut o = String::new();
        o.push_str(&format!(
            "── ACT RECALL ──\nledger {} claims ({} contradicted) · top {} · embedder {} · seed {}\n\n",
            self.ledger_size, self.contradicted_claims, self.top, self.embedder, self.seed
        ));
        o.push_str("  arm          recall   warn    MRR   null(unif)  null(status)   Δunif   Δstatus  verdict\n");
        for a in &self.arms {
            o.push_str(&format!(
                "  {:<11} {:>5.3}  {:>5.3}  {:>5.3}      {:>6.3}        {:>6.3}  {:>+7.3}  {:>+7.3}  {}\n",
                a.arm,
                a.recall,
                a.warning_recall,
                a.mrr,
                a.null_uniform_recall,
                a.null_status_recall,
                a.margin_uniform,
                a.margin_status,
                a.verdict
            ));
        }
        if self.embedder == "random-projection" {
            o.push_str(
                "\n  NOTE: random-projection is a lexical hash. These numbers are a floor,\n\
                 \x20       not a retrieval claim. Set PHYSIS_MODEL_DIR and re-run.\n",
            );
        }
        o.push_str(&format!(
            "\n── POLARITY: refutation vs its own endorsement ──\nledger {} claims (every target's affirmed twin added) · null 0.500 by construction\n\n",
            self.polarity_ledger_size
        ));
        o.push_str("  arm          discrim  target@top  twin@top  reassured  verdict\n");
        for p in &self.polarity {
            o.push_str(&format!(
                "  {:<11} {:>6.3}     {:>2}/{:<2}       {:>2}/{:<2}     {:>2}/{:<2}    {}\n",
                p.arm,
                p.discrimination,
                p.target_in_top,
                p.queries,
                p.twin_in_top,
                p.queries,
                p.reassured_instead,
                p.queries,
                p.verdict
            ));
        }
        if self.polarity.iter().any(|p| p.reassured_instead > 0) {
            o.push_str(
                "\n  `reassured` is the harmful cell: the endorsement made the list and the\n   refutation did not, so `act` printed a Supported claim about the very\n   thing a Contradicted claim refutes.\n",
            );
        }
        o.push_str(
            "\n── SELECTION: does reserving a slot for the warning pay? ──\nlexical arm on the twin ledger · reassured = harm · false alarms = price\n\n",
        );
        o.push_str("  top  policy          reassured  target@top  false alarms  removed/alarm\n");
        for r in &self.selection {
            o.push_str(&format!(
                "  {:>3}  {:<14}   {:>2}/{:<2}      {:>2}/{:<2}       {:>2}/{:<2}      {}\n",
                r.top,
                r.policy,
                r.reassured,
                r.pairs,
                r.target_in_top,
                r.pairs,
                r.false_alarms,
                r.unrelated,
                match r.harm_removed_per_false_alarm {
                    None => "—".to_string(),
                    Some(v) if v.is_infinite() => "free".to_string(),
                    Some(v) => format!("{v:.2}"),
                }
            ));
        }
        o.push_str(
            "\n── HYBRID: alpha * cosine + (1-alpha) * BM25 ──\nalpha 1.00 is cosine only (the control) · polarity null 0.500\n\n",
        );
        o.push_str(
            "  policy         recall(para)  recall(lex)  polarity(lex)  polarity(para)  combined\n",
        );
        let best = self
            .hybrid
            .iter()
            .max_by(|a, b| {
                a.combined
                    .partial_cmp(&b.combined)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|h| h.policy.clone())
            .unwrap_or_default();
        for h in &self.hybrid {
            o.push_str(&format!(
                "  {:<13} {:>8.3}     {:>8.3}     {:>8.3}       {:>8.3}     {:>6.3}{}\n",
                h.policy,
                h.paraphrase_recall,
                h.lexical_recall,
                h.polarity_lexical,
                h.polarity_paraphrase,
                h.combined,
                if h.policy == best {
                    "  <-- best combined"
                } else {
                    ""
                }
            ));
        }
        o.push_str("\n  misses, most instructive first:\n");
        for a in &self.arms {
            for c in a.cases.iter().filter(|c| c.rank.is_none()) {
                o.push_str(&format!(
                    "  [{}] {}\n      wanted: {}\n      got:    {}\n",
                    a.arm,
                    c.command.chars().take(72).collect::<String>(),
                    c.target.chars().take(72).collect::<String>(),
                    c.top_statement.chars().take(72).collect::<String>()
                ));
            }
        }
        o
    }
}

/// Build the ledger. One pass, so every claim carries the same age.
///
/// Age is the variable the plan's null controls for, and the cheapest way to
/// control for it is to remove it: nothing here is older than anything else, so
/// nothing can be ranked by recency by accident.
///
/// `with_twins` adds [`AFFIRMED_TWINS`]. The base ledger keeps it off so the
/// recall numbers already recorded stay comparable — the polarity arm asks a
/// different question on a different ledger, and mixing the two would change
/// both.
fn build_ledger(
    embedder: &dyn VectorEmbed,
    with_twins: bool,
) -> (PhysisCore, Vec<(String, String)>) {
    let mut core = PhysisCore::new();
    // (id, statement) in insertion order, for the nulls to draw from.
    let mut index: Vec<(String, String)> = Vec::new();

    let insert = |core: &mut PhysisCore,
                  index: &mut Vec<(String, String)>,
                  text: &str,
                  contradicted: bool| {
        let mut h = Hypothesis::new(text, embedder.embed(text));
        if contradicted {
            h.add_contradicting_evidence(Evidence::contradicts("benchmark", "measured to fail"));
        } else {
            h.add_supporting_evidence(Evidence::supports("benchmark", "recorded"));
        }
        index.push((h.id.clone(), h.statement.clone()));
        core.hypotheses.insert(h.id.clone(), h);
    };

    for p in PAIRS.iter() {
        insert(&mut core, &mut index, p.claim, true);
    }
    if with_twins {
        // Supported, not Contradicted: the twin is the endorsement, and its
        // status is what makes surfacing it instead of the target harmful
        // rather than merely wrong.
        for t in AFFIRMED_TWINS.iter() {
            insert(&mut core, &mut index, t, false);
        }
    }
    for s in CONTRADICTED_DISTRACTORS.iter() {
        insert(&mut core, &mut index, s, true);
    }
    for s in NEUTRAL_DISTRACTORS.iter() {
        insert(&mut core, &mut index, s, false);
    }
    (core, index)
}

/// Empirical recall of drawing `top` distinct claims at random from `pool`.
///
/// Measured rather than derived from `top/|pool|` so the null is produced the
/// same way the arm is — a sampled list of the same length — and so a mistake
/// in the pool shows up as a number instead of hiding in the algebra.
fn null_recall(pool: &[String], targets: &[String], top: usize, seed: u64) -> f32 {
    if pool.is_empty() || targets.is_empty() {
        return 0.0;
    }
    let mut rng = StdRng::seed_from_u64(seed);
    let mut hits = 0usize;
    let mut draws = 0usize;
    for t in targets {
        for _ in 0..NULL_TRIALS {
            let mut idx: Vec<usize> = (0..pool.len()).collect();
            idx.shuffle(&mut rng);
            let surfaced = &idx[..top.min(idx.len())];
            if surfaced.iter().any(|&i| &pool[i] == t) {
                hits += 1;
            }
            draws += 1;
        }
    }
    hits as f32 / draws as f32
}

/// Score one arm.
pub fn run_arm(
    core: &PhysisCore,
    index: &[(String, String)],
    arm: Arm,
    top: usize,
    embedder: &dyn VectorEmbed,
    seed: u64,
) -> ArmResult {
    let mut cases: Vec<CaseResult> = Vec::new();
    let mut hits = 0usize;
    let mut warned = 0usize;
    let mut rr = 0f32;

    for p in PAIRS.iter() {
        let command = arm.command(p);
        // The whole ledger is ranked, not only the top, so a miss can report
        // the score the target actually got. `bearing_on` truncates, so the
        // full ranking is asked for and cut here instead.
        let full = bearing_on(core, command, embedder, index.len());
        let rank = full.iter().position(|b| b.statement == p.claim);
        let target_relevance = rank.map(|i| full[i].relevance).unwrap_or(f32::NAN);
        let within = rank.filter(|&i| i < top);
        if let Some(i) = within {
            hits += 1;
            rr += 1.0 / (i as f32 + 1.0);
            if full[i].is_warning() {
                warned += 1;
            }
        }
        cases.push(CaseResult {
            command: command.to_string(),
            target: p.claim.to_string(),
            rank: within.map(|i| i + 1),
            target_relevance,
            top_statement: full
                .first()
                .map(|b| b.statement.clone())
                .unwrap_or_default(),
        });
    }

    let queries = PAIRS.len();
    let recall = hits as f32 / queries as f32;
    let targets: Vec<String> = PAIRS.iter().map(|p| p.claim.to_string()).collect();
    let all: Vec<String> = index.iter().map(|(_, s)| s.clone()).collect();
    let contradicted: Vec<String> = PAIRS
        .iter()
        .map(|p| p.claim.to_string())
        .chain(CONTRADICTED_DISTRACTORS.iter().map(|s| s.to_string()))
        .collect();

    let null_uniform_recall = null_recall(&all, &targets, top, seed);
    let null_status_recall = null_recall(&contradicted, &targets, top, seed.wrapping_add(1));
    let margin_uniform = recall - null_uniform_recall;
    let margin_status = recall - null_status_recall;

    // One query's worth of recall. A margin under this is one lucky pairing,
    // and one lucky pairing is not a result.
    let noise = 1.0 / queries as f32;
    let verdict = if margin_uniform > noise && margin_status > noise {
        "HOLDS".to_string()
    } else if margin_uniform > noise {
        "WEAK — beats chance, not a status filter".to_string()
    } else {
        "FAILS".to_string()
    };

    ArmResult {
        arm: arm.label().to_string(),
        queries,
        top,
        hits,
        recall,
        warned,
        warning_recall: warned as f32 / queries as f32,
        mrr: rr / queries as f32,
        null_uniform_recall,
        null_status_recall,
        margin_uniform,
        margin_status,
        verdict,
        cases,
    }
}

/// A pairwise discrimination: refutation versus its own endorsement.
///
/// The base arms ask "is the target in the top?". This asks the question that
/// follows it: with both the refutation and its affirmed twin in the ledger,
/// **which one comes first?** There is no topic to separate them — same
/// subject, same vocabulary, opposite verdict — so a ranker with no purchase on
/// polarity is a coin flip, and the null is exactly 0.500 by construction
/// rather than by sampling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolarityResult {
    /// Which phrasing of the command was issued.
    pub arm: String,
    pub queries: usize,
    pub top: usize,
    /// Pairs where the refutation outranked its endorsement.
    pub target_first: usize,
    /// `target_first / queries`. The null is 0.500.
    pub discrimination: f32,
    /// Pairs where the refutation reached the list the operator sees.
    pub target_in_top: usize,
    /// Pairs where the *endorsement* reached it. A high number here with a low
    /// `target_in_top` is the harmful case: `act` prints a reassuring claim and
    /// withholds the refutation.
    pub twin_in_top: usize,
    /// Pairs where the endorsement was surfaced and the refutation was not.
    pub reassured_instead: usize,
    pub verdict: String,
    pub cases: Vec<PolarityCase>,
}

/// One command's refutation-versus-endorsement outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolarityCase {
    pub command: String,
    /// 1-based rank of the refutation over the whole ledger.
    pub target_rank: usize,
    /// 1-based rank of its affirmed twin.
    pub twin_rank: usize,
    pub target_relevance: f32,
    pub twin_relevance: f32,
}

/// Score the polarity arm for one phrasing.
pub fn run_polarity(
    core: &PhysisCore,
    index: &[(String, String)],
    arm: Arm,
    top: usize,
    embedder: &dyn VectorEmbed,
) -> PolarityResult {
    let mut cases = Vec::new();
    let (mut target_first, mut target_in_top, mut twin_in_top, mut reassured) = (0, 0, 0, 0);

    for (i, p) in PAIRS.iter().enumerate() {
        let command = arm.command(p);
        let full = bearing_on(core, command, embedder, index.len());
        let twin = AFFIRMED_TWINS[i];
        let tr = full.iter().position(|b| b.statement == p.claim);
        let wr = full.iter().position(|b| b.statement == twin);
        // Both are in this ledger by construction; a missing one is a corpus
        // bug, and ranking it last is the reading that cannot flatter the
        // result.
        let tr = tr.unwrap_or(full.len().saturating_sub(1));
        let wr = wr.unwrap_or(full.len().saturating_sub(1));
        if tr < wr {
            target_first += 1;
        }
        let t_in = tr < top;
        let w_in = wr < top;
        if t_in {
            target_in_top += 1;
        }
        if w_in {
            twin_in_top += 1;
        }
        if w_in && !t_in {
            reassured += 1;
        }
        cases.push(PolarityCase {
            command: command.to_string(),
            target_rank: tr + 1,
            twin_rank: wr + 1,
            target_relevance: full.get(tr).map(|b| b.relevance).unwrap_or(f32::NAN),
            twin_relevance: full.get(wr).map(|b| b.relevance).unwrap_or(f32::NAN),
        });
    }

    let queries = PAIRS.len();
    let discrimination = target_first as f32 / queries as f32;
    // One query's worth either side of the coin flip. Eight pairs cannot
    // support a finer claim than that, and pretending otherwise would be the
    // thing this module exists to stop.
    let noise = 1.0 / queries as f32;
    let verdict = if discrimination > 0.5 + noise {
        "HOLDS — ranks the refutation over its endorsement".to_string()
    } else if discrimination < 0.5 - noise {
        "INVERTED — ranks the endorsement first".to_string()
    } else {
        "COIN FLIP — no purchase on polarity".to_string()
    };

    PolarityResult {
        arm: arm.label().to_string(),
        queries,
        top,
        target_first,
        discrimination,
        target_in_top,
        twin_in_top,
        reassured_instead: reassured,
        verdict,
        cases,
    }
}

/// One selection policy, scored on both the benefit and the cost.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionResult {
    /// `relevance`, or `reserve@<floor>`.
    pub policy: String,
    pub top: usize,
    /// Pairs where the endorsement was shown and the refutation was not.
    /// The harm the reserve exists to remove. Lower is better.
    pub reassured: usize,
    /// Pairs where the refutation reached the list at all.
    pub target_in_top: usize,
    /// Commands with nothing to warn about that got a warning anyway.
    /// The price of removing the harm. Lower is better.
    pub false_alarms: usize,
    pub pairs: usize,
    pub unrelated: usize,
    /// `reassured` removed per false alarm bought. `None` when the policy
    /// removes nothing, which is the honest reading of a policy that only
    /// costs.
    pub harm_removed_per_false_alarm: Option<f32>,
}

/// Score one selection policy at one `top`.
///
/// Both halves are measured in the same pass and on the same ledger, because a
/// policy evaluated only on the case it was designed for is a tautology.
pub fn run_selection(
    core: &PhysisCore,
    arm: Arm,
    top: usize,
    embedder: &dyn VectorEmbed,
    selection: crate::ported::act::Selection,
    baseline_reassured: Option<usize>,
) -> SelectionResult {
    let mut reassured = 0usize;
    let mut target_in_top = 0usize;
    for (i, p) in PAIRS.iter().enumerate() {
        let shown =
            crate::ported::act::bearing_on_with(core, arm.command(p), embedder, top, selection);
        let t = shown.iter().any(|b| b.statement == p.claim);
        let w = shown.iter().any(|b| b.statement == AFFIRMED_TWINS[i]);
        if t {
            target_in_top += 1;
        }
        if w && !t {
            reassured += 1;
        }
    }
    let mut false_alarms = 0usize;
    for cmd in UNRELATED_COMMANDS.iter() {
        let shown = crate::ported::act::bearing_on_with(core, cmd, embedder, top, selection);
        if shown.iter().any(|b| b.is_warning()) {
            false_alarms += 1;
        }
    }
    let policy = match selection {
        crate::ported::act::Selection::Relevance => "relevance".to_string(),
        crate::ported::act::Selection::WarningReserve { floor_ratio } => {
            format!("reserve@{floor_ratio:.2}")
        }
        crate::ported::act::Selection::Hybrid { alpha } => format!("hybrid@{alpha:.2}"),
        crate::ported::act::Selection::HybridRrf { k } => format!("rrf@{k:.0}"),
        crate::ported::act::Selection::CascadeRerank { pool } => format!("cascade@{pool}"),
        crate::ported::act::Selection::CascadeFloor { pool, floor } => {
            format!("cascade@{pool}+floor{floor:.2}")
        }
    };
    // Against the relevance baseline measured in the same run, never against a
    // remembered number.
    let harm_removed_per_false_alarm = baseline_reassured.and_then(|base| {
        let removed = base.saturating_sub(reassured);
        if removed == 0 {
            None
        } else if false_alarms == 0 {
            Some(f32::INFINITY)
        } else {
            Some(removed as f32 / false_alarms as f32)
        }
    });
    SelectionResult {
        policy,
        top,
        reassured,
        target_in_top,
        false_alarms,
        pairs: PAIRS.len(),
        unrelated: UNRELATED_COMMANDS.len(),
        harm_removed_per_false_alarm,
    }
}

/// The floors swept. 0.00 always reserves; 1.00 reserves only when the warning
/// already leads, which is the relevance policy by another name and is included
/// as the sweep's own control.
pub const FLOORS: [f32; 5] = [0.00, 0.80, 0.90, 0.95, 1.00];

/// Absolute cosine floors swept. Chosen to straddle the gap
/// `examples/floor_spread` measured on bge-base-en-v1.5 — commands with a
/// refutation bottom out at 0.717, commands with nothing to say top out at
/// 0.643 — with one value below the gap and one above it, so the sweep shows
/// the edges of the window and not only its middle.
pub const ABS_FLOORS: [f32; 5] = [0.60, 0.66, 0.68, 0.70, 0.75];

/// One mixing weight, scored on both legs at once.
///
/// The point of the sweep is that the two legs are measured on the **same
/// knob**: a weight that buys polarity by losing the topic has bought nothing,
/// and only putting both numbers on one row makes that visible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridPoint {
    /// `hybrid@<alpha>`, or `rrf@<k>` for the rank-fusion policy.
    pub policy: String,
    /// The topic leg: paraphrase recall@top on the base ledger.
    pub paraphrase_recall: f32,
    /// The topic leg's easy case, as a floor check.
    pub lexical_recall: f32,
    /// The verdict leg: polarity discrimination on the twin ledger, null 0.500.
    pub polarity_lexical: f32,
    pub polarity_paraphrase: f32,
    /// Both legs at once. `paraphrase_recall + polarity_paraphrase`, so a
    /// policy that trades one for the other cannot climb it.
    pub combined: f32,
}

/// Sweep the mixing weight and report both legs at every point.
pub fn run_hybrid_sweep(
    base: &PhysisCore,
    base_index: &[(String, String)],
    twin: &PhysisCore,
    twin_index: &[(String, String)],
    top: usize,
    embedder: &dyn VectorEmbed,
) -> Vec<HybridPoint> {
    let mut out = Vec::new();
    let recall_at = |core: &PhysisCore,
                     index: &[(String, String)],
                     arm: Arm,
                     sel: crate::ported::act::Selection| {
        let hits = PAIRS
            .iter()
            .filter(|p| {
                crate::ported::act::bearing_on_with(core, arm.command(p), embedder, top, sel)
                    .iter()
                    .any(|b| b.statement == p.claim)
            })
            .count();
        let _ = index;
        hits as f32 / PAIRS.len() as f32
    };
    let polarity_at = |arm: Arm, sel: crate::ported::act::Selection| {
        let n = PAIRS
            .iter()
            .enumerate()
            .filter(|(i, p)| {
                let full = crate::ported::act::bearing_on_with(
                    twin,
                    arm.command(p),
                    embedder,
                    twin_index.len(),
                    sel,
                );
                let tr = full.iter().position(|b| b.statement == p.claim);
                let wr = full.iter().position(|b| b.statement == AFFIRMED_TWINS[*i]);
                match (tr, wr) {
                    (Some(t), Some(w)) => t < w,
                    (Some(_), None) => true,
                    _ => false,
                }
            })
            .count();
        n as f32 / PAIRS.len() as f32
    };

    let mut policies: Vec<(String, crate::ported::act::Selection)> = ALPHAS
        .iter()
        .map(|a| {
            (
                format!("hybrid@{a:.2}"),
                crate::ported::act::Selection::Hybrid { alpha: *a },
            )
        })
        .collect();
    // The crate's own hybrid, which has existed in rag.rs since G5 and which
    // the ledger has never called. Included so the sweep says whether the
    // weight is worth having at all, or whether the shipped rank fusion
    // already gets there.
    policies.push((
        "rrf@60".to_string(),
        crate::ported::act::Selection::HybridRrf { k: 60.0 },
    ));
    // The cascade: cosine retrieves the pool, BM25 ranks it. Pools swept
    // because the pool size is the knob that trades the two stages, the way
    // alpha was supposed to and did not.
    for pool in [3usize, 5, 10, 20] {
        policies.push((
            format!("cascade@{pool}"),
            crate::ported::act::Selection::CascadeRerank { pool },
        ));
    }

    for (policy, sel) in policies {
        let paraphrase_recall = recall_at(base, base_index, Arm::Paraphrase, sel);
        let lexical_recall = recall_at(base, base_index, Arm::Lexical, sel);
        let polarity_lexical = polarity_at(Arm::Lexical, sel);
        let polarity_paraphrase = polarity_at(Arm::Paraphrase, sel);
        out.push(HybridPoint {
            policy,
            paraphrase_recall,
            lexical_recall,
            polarity_lexical,
            polarity_paraphrase,
            combined: paraphrase_recall + polarity_paraphrase,
        });
    }
    out
}

/// Mixing weights swept. 1.00 is the cosine-only policy and is the control:
/// it must reproduce the recorded 0.875 / 0.125.
pub const ALPHAS: [f32; 11] = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];

/// Build the ledger and score both arms.
pub fn run(top: usize, embedder: &dyn VectorEmbed, embedder_kind: &str, seed: u64) -> RecallRun {
    let (core, index) = build_ledger(embedder, false);
    let arms = vec![
        run_arm(&core, &index, Arm::Lexical, top, embedder, seed),
        run_arm(&core, &index, Arm::Paraphrase, top, embedder, seed),
    ];
    // A second ledger, because the twins would change the base numbers if they
    // shared one. Same construction, eight claims longer.
    let (tcore, tindex) = build_ledger(embedder, true);
    let polarity = vec![
        run_polarity(&tcore, &tindex, Arm::Lexical, top, embedder),
        run_polarity(&tcore, &tindex, Arm::Paraphrase, top, embedder),
    ];
    // The selection sweep runs on the twin ledger — the only one where
    // `reassured` is defined — at the cut where the harm was largest (1) and
    // the cut the CLI defaults to.
    let mut selection = Vec::new();
    for t in [1usize, top] {
        let base = run_selection(
            &tcore,
            Arm::Lexical,
            t,
            embedder,
            crate::ported::act::Selection::Relevance,
            None,
        );
        let baseline = base.reassured;
        selection.push(base);
        for f in FLOORS {
            selection.push(run_selection(
                &tcore,
                Arm::Lexical,
                t,
                embedder,
                crate::ported::act::Selection::WarningReserve { floor_ratio: f },
                Some(baseline),
            ));
        }
        // The cascade goes through the same cost arm as the reserve did. A
        // reordering cannot change which claims are *available* at top 5, so
        // its false-alarm count should be identical there; at top 1 it picks a
        // different single claim out of the same pool, and that is where both
        // the benefit and any new cost have to show up.
        selection.push(run_selection(
            &tcore,
            Arm::Lexical,
            t,
            embedder,
            crate::ported::act::Selection::CascadeRerank { pool: 5 },
            Some(baseline),
        ));
        // The floor sweep. `false_alarms` must fall and `target_in_top` must
        // not: a floor that buys silence by dropping the refutation has bought
        // the wrong silence, and only measuring both columns shows which.
        for f in ABS_FLOORS {
            selection.push(run_selection(
                &tcore,
                Arm::Lexical,
                t,
                embedder,
                crate::ported::act::Selection::CascadeFloor { pool: 5, floor: f },
                Some(baseline),
            ));
        }
    }

    let hybrid = run_hybrid_sweep(&core, &index, &tcore, &tindex, top, embedder);

    RecallRun {
        hybrid,
        selection,
        polarity,
        polarity_ledger_size: tindex.len(),
        ledger_size: index.len(),
        contradicted_claims: PAIRS.len() + CONTRADICTED_DISTRACTORS.len(),
        top,
        seed,
        null_trials: NULL_TRIALS,
        embedder: embedder_kind.to_string(),
        physis_version: env!("CARGO_PKG_VERSION").to_string(),
        git_commit: physis_core::bench::git_head(),
        arms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use physis_core::embed::RandomProjectionEmbedder;

    /// The corpus must be what the doc header says it is. A ground truth that
    /// drifts from its description is worse than none.
    #[test]
    fn the_ledger_is_the_shape_the_nulls_assume() {
        let e = RandomProjectionEmbedder::new(128);
        let (core, index) = build_ledger(&e, false);
        assert_eq!(
            index.len(),
            LEDGER_SIZE,
            "ledger size is quoted in the results"
        );
        assert_eq!(
            core.hypotheses.len(),
            LEDGER_SIZE,
            "no statement collided into one id"
        );
        let contradicted = core
            .hypotheses
            .values()
            .filter(|h| format!("{:?}", h.status) == "Contradicted")
            .count();
        assert_eq!(
            contradicted,
            PAIRS.len() + CONTRADICTED_DISTRACTORS.len(),
            "the status null draws from this pool"
        );
        // Every target must be findable at all, or the arm measures the corpus.
        for p in PAIRS.iter() {
            assert!(
                index.iter().any(|(_, s)| s == p.claim),
                "target absent from the ledger: {}",
                p.claim
            );
        }
    }

    /// The reserve must not fire when the warning is irrelevant. A floor of
    /// 1.00 reserves only when the warning already leads, so it must be
    /// indistinguishable from the relevance policy — that is the sweep's own
    /// control, and if it moves the sweep is measuring noise.
    #[test]
    fn the_top_floor_is_the_relevance_policy_by_another_name() {
        let e = RandomProjectionEmbedder::new(128);
        let (core, _) = build_ledger(&e, true);
        for arm in [Arm::Lexical, Arm::Paraphrase] {
            let base = run_selection(
                &core,
                arm,
                5,
                &e,
                crate::ported::act::Selection::Relevance,
                None,
            );
            let ceiling = run_selection(
                &core,
                arm,
                5,
                &e,
                crate::ported::act::Selection::WarningReserve { floor_ratio: 1.0 },
                None,
            );
            assert_eq!(base.reassured, ceiling.reassured, "{}", arm.label());
            assert_eq!(base.false_alarms, ceiling.false_alarms, "{}", arm.label());
        }
    }

    /// And a floor of 0.00 must always fire when any warning-eligible claim
    /// exists — otherwise the sweep's other end is not the other end.
    #[test]
    fn the_bottom_floor_always_surfaces_a_warning() {
        let e = RandomProjectionEmbedder::new(128);
        let (core, _) = build_ledger(&e, true);
        for cmd in UNRELATED_COMMANDS.iter() {
            let shown = crate::ported::act::bearing_on_with(
                &core,
                cmd,
                &e,
                5,
                crate::ported::act::Selection::WarningReserve { floor_ratio: 0.0 },
            );
            assert!(
                shown.iter().any(|b| b.is_warning()),
                "floor 0.00 surfaced no warning for {cmd}"
            );
        }
    }

    /// The reserve must not silently shorten the list, and must not duplicate
    /// the claim it promotes.
    #[test]
    fn the_reserve_keeps_the_list_the_same_length_and_free_of_duplicates() {
        let e = RandomProjectionEmbedder::new(128);
        let (core, index) = build_ledger(&e, true);
        for top in [1usize, 3, 5] {
            let shown = crate::ported::act::bearing_on_with(
                &core,
                "cargo build --release",
                &e,
                top,
                crate::ported::act::Selection::WarningReserve { floor_ratio: 0.0 },
            );
            assert_eq!(shown.len(), top.min(index.len()));
            let mut ids: Vec<&str> = shown.iter().map(|b| b.id.as_str()).collect();
            ids.sort_unstable();
            let before = ids.len();
            ids.dedup();
            assert_eq!(ids.len(), before, "the promoted claim was duplicated");
        }
    }

    /// A twin is only a twin if it keeps the target's words and inverts only
    /// the verdict. Written by hand, so checked by machine — the same rule as
    /// the paraphrase arm, in the opposite direction.
    #[test]
    fn every_twin_shares_most_of_its_target_vocabulary() {
        const STOP: [&str; 24] = [
            "the", "a", "an", "and", "or", "to", "of", "in", "on", "at", "for", "with", "is",
            "are", "it", "this", "that", "as", "by", "from", "into", "than", "not", "no",
        ];
        let norm = |s: &str| -> Vec<String> {
            s.to_ascii_lowercase()
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|w| w.len() > 2 && !STOP.contains(w))
                .map(|w| w.to_string())
                .collect()
        };
        for (i, p) in PAIRS.iter().enumerate() {
            let claim = norm(p.claim);
            let twin = norm(AFFIRMED_TWINS[i]);
            let shared = claim.iter().filter(|w| twin.contains(w)).count();
            let frac = shared as f32 / claim.len() as f32;
            assert!(
                frac >= 0.5,
                "twin {i} shares only {:.0}% of its target's content words — that is a \
                 different claim, not an inverted one\n  target: {}\n  twin:   {}",
                frac * 100.0,
                p.claim,
                AFFIRMED_TWINS[i]
            );
        }
    }

    /// The twin ledger is the base ledger plus exactly the twins. If it were
    /// anything else, the polarity numbers and the recall numbers would not be
    /// about the same corpus.
    #[test]
    fn the_twin_ledger_is_the_base_ledger_plus_the_twins() {
        let e = RandomProjectionEmbedder::new(128);
        let (_, base) = build_ledger(&e, false);
        let (_, twinned) = build_ledger(&e, true);
        assert_eq!(twinned.len(), base.len() + AFFIRMED_TWINS.len());
        for t in AFFIRMED_TWINS.iter() {
            assert!(twinned.iter().any(|(_, s)| s == t), "twin missing: {t}");
            assert!(
                !base.iter().any(|(_, s)| s == t),
                "twin leaked into the base ledger, which would move the recall numbers: {t}"
            );
        }
    }

    /// A blind ranker has no purchase on polarity, and the check is on the
    /// mechanism rather than the verdict: with every relevance identical, the
    /// order is decided entirely by the tie-break, so any discrimination it
    /// scores is an artefact of that tie-break and not a capability.
    ///
    /// This test is why `bearing_on` tie-breaks on the statement. It first
    /// asserted the verdict string, and flaked: the old tie-break was a UUID
    /// prefix, freshly random per process, so eight coin flips landed wherever
    /// they landed. That was a real defect in `act` — ties were not
    /// reproducible — and the test found it by being wrong about what it could
    /// assert.
    #[test]
    fn a_blind_ranker_is_a_coin_flip_on_polarity() {
        struct Blind;
        impl VectorEmbed for Blind {
            fn embed(&self, _t: &str) -> Vec<f32> {
                vec![1.0; 8]
            }
            fn dimension(&self) -> usize {
                8
            }
        }
        let (core, index) = build_ledger(&Blind, true);
        let shown =
            crate::ported::act::bearing_on(&core, "cargo build --release", &Blind, index.len());
        let first = shown[0].relevance;
        assert!(
            shown.iter().all(|b| (b.relevance - first).abs() < 1e-6),
            "a blind embedder must score every claim alike"
        );
        // And the order it produces must at least be the same twice, or no
        // number measured on it means anything.
        let again =
            crate::ported::act::bearing_on(&core, "cargo build --release", &Blind, index.len());
        assert_eq!(
            shown.iter().map(|b| &b.statement).collect::<Vec<_>>(),
            again.iter().map(|b| &b.statement).collect::<Vec<_>>(),
            "tied claims must come back in a stable order"
        );
    }

    /// The paraphrase arm only tests what it claims to if the command really
    /// shares no content word with the claim. Written by hand, so checked by
    /// machine.
    #[test]
    fn the_paraphrase_arm_shares_no_content_word_with_its_claim() {
        // Function words are allowed to overlap; they carry no topic.
        const STOP: [&str; 24] = [
            "the", "a", "an", "and", "or", "to", "of", "in", "on", "at", "for", "with", "is",
            "are", "it", "this", "that", "as", "by", "from", "into", "than", "not", "no",
        ];
        let norm = |s: &str| -> Vec<String> {
            s.to_ascii_lowercase()
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|w| w.len() > 2 && !STOP.contains(w))
                .map(|w| w.to_string())
                .collect()
        };
        for p in PAIRS.iter() {
            let cmd = norm(p.paraphrase);
            let claim = norm(p.claim);
            let shared: Vec<&String> = cmd.iter().filter(|w| claim.contains(w)).collect();
            assert!(
                shared.is_empty(),
                "paraphrase shares {:?} with its claim — the arm is not testing paraphrase\n  cmd:   {}\n  claim: {}",
                shared,
                p.paraphrase,
                p.claim
            );
        }
    }

    /// The lexical arm must, conversely, share vocabulary — otherwise the two
    /// arms are the same arm and the comparison says nothing.
    #[test]
    fn the_lexical_arm_shares_vocabulary_with_its_claim() {
        for p in PAIRS.iter() {
            let claim = p.claim.to_ascii_lowercase();
            let shared = p
                .lexical
                .to_ascii_lowercase()
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|w| w.len() > 2)
                .any(|w| claim.contains(w));
            assert!(
                shared,
                "lexical arm shares nothing with its claim: {}",
                p.lexical
            );
        }
    }

    /// The null must be the number the construction implies. If drawing `top`
    /// of 48 does not land near 5/48, the sampler is broken and every margin
    /// computed against it is wrong.
    #[test]
    fn the_uniform_null_matches_its_construction() {
        let pool: Vec<String> = (0..48).map(|i| format!("claim {i}")).collect();
        let targets: Vec<String> = (0..8).map(|i| format!("claim {i}")).collect();
        let got = null_recall(&pool, &targets, 5, 7);
        let want = 5.0 / 48.0;
        assert!(
            (got - want).abs() < 0.02,
            "uniform null {got} should be near {want}"
        );
    }

    /// And the status null must be the harder one. If it were not above the
    /// uniform null, the second control would add nothing.
    #[test]
    fn the_status_null_is_harder_than_the_uniform_one() {
        let e = RandomProjectionEmbedder::new(128);
        let r = run(5, &e, "random-projection", 7);
        let a = r.arm("lexical").unwrap();
        assert!(
            a.null_status_recall > a.null_uniform_recall,
            "status null {} must exceed uniform {}",
            a.null_status_recall,
            a.null_uniform_recall
        );
    }

    /// The measurement must be able to fail. An embedder that ranks by nothing
    /// — a constant vector for every input — must not produce a HOLDS.
    #[test]
    fn a_blind_ranker_cannot_produce_a_verdict() {
        struct Blind;
        impl VectorEmbed for Blind {
            fn embed(&self, _t: &str) -> Vec<f32> {
                vec![1.0; 8]
            }
            fn dimension(&self) -> usize {
                8
            }
        }
        let r = run(5, &Blind, "blind", 7);
        for a in &r.arms {
            assert_ne!(
                a.verdict, "HOLDS",
                "a blind ranker scored HOLDS on {}",
                a.arm
            );
        }
    }

    /// Same seed, same numbers. A benchmark that moves on its own cannot
    /// attribute a change to a code change.
    #[test]
    fn the_run_is_deterministic() {
        let e = RandomProjectionEmbedder::new(128);
        let a = run(5, &e, "random-projection", 7);
        let b = run(5, &e, "random-projection", 7);
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    /// Every target is Contradicted, so a hit and a warning are the same event.
    /// If they ever diverge, `is_warning` has changed under the benchmark.
    #[test]
    fn every_hit_is_also_a_warning() {
        let e = RandomProjectionEmbedder::new(128);
        let r = run(5, &e, "random-projection", 7);
        for a in &r.arms {
            assert_eq!(
                a.hits, a.warned,
                "{}: hit and warning counts must agree",
                a.arm
            );
        }
    }
}
