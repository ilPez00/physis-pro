# Research directive: the semantic operational computer

User directive, adopted 2026-09-15. This extends the active Core/Pro plan.
Application composition and operation belong to Pro; Core remains the reusable
CLI brain. The coordinator builds the shared skeleton; Sonnet workers implement
and research within assigned ownership.

## Objective

Use **Physis to remake the computer as a semantic, inspectable, deterministic
operational substrate** over the existing OS, filesystem, shell, browser,
applications, network devices and robots.

The user should express intents such as:

```text
find the thesis diagrams
organize these papers by experiment
continue what I was doing yesterday
open the relevant project
build and test Physis
print four of these brackets
move this workflow to another machine
```

The user need not know filenames, directories, application boundaries, APIs,
device addresses or exact commands. Internally those bindings remain explicit,
inspectable and validated.

```text
intent -> objects -> state -> capabilities -> plan -> execution
       -> observation -> updated map
```

Do not build an LLM operating system where the model repeatedly looks at the
screen and improvises. LLMs may interpret ambiguous intent and propose structure;
execution, state, provenance and validation should be mechanical wherever possible.

## Research first, but briefly; continue during implementation

Search relevant work from the last seven months: **2026-02-15 through
2026-09-15** for the initial pass. Prioritize arXiv and use Emergent Mind for
discovery. Recursively follow related papers and implementations. Record search
coverage and remaining references; finite discovery is not proof of exhaustiveness.
Keep relevant older methods as explicitly dated lineage outside the main window.

Seeds: Persistent Semantic Context, ActionEngine, Executable Agentic Memory,
Graph of Skills, SING, ReFind, WorldDB, User as Code, MRMS, Memanto, robotic Skill
Graphs, semantic filesystems, computer-use agents, tool discovery, executable
memory, graph/state-machine planning, ontology induction, context compression,
semantic digital twins and neuro-symbolic robotics.

Extract **methods, equations, datasets, ablations and numerical results**, not
only summaries. Maintain a research table mapping useful mechanisms to Physis,
with the original paper, implementation and precise experimental setting.

### Supplied empirical anchors — UNVERIFIED until checked in primary sources

| Work | User-supplied anchor to reproduce or challenge |
|---|---|
| ActionEngine | 95% success vs 66%; approximately one LLM call; 11.8x cheaper; 2x faster |
| Executable Agentic Memory | Up to +19.6% AndroidWorld performance; 6x lower token cost; 2.8 seconds mean latency |
| Graph of Skills | +43.6% reward; -37.8% input tokens; skill libraries of 200–2,000 |
| SING | 7,471 tools; up to +59.8% Global Recall@5, +28.9% downstream success, -99.8% exposed schemas |
| Persistent Semantic Context | +19 percentage-point accuracy; environmental semantic knowledge remains a dominant failure source |
| ReFind | Raw-history iterative lexical retrieval 58.2 vs 53.2 for the strongest compared graph/tree baseline |
| User-as-Code | 78.8% LoCoMo recall; approximately 99% aggregate-query accuracy where retrieval systems fell to 6–43% |
| WorldDB | 96.40% LongMemEval-s; graph machinery contributes approximately +7 percentage points independently in ablation |

These anchors do not prove the Physis architecture. Check whether each delta is
relative percent or percentage points, its baseline, sample count, model, split,
latency/cost definition and ablation conditions. Correct the table with sourced
findings; preserve the supplied claim as provenance. **Never discard the raw
substrate.**

## Core representation

Every observation enters an immutable event log:

\[
e_t=(t,source,actor,object,operation,input,output,state,provenance)
\]

Never rewrite history. Other state is a materialized view over the log. Record
event identity, schema version, relevant time semantics and source references
so a view can be invalidated and rebuilt. Preserve raw inputs with appropriate
local scope rather than treating a lossy embedding or summary as the source.

For each object:

\[
R(x)=\{r_{dense},r_{sparse},r_{ngram},r_{position},r_{graph},r_{time},r_{symbolic}\}
\]

Cross-representation agreement is a **hypothesis**, not a trust rule. For a
candidate relation, test:

\[
I_{ij}=\frac{1}{K}\sum_k z(s_k(i,j))-
\lambda Var_k[z(s_k(i,j))]
\]

where each strength is normalized under a specified training/reference
distribution. Account for dependence between representations and missing views;
do not manufacture independent votes from correlated variants of one model.
Test whether high agreement predicts held-out usefulness. Existing negative
Physis findings on cross-model agreement must be part of the comparison.

Hybrid retrieval candidate:

\[
Score(x|q)=\alpha Dense+\beta BM25+\gamma Graph+\delta Temporal+
\epsilon Reliability+\zeta TaskContext
\]

Learn or benchmark coefficients on development data. Always compare plain BM25
and plain dense retrieval; include a staged/routed alternative if raw score
scales make weighted fusion misleading. Freeze final thresholds before scoring.

## Executable ontology

Represent a capability as:

\[
S=(P,A,E,V,C,R),\qquad E(S_i,state)\models P(S_j)
\]

P: preconditions; A: executable action; E: expected effects/postconditions;
V: validator; C: cost; R: risk/reliability metadata. Expected effects justify a
candidate composition; measured state must satisfy the next step's preconditions
at execution time. An expected effect is not an observed fact.

Affordances include:

```text
file -> can_read
git_repo -> can_build
browser -> can_navigate
printer -> can_print
STL -> printable_on
machine -> exposes_capability
```

Natural language resolves into this graph; it does not directly operate hardware.
Model-provided strings must not become arbitrary shell commands.

Use Beta reliability as a candidate estimator:

\[
p_s\sim Beta(1+n_{success},1+n_{failure})
\]

For risky operations, compare lower-confidence reliability with raw success
rate. Count validated outcomes, distinguish unknown/cancelled runs, and condition
or stratify on meaningful environment/version differences. The selection rule
and prior are benchmark parameters, not proof of safety.

## Context compiler

Compile only the relevant world slice:

\[
\max\sum_i x_i U(c_i,q)\quad\text{subject to}\quad
\sum_i x_i Tokens(c_i)\le B
\]

Utility includes relevance, dependency coverage, reliability, freshness and
novelty. Keep cited object/event IDs and dependencies. Benchmark against full
context, BM25, dense RAG and graph-only retrieval at declared budgets.

## Operational loop

```text
observe(event):
    append_to_raw_log(event)
    update_sparse_index(event)
    update_embeddings(event)
    update_temporal_index(event)
    candidates = propose_relations(event)
    for relation in candidates:
        evidence = compare_across_representations(relation)
        if passes_threshold_and_controls(evidence):
            materialize_relation(relation, provenance=evidence)
    invalidate_views_depending_on(changed_sources)
```

Index updates are derived, retryable work; a failed index update cannot erase
the raw observation. Persist the view version and checkpoint. A statistical
control is evaluated by a frozen protocol, not rerun opportunistically until a
single live relation passes. Unproven relations remain proposals.

```text
execute(intent):
    goal = parse_intent(intent)
    state = retrieve_relevant_state(goal)
    skills = retrieve_capabilities(goal, state)
    plan = graph_search(skills,
                        require_preconditions=True,
                        minimize=(risk + latency + cost))
    for step in plan:
        assert preconditions(step)
        result = step.execute()
        assert_or_measure_postconditions(step, result)
        record(result)
        if failure:
            attempt_local_repair()
            update_skill_reliability()
    return verified_result
```

Normalize or explicitly weight heterogeneous risk/latency/cost terms. Recheck
state versions immediately before action; log intent before execution and the
actual result after it, including failures. Limit local repair attempts and
avoid repeating non-idempotent effects. The operational plan must preserve
existing authorization; this directive does not order an actual print job or
authorize unattended physical actuation. Physical adapters need explicit
capability, material/geometry and machine-state validation.

Prefer local repair over complete replanning when the repair is applicable.
The model must not decide whether an operation succeeded when the environment
can tell us directly.

## Build the experiment

Instrument every operation: state, retrieved objects, selected skills, graph
path, tokens, model calls, latency, failures and independently validated outcome.

Create real computer tasks spanning filesystem, shell, Git, browser, documents,
applications, cross-device execution and at least one physical-machine class.
Track simulator/dry-run evidence separately from real physical execution.

Compare:

```text
LLM + raw tools
dense retrieval
BM25
structured graph
Physis hybrid
```

Measure task success, retrieval Recall@K/nDCG, token count, LLM calls, p50/p95
latency, execution steps, recovery success, ontology precision, stale-context
errors, structure-building cost and cross-machine transfer. Include the cost of
building/updating structure, not only its warm query cost.

Use McNemar's test for paired task success, Wilcoxon signed-rank for appropriate
continuous paired comparisons when distributional assumptions are unsafe, and
bootstrap 95% confidence intervals plus effect sizes. Document ties, missing
arms, clustered tasks and low sample counts; do not apply a test mechanically
when its pairing or independence requirements are absent. Hold out tasks and
freeze thresholds before final evaluation.

Run ablations:

\[
\Delta_j=Metric(full)-Metric(full\setminus component_j)
\]

Remove graph structure, ontology, temporal information, positional/order
information, embeddings, raw lexical search, executable pre/postconditions and
cross-representation agreement. Declare metric direction: a positive delta for
cost/latency is a cost increase, not an improvement.

## End state and first increment

Resources describe what they are, what they can do, how they relate, how reliable
the relations are, and how they compose. Applications, files, APIs, machines and
robots are surfaces over the same operational semantic substrate.

The user specifies **what**. Physis resolves **what exists and what can
transform it**. Deterministic machinery executes **how**. Models remain
replaceable interpreters at the edges.

Build the smallest complete version on a real machine first: a local fixture
workspace with observed files/projects, typed capabilities, deterministic plans,
mechanical validators, a durable event log and a rebuildable view. Expand the
same contract to browser, cross-device and physical-machine tasks after that
slice passes. A fixture-only slice is not the final benchmark's coverage.

Research continuously while implementing. Adopt a better existing method with
attribution when supported by evidence. Turn each claimed Physis difference into
an explicit benchmarkable hypothesis.
