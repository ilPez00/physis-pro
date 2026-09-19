# Literature pass — development-cycle-2026-09-15 (semantic operational computer directive)

Timeboxed to ~20 minutes per the priority-update directive. This is **not** an
exhaustive search of the window 2026-02-15 to 2026-09-15; it is a first pass
over the directive's eight named, numerically anchored seeds, verified against
each paper's own arXiv abstract page (not the full PDF/table — see
"Verification depth" per entry). A follow-up queue for the remaining named
seeds (Persistent Semantic Context [partially covered below], ActionEngine,
Executable Agentic Memory, Graph of Skills, SING, ReFind, WorldDB, User as
Code, MRMS, Memanto — all covered — plus semantic filesystems, computer-use
agents, tool discovery beyond SING, graph/state-machine planning beyond
ActionEngine, ontology induction, context compression, semantic digital
twins, and neuro-symbolic robotics, none searched this pass) is at the bottom.
Machine-readable form: `literature.json` in this directory.

**Search method:** WebSearch per named seed/anchor (one query per entry
below), then WebFetch of the arXiv `/abs/` page for each of the eight
directive-supplied anchors to check the claimed numbers against the paper's
own abstract. No full-text PDF extraction, no ablation-table verification,
no dataset/checksum verification — all of that remains open (see "Verification
depth" and the follow-up queue).

**Correction (2026-09-15, after a peer review flagged this):** every row in
the table below was checked against an arXiv **abstract**, never against the
paper's own results table, sample-level methodology, or dataset/checksum
details. An abstract is not the results table. Every number in this section
is **UNVERIFIED** in the directive's sense ("Mark supplied numbers UNVERIFIED
until extracting the original experimental setup/table/page") — including
the five rows this document originally called "MATCHES" without further
qualification. "Matches the abstract's own wording" is a real but shallow
check: it rules out this worker or the user having misquoted the paper's own
summary of itself; it does not rule out cherry-picked framing, an
unfavorable-condition omission, or a number that reads differently once the
table's caveats are visible. Treat every "MATCHES" below as "matches the
abstract text, UNVERIFIED against the table."

## Supplied empirical anchors — verification status

| Work | arXiv ID | User-supplied anchor | Verified against abstract | Verification depth |
|---|---|---|---|---|
| ActionEngine | 2602.20502 | 95% vs 66%; ~1 LLM call; 11.8x cheaper; 2x faster | **MATCHES**, with one correction: the 95%/66%/1-call figures are specifically on **Reddit tasks from WebArena**, not all tasks in the benchmark. Abstract also gives task-completion-time (237s→118s, -50%) and LLM-calls-per-task (10.2→1.8, -82%) numbers the user's anchor did not include. | Abstract only. Baseline named as "the strongest vision-only baseline" -- which specific system that is, and full-benchmark (non-Reddit-subset) numbers, are UNVERIFIED. |
| Executable Agentic Memory | 2605.12294 | up to +19.6% AndroidWorld; 6x lower token cost; 2.8s mean latency | **MATCHES** the abstract's own wording ("outperforms ... UI-TARS-7B by up to 19.6%"), but the abstract does not say whether 19.6 is percentage points or relative percent, and the user's anchor does not either -- **UNRESOLVED, flag as ambiguous, do not repeat as pp or as relative without checking the results table.** 8.3K tokens/step and 52.6% absolute AndroidWorld score also appear in search results but are UNVERIFIED against the abstract text directly (found via search-engine synthesis, not the fetched abstract). | Abstract only; ambiguity unresolved. |
| Graph of Skills | 2604.05333v2 | +43.6% reward; -37.8% input tokens; libraries 200-2,000 | **MATCHES** abstract wording exactly ("improves average reward by 43.6% ... while reducing input tokens by 37.8%"), baseline named as "vanilla full skill-loading baseline," benchmarks SkillsBench + ALFWorld. Relative-percent-vs-pp is again **unspecified in the abstract itself** -- reward is a continuous score, so a bare "43.6%" is most consistent with a relative improvement, but this is an inference, not a confirmed reading. | Abstract only. |
| SING | 2606.16591 | 7,471 tools; +59.8% Global Recall@5; +28.9% success; -99.8% exposed schemas | **MATCHES** abstract wording. Corpus size (7,471 tools, 779 servers) confirmed. Three benchmark names (MCP-Universe, MCP-Atlas, MCP-Bench) found via search synthesis but **not directly quoted from the fetched abstract text** -- treat benchmark-name attribution as UNVERIFIED pending a table read. | Abstract only. |
| ReFind | 2608.12888 | 58.2 vs 53.2 (strongest compared baseline) | **MATCHES**, and more precisely specified than the user's anchor: metric is mean accuracy on ~2,800 questions under MemoryAgentBench's incremental multi-turn setting, baseline is specifically HippoRAG 2 (53.2), backbone is GPT-4o-mini for every compared system (a controlled-backbone comparison, which strengthens the claim). | Abstract only; the "raw-history iterative lexical retrieval" framing in the user's anchor matches the paper's own description of ReFind as unstructured/lexical. |
| WorldDB | 2604.18478 | 96.40% LongMemEval-s; ~+7pp from graph machinery in ablation | **MATCHES**, and the ablation number in the abstract is more precise than the user's anchor: "+7.0pp task-averaged independently of the underlying answerer," attributed specifically to "resolver-unified entities and typed refers_to edges." Answerer model is Claude Opus 4.7, n=500 questions, ~115k-token stacks. Comparison baselines: Hydra DB (90.79%, so WorldDB is +5.61pp not the raw 96.40% in isolation) and Supermemory (85.20%, +11.20pp). | Abstract only; the exact ablation table (what else was removed alongside the graph layer) is UNVERIFIED. |
| User as Code | 2606.16707 | 78.8% LoCoMo recall; ~99% aggregate-query vs 6-43% for retrieval | **MATCHES** on both numbers via the fetched abstract: "78.8% on LOCOMO" and "retrieval-based memory collapses (6-43%) while UaC stays near-perfect (99%)." Abstract additionally gives a Full-Context upper bound of 79.8% (UaC is within 1.0pp of it) that the user's anchor omitted. | Abstract only; the specific retrieval baselines making up the 6-43% range, and the aggregate-query task's exact definition beyond the one worked example, are UNVERIFIED. |
| Persistent Semantic Context | not found under this exact name | +19pp accuracy; semantic-knowledge gap remains dominant failure source | **NOT LOCATED under this exact title in this pass.** Nearby, differently-scoped papers were found instead: "Memori: A Persistent Memory Layer..." (2603.19935, 81.95% LoCoMo, 1,294 tokens/query) and "Persistent Semantic Entities in Tool-Augmented LLM Systems" (2608.07952, a different topic -- implicit cross-session state, not an accuracy-percentage-point paper). **Mark the +19pp anchor UNVERIFIED / possible title mismatch** -- either the seed name in the directive refers to a paper not surfaced by these two searches, or it is a paraphrase of a paper found under a different title. Needs a follow-up search on the +19pp figure itself rather than the title string. | Not located; do not cite the +19pp number as sourced. |

**Cross-cutting caveat that applies to every row above:** none of these
numbers were checked against the paper's results *table* or its exact
train/test split, sample count beyond what the abstract states, or
statistical-significance reporting. "Matches the abstract" is one level of
verification above "the user's paraphrase" and one level below "checked
against the primary experimental table," which the directive explicitly asks
for and this pass did not have time to do for all eight. Treat every row as
**PARTIALLY VERIFIED**, not VERIFIED, until a table-level read happens.

## Seeds searched but not part of the eight numeric anchors

| Seed | What was found | Note |
|---|---|---|
| Memanto | 2604.22085, "Typed Semantic Memory with Information-Theoretic Retrieval," 89.8% LongMemEval / 87.1% LoCoMo, vector-only (no graph), 13 memory categories | Directive names "Memanto" without a numeric anchor; found and dated within window (2026-04-23). Relevant to Physis's own executable-ontology/typed-relation design as a counter-example: claims graph infrastructure is *not* necessary for SOTA-among-vector-only, which is a testable tension with WorldDB's "+7pp from graph machinery" ablation above -- worth a side-by-side once both are table-verified. |
| MRMS | 2607.04617, "Multi-Resolution Memory Substrate," architecture paper (structured/vector/graph representational axis crossed with short/medium/long-term temporal axis) | No single headline accuracy number in the abstract to anchor against; described as a lightweight prototype demonstration, not a benchmark leaderboard entry. Relevant to Physis's own `R(x) = {r_dense, r_sparse, ..., r_time, r_symbolic}` representation set in the directive -- structurally the closest match of everything found this pass. |

## Relevant older lineage explicitly outside the 2026-02-15–2026-09-15 window

None retrieved this pass -- no search this round intentionally targeted
pre-window lineage (e.g. the original ColBERT/MaxSim line already cited in
E87, or classical bi-temporal-database literature underlying
`TemporalValidity`'s `valid`/`expired` legs). Flagged as a gap, not a finding:
this pass optimized for checking the directive's numeric anchors inside the
window, per the timebox.

## Access failures

None. All ten fetch/search operations this pass returned content; no source
access failures to report.

## Follow-up queue (unvisited within this pass's timebox)

1. Table-level verification (not abstract-level) for all eight anchors above,
   especially resolving the relative-percent-vs-percentage-point ambiguity on
   ActionEngine's efficiency numbers, EAM's 19.6%, and Graph of Skills' 43.6%.
2. Locate the actual "Persistent Semantic Context" paper (title as given in
   the directive did not surface directly) and check the +19pp anchor.
3. Not yet searched at all this pass: semantic filesystems, computer-use
   agents (general, beyond ActionEngine's GUI-agent framing), tool discovery
   beyond SING, executable memory beyond EAM/UaC, graph/state-machine
   planning beyond ActionEngine's state-machine memory, ontology induction,
   context compression (beyond what Physis's own context-compiler section
   already benchmarks internally), semantic digital twins, neuro-symbolic
   robotics, robotic Skill Graphs (note: "Graph of Skills" found here is an
   LLM-agent-skill-retrieval paper, not a robotics skill graph -- these may be
   two different things under similar names; needs disambiguation).
4. Recursive citation-following from any of the eight anchor papers into
   their own related work -- not attempted this pass; the timebox was spent
   on breadth across the eight named anchors, not depth into any one paper's
   citation graph.
5. Cross-check WorldDB's "graph machinery contributes ~+7pp" against
   Memanto's "graph infrastructure not necessary for SOTA" as a genuine
   tension worth a side-by-side table once both are table-verified (noted
   above, repeated here as an action item).

**This is not an exhaustive search of the literature window.** It is a
first, timeboxed pass over the directive's named numeric anchors, each
checked one level deeper (abstract) than the user-supplied summary, with an
explicit list of what remains open.
