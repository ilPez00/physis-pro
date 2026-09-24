# Experiment plan

Date: 2026-09-24
Ranked by (falsifying power × publication impact) ÷ cost. Each entry names the
claim it can kill and the artifact it must leave behind. Run in order; every
completed experiment gets a stored JSON and a registry claim before any
manuscript is edited.

Standing rule from the programme brief: **prefer simple tests that can kill a
claim over elaborate tests that can only support it.**

---

## 0. Commit the portfolio ✔ (done)

Blockers 1 partly cleared. Five commits landed; `research/papers/` is now
tracked. Remaining: nothing is tagged and no result carries a commit hash,
so `provenance.json` cannot yet be completed. Tag at the next milestone.

---

## 1. P4 — sequential permuted null ⏳ running

**Kills:** the proposer's advantage under the realistic split protocol.
**Why:** the claim was asserted in the abstract without the arm existing.
**Doing:** `e21_sequential_split.py` with a 1,000-shuffle arm using
`run_e21.topk(mode="permuted")`'s own construction (shuffle training labels,
keep features, rebuild centroids, predict the same test entries).
**Artifact:** `research/e21_sequential_split.json` gains a `null` block per frac.
**Cost:** ~45 min CPU, no new data.
**Outcome branches:** margin ≈ +0.5 → the sequential result stands as primary.
Margin near zero → the proposer advantage is a random-split artifact and P4
loses its headline.

## 2. P1 — paired cold-start with two new arms ⭐ highest value

**Kills:** the cold-start effect, currently the only positive result P1 has.
**Why it is broken today:** the arms are compared on *different* held-out
target sessions, so the comparison is unpaired. Of the four arms the brief
asks for, only two exist:

| arm | status |
|---|---|
| A — local-only prior | exists |
| B — borrowed-only prior | exists |
| C — borrowed + local (shrunk/hierarchical) | **missing** |
| D — chance / global base rate | **missing** |

**Doing:** for each target-history size $N$, evaluate all four arms on the
**same** held-out sessions; sweep $N$ by sessions and by actions; add
smoothing for sparse action kinds; report paired CIs, coverage, and Brier/log
loss alongside AUC. Session-level resampling, ≥10,000 replicates.
**Artifact:** `research/e1_coldstart_paired.json`.
**Cost:** moderate — new code, existing corpus.
**Outcome branches:** borrowed+local beats local-only with a paired CI
excluding zero → P1 gains a real positive result. No arm separates → P1
becomes a clean negative with no cold-start rescue, which is still publishable
and more honest.

## 3. P1 — multiplicity correction ⭐ cheap, potentially fatal

**Kills:** the single surviving transfer pair.
**Why:** 12 directional tests are reported with no family-wise control. "One
95% CI excludes 0.5" is not established evidence out of 12 tests.
**Doing:** Holm and Benjamini–Hochberg over the 12 pairs, plus a global test
that the 12 AUCs are not all 0.5. Report corrected thresholds next to the raw
ones.
**Artifact:** `research/e1_multiplicity.json`.
**Cost:** minutes on stored intervals.
**Outcome branches:** `cline→opencode` survives both → the headline holds and
is now defensible. It fails → P1 is fully negative, and the abstract must say
so. **This is the cheapest test in the plan and it can remove P1's only
positive claim, so it runs before any other P1 work.**

## 4. P1 — the anti-predictive finding

**Kills:** nothing; it *adds* a stronger claim.
**Why:** 5–6 of 12 pairs are significantly **below** chance. "Does not
transfer" understates a result where a borrowed prior actively misleads.
**Doing:** report the below-chance count as a first-class finding, with the
same session-level CIs; check whether below-chance pairs share a source or
target harness.
**Artifact:** extends `bootstrap_unit_check.json`.
**Cost:** analysis only.

## 5. P9 — granularity kill test with intervals

**Kills:** the dataset paper's central validation.
**Why:** currently point AUCs (0.627–0.751 fine vs 0.446–0.525 coarse) with
no interval and no stated session-level unit. Also unexplained: *why* the fine
taxonomy wins. Candidates: command identity matters; project identity leaks
through command labels; rare actions dominate; harness-specific namespaces.
**Doing:** session-level bootstrap CIs around the *differences*; ablations
holding command identity constant (project-stratified), a rare-action
truncation arm, and per-harness breakdowns.
**Artifact:** `research/dataset_granularity_cis.json`.
**Cost:** moderate.
**Outcome branches:** CI on the difference excludes zero → kill test passes
properly. It includes zero → the taxonomy's value is unestablished and P9
reverts to a resource paper with no validated design choice.

## 6. P9 — release security audit 🔒 publication gate

**Kills:** the release, if it fails.
**Doing:** scan the released corpus for API keys, bearer tokens, SSH material,
hostnames, usernames, absolute paths, project names, emails and URLs —
including inside fields assumed redacted. Produce `research/release_audit.md`.
**Artifact:** `research/release_audit.md` + a scanner script.
**Cost:** low, but blocking. No release without it.

## 7. P10 — positive control ⭐

**Kills:** the negative result itself, if the pipeline cannot detect a
criticality that is known to be present. Without this, "power law never beats
lognormal" may be a property of the fitting code, not of the field.
**Doing:** run the *identical* pipeline on (a) a known-critical system
(branching process at the critical ratio; BTW sandpile), (b) a clearly
non-critical control, (c) the Empedoclean field. Add finite-size scaling at
N = 100/300/1000/3000: largest-cluster fraction, susceptibility, cutoff
scaling, seed variance.
**Artifact:** `research/empedocles_controls.json`.
**Cost:** moderate; new code.
**Outcome branches:** pipeline detects the known criticality → the
falsification stands and P10 is a solid negative result. Pipeline fails the
positive control → the falsification is uninterpretable and P10 must be
withdrawn, with the failure reported.

## 8. P3 + P4 — merge, and the anisotropy caveat

**Kills:** nothing; produces one strong paper instead of two weak ones.
**Doing:** merge into the eight-stage arc (certify → injections kill it →
misfilings at chance → cross-embedder instability → structural alternatives
fail → geometry still proposes → sequential weakens it → what remains is
human-decision leverage).
**Also required:** test whether cross-embedder disagreement *substantially
exceeds* within-embedder clustering stochasticity. If it does not, P3's
"representation instability is the dominant cause" must be weakened. Keep the
anisotropy slope exploratory — it rests on three points.
**Cost:** writing plus one clustering-stability experiment.

## 9. P2 — external replication

**Kills:** the trajectory claim, which currently rests on six files.
**Doing:** hundreds-to-thousands of evolving files from multiple public
repositories; representations BGE / Nomic / a third model / TF-IDF /
bag-of-tokens / edit distance; nulls = shuffled revision order, matched
non-self files, edit-magnitude-preserving semantic destruction, commit-time
shuffle, static-identity baseline, overlap-only predictor.
**Central question:** does an embedding trajectory reveal temporal structure
*beyond* the trivial fact that adjacent revisions share most of their text?
**Also:** measure effective rank / participation ratio rather than inferring
"2–3 dimensions" from PC1+PC2 explained variance; and treat bounded wandering
(`quality.rs`) as a trajectory *type*, not a failure.
**Cost:** high. P2 stays a pilot until this lands.

## 10. PM — validate the ledger

**Kills:** PM's status as a methodology contribution.
**Doing:** for each of the 12 cases verify that the original claim existed
before correction, that the apparent positive and the control are recoverable,
that the corrected result reproduces, and that the failure category and
detection timing are accurate. Build the executable claim ledger
(id, pre-registered metric, pre-registered null, threshold, status,
supersedes, evidence hash) and test the invariants: a result cannot rewrite
its own criterion; superseded claims stay visible; a missing required control
yields no promotable verdict; a STOP must actually prevent execution rather
than print STOP.
**Also:** the novelty claim is "executable falsification for agentic loops",
not "falsification is good". It must be positioned against the AI-scientist
literature (Phase 9) or it will be rejected as a truism.

---

## Deferred

- Second independent rater for the ontology filing task. No independent
  human-labelled corpus exists; do not fake one. State plainly that the model
  predicts *one rater's policy*, not ontology truth.
- External taxonomies (WordNet / SUMO / DOLCE / Wikidata subsets) with ≥3
  embedders.
- Current related-work audit per paper.
