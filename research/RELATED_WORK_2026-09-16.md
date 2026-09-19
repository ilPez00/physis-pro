# Related work — world models, 2026-09-16

> Purpose: record two supplied world-model papers and what, if anything, physis
> can take from them. House rule unchanged: **any adopted idea still has to beat
> a construction-matched null on physis's own harnesses before it ships.** That
> rule is the reason this note separates "method worth borrowing" from "evidence
> worth citing" — neither paper below supplies the latter for anything physis
> does.

Source note: both papers arrived as **supplied full text**, not as a fetch.
Tables 1–6 (Light-WAM) and Table 1 (SmallWorld) could therefore be read at
table level, which is a deeper check than the abstract-only pass in
`RELATED_WORK_2026-09-14.md`. Nothing was independently retrieved, and no
dataset, checkpoint or checksum was verified. Every number below is
`supplied_full_text`, not `independently_verified`.

---

## 1. Light-WAM — arXiv:2606.08242v1 [cs.CV], 6 Jun 2026

*Light-WAM: Efficient World Action Models with State-Fusion Action Decoding.*
Ziang Li, Dongzhou Cheng, Yibin Wang, Shiyue Wang, Xiaoyang Xu, Lingxuan Weng,
Juan Wang, Jiaqi Wang (Wuhan University; Shanghai Innovation Institute;
Southeast University; Fudan; East China Normal University).

**The claim.** World Action Models (WAMs) co-train a robot policy with future
video prediction, which improves the representations the policy learns. Existing
WAMs couple that objective to large generative architectures, so they are
expensive to train and slow to run closed-loop. Light-WAM keeps the objective
and removes the cost: a compact frozen video backbone (Wan2.1-T2V-1.3B) adapted
with LoRA plus sparse adapters at layers {8, 16, 24}, future-video supervision
applied in a **2× spatially downsampled latent space**, and a direct action
decoder — `StateFusionActionExpert` — that pools multi-level adapted states
through 16 learned queries per layer and predicts an action chunk in **one
forward pass**, with no test-time video rollout and no iterative denoising.

**Numbers (their tables).**

| what | value |
|---|---|
| LIBERO average success | 97.2% (98.2 / 99.6 / 97.8 / 93.0 on Spatial / Object / Goal / Long) |
| LIBERO rank | 1st among methods *without* embodied pretraining; 3rd overall |
| RoboTwin 2.0, 50 tasks | 76.4% clean / 76.3% randomized |
| parameters | 1.99B total, **0.44B trainable** (Fast-WAM: 6.73B / 6.02B) |
| training throughput | 2.08 steps/s vs Fast-WAM 0.49 → 4.25× |
| peak training memory | 43.1 GiB vs 70.7 GiB |
| inference | **72.03 ms**, 4.1 GiB peak (Fast-WAM 404.62 ms / 12.7 GiB; LingBot-VA 3214 ms / 18.9 GiB; Motus 2148 ms) |
| action branch alone | 2.1 ms of the 58.6 ms policy forward |

**Ablations (their Table 5, LIBERO-Spatial).** No downsampling 99.0 vs 2× 98.2
— the resolution cut costs 0.8 pp and buys the 4.25× throughput. Five adapter
layers 98.0 vs three 98.2 — more adapters buy nothing. **8 learned queries 95.4
vs 16 queries 98.2** — the query bottleneck has a real capacity floor below
which task-relevant detail is lost.

**What physis can take.**

1. **"The expensive component is not what produces the benefit."** Their
   central move is showing that test-time future generation — the costly part —
   is unnecessary, and that the value of video prediction is training-time
   representation learning. Physis has the same shape of problem, measured:
   `pack` spends 65–105 s per query and returns *worse* evidence coverage than a
   1–2 s lexical leg over the identical chunk grid and budget, on four
   independent frozen holdouts
   (`docs/plans/2026-09-16-day3-retrieval-repair.md`). The transferable lesson
   is the *method*: identify which component actually buys the outcome, then
   delete the rest — and report cost beside quality, which their Tables 3 and 4
   do and physis's retrieval notes only partly do (tokens and wall time, no
   memory).
2. **A controlled bottleneck needs its floor measured.** 8 queries → 95.4 is
   the same finding as "3000 tokens over ~2900 chunks is ~0.4% of the corpus",
   stated from the other side. Physis compresses everywhere (`pack`, `corpus.rs`
   community summaries, `TemporalValidity`); the ablation is a reminder that the
   floor is an empirical constant, not a design choice.
3. **Granularity as an explicit, measured knob.** Their resolution ablation
   trades 0.8 pp for 4.25× throughput. Physis's equivalent knob is
   `CHUNK_LINES = 40` in `src/pack.rs` — a constant, never swept. Sweeping
   {20, 40, 80} at equal budget on a frozen holdout is a cheap, direct
   experiment this paper makes worth running.

**What it does not give.** No construction-matched null anywhere in the headline
comparisons; baselines are prior systems, not matched constructions. It is
evidence about *their* task and method, not about physis's.

---

## 2. SmallWorld — arXiv:2511.23465v1 [cs.LG], 28 Nov 2025

*SmallWorld: Assessing Dynamics Understanding of World Models in Isolated
Environments.* Xinyi Li, Zaishuo Xia, Weyl Lu, Chenjie Hao, Yubei Chen (UC
Davis; Open Path AI Foundation).

**The claim.** World models are normally evaluated as a component of a
model-based RL pipeline, judged by accumulated policy reward, on environments
whose dynamics are coupled and opaque — so their predictive capability is never
actually measured. SmallWorld isolates it: deterministic, controllable, fully
observable state-space tasks, **no reward function at all**, parameterized data
distributions, and long-horizon imagination as a free variable.

**The suite.** Ten MuJoCo physics tasks in three groups — linear-velocity (free
fall, circular, projectile, inclined plane, pendulum), rotational (rigid-body
rolling, rotation, spin), energy-conservation (elastic collision, bouncing ball)
— plus a geometry suite framed as keypoint reprojection under a pinhole camera
model, plus established benchmarks converted to state space (Atari, Go,
DMControl, Panda Hand, Memory-Maze).

**Results (their Table 1, MSE over 90 imagined steps given 10 conditioning
steps; four architectures — RSSM/DreamerV3, Transformer/IRIS-style, Diffusion
Forcing, Neural ODE/MoSim).**

- Diffusion Forcing and MoSim consistently lowest error; Transformer
  consistently better than RSSM.
- MoSim is excellent on formalized physics (Spin 1.27e-7, Roll 0.0012) and poor
  on Atari (2683.9); Diffusion is the reverse (Atari 332.79, Spin 0.0012). The
  proposed reading: Neural ODE models *change* of state, which suits domains
  where transitions follow formal rules and not domains where they are abrupt.
- Long-horizon (their Fig. 4): MoSim's error grows **exponentially** with
  horizon — "weak self-correction, since MoSim generates the next state solely
  from the immediately preceding one". The others degrade more gracefully, and
  their differences shrink as horizon grows, which the authors attribute to
  "gradually losing information along the imagination trajectory and eventually
  relying only on their fundamental understanding of the underlying dynamics".
- Conclusion: a precision/robustness trade-off between paradigms, and that
  isolated evaluation "reveals structural differences that conventional
  RL-centric evaluations often obscure".

**What physis can take.**

1. **The methodological move is already this repository's move.** "Separate the
   model from the pipeline so you can measure the model" is exactly
   `benchmarks/retrieval/run.py`'s rule ("a benchmark scored by the thing it
   measures is not a benchmark") and exactly what the Day 3 work did — score the
   retrieval leg in isolation at equal budget instead of through an agent's task
   success. SmallWorld is the external precedent for that discipline, and worth
   citing when the ASTRA/VASTRA probes are defended
   (`docs/plans/2026-09-16-astra-vastra-programme.md` §6).
2. **Two design factors physis's harnesses lack.** (a) *Parameterized data
   distribution*: their tasks expose an explicit difficulty knob (initial
   velocity range) so generalisation can be probed. The retrieval holdouts have
   no such knob — difficulty is whatever the authored case happens to be, which
   is precisely why set #2 (7/12 boundary-straddling) and set #4
   (definition-seeking) gave opposite verdicts. Adding an explicit axis — span
   length, lexical overlap between query and evidence, file size — would make
   holdout difficulty a controlled variable instead of an accident. (b) *No
   reward confound*: already true here.
3. **A testable hypothesis about physis's own predictions.** Their long-horizon
   result is a statement about forecast error growth, and `physis-core` ships
   the machinery to test the same thing: `hypothesis.rs` has `Prediction`,
   `resolve_prediction`, `open_predictions` and `FitnessBreakdown`. The
   question — does physis's prediction error grow with the number of intervening
   revisions, and does it degrade by "falling back on fundamentals" or by
   diverging? — is answerable on the existing E78 (temporal drift) and E75
   (history constrains the future) harnesses. Cheap, and it is the first
   external hypothesis about `resolve_prediction` this track has had.
4. **Order-awareness is the mechanism they identify as missing.** MoSim fails
   at long horizon because it conditions on the immediately preceding state
   only. Physis's `becoming.rs` exists precisely because order-blind methods
   cannot separate "meaning changed" from "two things share a name", and
   `epistemic.rs` keeps a replayable belief timeline — the history MoSim lacks.
   This is external support for a primitive already in the tree, not a new one.

**What it does not give.** Same caveat as above: no construction-matched nulls,
and Table 1's columns span wildly different scales (Atari MSE in the hundreds to
thousands against Spin at 1e-7), so cross-domain comparisons are meaningful only
within a domain. Cite it for method, not for evidence.

---

## 3. CausalARC — arXiv:2509.03636v3 [cs.AI], 19 Mar 2026

*CausalARC: Abstract Reasoning with Causal World Models.* Jacqueline Maasch
(Cornell Tech), John Kalantari, Kia Khezeli (YRIKKA). NeurIPS 2025 LAW workshop.

**The claim.** Reasoning benchmarks are static datasets; static datasets leak,
and their difficulty is whatever the author happened to pick. CausalARC instead
makes each reasoning task a **sample from a fully specified structural causal
model (SCM)**, so the task distribution is a generative model: instances can be
drawn at will, exact test cases were never seen, and difficulty is a parameter
rather than an accident. Because the SCM is fully specified, all three rungs of
Pearl's Causal Hierarchy are well-defined — observational (L1), interventional
(L2), counterfactual (L3) — and the principled augmentations that produce L2 and
L3 samples are exactly what few-shot in-context demonstrations then expose.

**Design.** Task instances are returned in the official ARC dictionary format so
existing ARC pipelines run unchanged. The static set is 50 instances (five
demonstration pairs, one test pair, 5–10 counterfactual pairs per demonstration).
Each instance is human-annotated with at least one representation of its world
model: a Python program in all cases, SCM notation in a subset, the causal
adjacency matrix in a (non-disjoint) subset. Tasks are labelled by theme —
**counting, extension, logical, ordering** — and most have tunable array sizes.

**Results (their tables).**

| setting | result |
|---|---|
| abstract reasoning, MARC with test-time training | **46%** on CausalARC vs **47.1%** on ARC-AGI-1 → comparable difficulty. Wide spread by theme: extension and counting much harder than logic and ordering; steep falloff as array size grows |
| counterfactual reasoning, in-context | L3 demonstrations **did not consistently help**; the authors attribute this to lower demonstration diversity, since L3 fixes the exogenous context and so resembles its L1 counterpart |
| program synthesis | total in-context examples matter, peaking at 6 for most models; o4-mini best; Llama 4 Scout 17B produced no correct program at all |
| causal discovery with logical reasoning | Grok 4 consistently best (its default reasoning effort is high); every other model unstable, with no consistent monotonic trend as complexity scales |
| PC algorithm baseline on `xor` | structural Hamming distance **40** at n = 1000, 5000 and 10000 — a faithfulness violation, since logical/deterministic functions break the assumption that statistical independence mirrors graph separation |

**Contamination, self-reported.** o4-mini emitted "For ARC-style input/output:"
in a response to a prompt that never mentioned ARC; Claude Sonnet 4 volunteered
program synthesis on counterfactual tasks unprompted; Grok 4 was exceptional on
`xor`, which occurs in ARC-AGI-1. Inference-time reasoning effort also moved
results significantly. The authors state plainly that strong performance here is
**necessary but not sufficient** for demonstrating reasoning.

**What physis can take.**

1. **Generative task sampling is the direct fix for the holdout problem hit
   today.** Four hand-authored retrieval holdouts were spent on 2026-09-16, and
   two of them (set #2, 7/12 boundary-straddling; set #4, definition-seeking)
   gave **opposite verdicts on the same comparison** because difficulty was an
   accident of authoring rather than a controlled variable. CausalARC's design
   factor 1 is precisely the remedy: sample instances from a parameterized
   generator, report performance over a *distribution* of instances with
   uncertainty, and keep an explicit difficulty knob. For `equal_budget.py` this
   means generating cases from a spec — target file, span, query template,
   lexical-overlap knob, span length — instead of writing them by hand. This is
   the single most actionable item in any of the three papers.
2. **The PCH ladder is the ledger's vocabulary, half-implemented.** Physis
   already marks L1-adjacent provenance (`Mechanism::{Generated, Received,
   Derived, Corrected, Serves}` in `src/address.rs`) and keeps an append-only
   causal ledger with `lineage()` / `descendants()` / `purposes()`. It has **no
   L2 rung** (no `do()` operator anywhere) and its L3 story is `dream.rs`
   retrospective proposals, which replay history but do **not** pin an exogenous
   context — so they are not counterfactuals in Definition 2.4's sense
   (`Y_x(u) := Y_{M_x}(u)`, *u fixed*). Naming the rungs is free; adding the
   context key that makes L3 well-defined is a schema decision, cheap now and
   expensive to retrofit (the same argument as F4 in
   `RELATED_WORK_2026-09-14.md`).
3. **"Skill acquired from unlimited priors is not generalization."** Their
   discussion of Chollet's skill-acquisition-efficiency definition is the right
   vocabulary for what the four holdouts showed: a BM25 baseline wins when the
   query's surface form carries the answer and loses when the answer is
   definitional, which is a prior buying skill, not generalization. Worth citing
   in the Day 3 note.
4. **Fine-grained per-theme error analysis is already the shape here.** The
   `classes` field on the retrieval case files is the same instrument as
   CausalARC's counting/extension/logical/ordering labels and ConceptARC's
   concept labels — and per-class reporting is exactly what exposed the set #2
   versus set #4 flip.
5. **Known hazard: faithfulness violations.** If Physis's `relation.rs`
   (CAUSES, INFLUENCES) or `ledger.rs` lineage ever feeds a constraint-based
   causal discovery step, the PC result on `xor` is the warning: deterministic
   and logical functions — of which the ontology grid is full — break the
   assumption those algorithms rest on.

**What it does not give.** No construction-matched null in the LM comparisons
(though the PC baseline is a real one), an acknowledged contamination risk in
every LM number, a static set of only 50 tasks that the authors say is not for
fine-tuning, and no human baseline. Cite it for **method** — generative task
sampling and the PCH rung definitions — not for capability numbers.

---

## 4. Where this lands in the plan

| item | destination |
|---|---|
| Isolated-harness discipline as external precedent | `docs/plans/2026-09-16-astra-vastra-programme.md` §6 |
| **Generative, parameterized holdout cases instead of hand-authored ones** | next revision of `benchmarks/retrieval/equal_budget.py` — highest priority |
| Explicit difficulty axis (span length / lexical overlap / file size) | same |
| PCH rung vocabulary + an exogenous-context key for L3 | `src/ledger.rs` / `src/address.rs` schema decision |
| `CHUNK_LINES` sweep at equal budget | Day 3 follow-up, needs a fresh holdout |
| Forecast-error-vs-horizon on `hypothesis.rs` | new experiment, reusing E75/E78 protocols |
| Peak memory reported beside latency for the pack path | Day 3 note reporting standard |

Nothing here changes a shipped default. All three papers argue for the same
discipline this repository already applies — *measure the component in isolation
before trusting the pipeline it sits in*, and *state what would falsify it* —
which is why the last four retrieval candidates were all kept off by default.
CausalARC adds the missing half of that discipline: **generate the test set from
a parameterized model, so its difficulty is a variable you control rather than
one you discover after the fact.**

**Actor**: continuation session (OpenCode) · **Date**: 2026-09-16
