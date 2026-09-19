# Core and Pro: the next ten development days

Written 2026-09-15. Schedule: 16–29 September, weekdays only.
Status: proposed implementation sequence; no milestone below is reported complete.

## Outcome

**All application features live in physis-pro. physis-core stays the CLI brain.**

By the end of this cycle, aim to demonstrate one Pro workflow:

> Import a maintenance record, find the relevant evidence, inspect what was
> believed at a particular time, review a possible contradiction, record a
> correction, and recall that correction after restart.

Core supplies reusable mechanisms through a Rust library and a small CLI/JSON
interface. Pro composes and operates them. Research supplies experiments and
controls; it does not require a third product repository.

Plan for one developer: roughly three quarters implementation and verification,
one quarter focused research. Days are work budgets, not delivery promises.
If a prerequisite fails, carry it into the next day and drop stretch work.
Do not compress compatibility work to make the calendar look complete.

## 1. Ownership and architecture

| Responsibility | Core | Pro |
|---|---|---|
| Meaning and evidence | Classification, proposal, typed relations, hypotheses, contradiction records, provenance, revision | Extraction, review queues, domain workflows, feedback policy |
| Time | Explicit validity, evidence history, replay primitives | Date extraction, timeline UI, collection and scheduling |
| Retrieval | Ranking, token budgets, representation interfaces | Workspace indexing, strategy selection, search UX, synthesis |
| Observation | Records, append/read, deterministic serialization | Filesystem/git/browser/process/agent adapters and watchers |
| Interaction | Small CLI and versioned JSON | Product CLI, GUI, TUI, MCP, notebook, agent execution |
| Operation | No product dependency | Providers, connectors, tenancy, deployment, licensing |

Dependency direction: `Pro applications -> Pro services -> Core mechanisms`.
Core must not import Pro. Keep local persistence primitives in Core; continuous
workspace memory is Pro. Keep model transport and automatic model selection in
Pro; optional local representation backends may remain in Core.

Target commands in this document are **proposed**, unless explicitly described
as existing. Pseudocode names express contracts, not assertions that APIs exist.
Use `physis-core ...` for mechanisms and `physis-pro ...` for product features.
Decide ownership of the optional `physis` alias once; do not install two
executables with the same name or route product commands through Core.

## 2. Starting point and research corrections

Source inspected on 2026-09-15; builds and experiments were not rerun for this
plan. Existing uncommitted migration work must be preserved and completed.

| Evidence | Consequence for this cycle |
|---|---|
| [`src/ported`](../../src/ported/mod.rs) contains relocated product modules; [`tests/system_interface.rs`](../../tests/system_interface.rs) imports Pro but invokes `CARGO_BIN_EXE_physis-core` | Finish entrypoints and test migration before adding functionality. A move is not integration. |
| [`observe.rs`](../../physis-core/src/observe.rs) still contains collection adapters | Extract adapters into Pro while preserving the log format and reader API. |
| [Core austerity audit](../../physis-core/docs/CORE_AUSTERITY_AUDIT.md) and [older split plan](../../physis-core/docs/SPLIT_PLAN.md) disagree on product topology | This plan selects two projects. Historical repository retirement/publication work is outside this cycle. |
| [E77 harness](../../benchmarks/conatus/e77_temporal.py) supplies event dates inside prose and queries write-time replay; [`TemporalValidity`](../../physis-core/src/temporal.rs) already has interval fields | Fix the input/query contract and the measurement. Do not infer that all temporal primitives are absent or add a duplicate temporal model. |
| [E86](../../research/E86_PARTICIPANT_BINDING.md) reports gains from binding participants; [E89](../../benchmarks/conatus/E89_NLI_ARM.md) reports additional recall plus adversarial false positives | Test a binding-aware residual route. Rules-first alone does not protect the residual from false positives. |
| [E87](../../benchmarks/conatus/E87_MAXSIM_PROJECT.md) reports that a single shared file can dominate project MaxSim | The follow-up is duplicate-controlled aggregation, not another unchanged MaxSim run. This result is about project grouping, not every query/chunk retrieval task. |
| [E88](../../research/mapper/E88_ANISOTROPY_COVARIATE.md) measures a covariate with limited independent samples | Test interventions before treating anisotropy as a cause or correction. |
| [E21](../../research/RESEARCH_STATUS.md#e21--geometry-as-a-proposer-the-first-positive-that-survives-its-null) leaves second-rater transfer open | Prepare blind review and measure agreement. More training on the original labels cannot settle this question. |
| [E74](../../research/E74_FOUR_HARNESSES.md) reports label differences across harnesses and limited transfer evidence | Keep task success separate from tool success; evaluate priors by held-out session/harness. |
| [E53](../../physis-core/research/E53_TRANSFORM_ANALOGY.md) already tests analogy ranking | Generation and cross-domain transfer remain follow-ups. Do not call transform untested or call its ranking result a generator. |

These are readings of recorded experiments, not fresh benchmark claims. Older
agenda items must be checked against later reports before being reopened.

## 3. Schedule

| Day | Date | Main deliverable | Gate before continuing |
|---|---|---|---|
| 1 | Wed 16 Sep | Complete product command routing | Pro entrypoints exercise moved services |
| 2 | Thu 17 Sep | Extract adapters and preserve state | Core standalone build; restart and migration parity |
| 3 | Fri 18 Sep | Reliable evidence retrieval | Equal-budget comparison with answer/citation checks |
| 4 | Mon 21 Sep | Explicit event-time and knowledge-time queries | Late-arrival, correction and replay fixtures pass |
| 5 | Tue 22 Sep | Contradiction routing experiment | No regression on locked consistent pairs; report abstentions |
| 6 | Wed 23 Sep | Pro evidence review and blind proposal packet | Corrections persist; suggestions cannot silently apply |
| 7 | Thu 24 Sep | Agent memory and outcome contract | Tool exit cannot mark the task successful |
| 8 | Fri 25 Sep | Complete maintenance-record workflow | CLI/MCP/UI agree after restart |
| 9 | Mon 28 Sep | One selected research follow-up | Controlled result, including a negative or inconclusive result |
| 10 | Tue 29 Sep | Stabilization and release candidate report | Boundary, compatibility and workflow checks pass |

**Beyond this cycle.** Two external design prompts — an intention-relative
semantic TUI (ASTRA) and an extension adding situated salience, persistent
identity and editable semiotics (VASTRA) — were evaluated against the codebase
on 2026-09-16. They are not on this schedule. Their map (what already exists,
what is new, the falsification order, and the prerequisites) is recorded in
[`2026-09-16-astra-vastra-programme.md`](2026-09-16-astra-vastra-programme.md),
and the one item that touches this cycle is already implied by Day 3: the Pro
MCP build and the pack path are prerequisites for anything that acts on a
selected semantic region.

### Day 1 — finish the relocation and route the commands

**Touch:** Pro `Cargo.toml`, `src/main.rs`, `src/ported/`, moved integration
tests; Core CLI only to finish removing product dispatch.

1. Record both Git revisions and dirty-tree diffs. Inventory moved files,
   callers, feature requirements, assets and tests before further edits.
2. Add Pro command adapters for workspace operations, MCP, notebook, action
   execution and Studio. Reuse existing Pro commands where semantics match.
3. Put web/TUI/MCP dependencies behind the appropriate Pro feature gates.
4. Update moved tests to invoke actual Pro binaries. An old executable under
   `target/` must never count as evidence for the new source.
5. Keep `src/ported` as temporary migration structure; rename/consolidate only
   after parity is demonstrated.

```text
Pro.main(args):
    command = parse_pro_command(args)
    services = open_pro_services(command.workspace, command.state)
    match command:
        System(request) -> services.workspace.handle(request)
        ServeStdio      -> mcp_adapter.serve(services)
        Notebook(query) -> services.notebook.answer(query)
        Studio          -> web_adapter.serve(services)
        Existing(cmd)   -> existing_pro_dispatch(cmd, services)

Core.main(args):
    request = parse_mechanism_command(args)
    result = core_engine.execute(request)
    emit_versioned_json_or_text(result)
```

**Acceptance:** CLI and MCP both perform write/read round trips through Pro;
errors have nonzero process status or protocol error results as appropriate;
machine-readable stdout contains no startup banners. Removed Core commands
produce an explicit unsupported-command result. Core remains usable without a
Pro installation. Record all moved routes, including anything still unwired.

### Day 2 — cut the remaining product dependencies and preserve data

**Touch:** Core `observe.rs`, relevant library exports; Pro observation adapters,
workspace service and feature configuration; boundary CI checks.

Keep observation records and storage mechanics in Core. Move `watch_fs`,
`watch_git`, `watch_shell`, `watch_agent`, `watch_proc`, `watch_browser` and
their external-tool integration into Pro. Audit each caller and fixture.

```text
Pro.collect(adapter, workspace):
    cursor = workspace.load_cursor(adapter.id)
    batch = adapter.collect(cursor)
    for item in batch:
        observation = normalize(item, source=adapter.id)
        workspace.append_if_new(observation.source_key, observation)
    workspace.commit_cursor_after_durable_append(batch.next_cursor)

Core.append(log, observation):
    validate_schema(observation)
    persist_record_without_product_policy(log, observation)

CI.boundary():
    inspect_cargo_dependency_graph(core_only=True)
    reject_product_dependencies_and_exported_product_modules()
    check_core_command_allowlist()
    build_core_without_pro_checkout()
```

**Acceptance:** retries do not duplicate records; interrupted ingestion does
not skip records; legacy logs retain IDs, order and provenance; a copied fixture
produces the same query results after migration and restart. Preserve distinct
stores until their semantics are explicitly mapped—sharing a service does not
justify blindly merging Pro's causal ledger and the workspace observation log.

### Day 3 — repair retrieval using an equal-budget experiment

**Problem:** returning fewer tokens is useful only if the required evidence
remains present. Existing answer comparisons use character budgets in parts of
the harness; report actual tokens and align budgets before making token claims.

**Touch:** Pro workspace retrieval/notebook; Core ranking/packing only for a
reusable correction; `benchmarks/retrieval/` fixtures and harness.

Freeze query/evidence-span cases covering symbols, paraphrases, Italian notes,
negation, duplicated headers, changed files and answers crossing chunk borders.
Use a development split to tune; freeze the final held-out cases before scoring.

```text
arms = [file_heads_at_budget, bm25_at_budget, current_pack, candidate_pack]
for case in held_out_cases:
    for arm in arms:
        pack = arm(case.query, frozen_corpus, same_token_budget)
        assert token_count(pack.text, tokenizer) <= budget
        score_evidence_spans(pack, case.required_spans)
        score_citation_resolution(pack, corpus_revision)
        measure_latency_and_tokens(pack)
        if configured_answer_model:
            score_answer_with_frozen_rubric(answer(pack), case)
report_paired_deltas_and_counts(grouped_by_source_or_project)
```

**Gate:** all citations resolve and budgets hold. On locked regression cases,
lose no required evidence; on broader evaluation, report quality/cost tradeoffs
and uncertainty. A quality improvement needs evidence against the strongest
simple baseline, not only against the current implementation. If inconclusive,
keep the baseline and record the misses. Missing model arms are NOT MEASURED.

### Day 4 — distinguish when something happened from when we learned it

**Problem:** replay of recorded knowledge and a query about past world state are
different operations. E77 puts world dates in prose; that does not test a typed
valid-time write path. Existing `is_valid_at` also needs auditing before reuse
because one time argument cannot express both questions independently.

**Touch:** Core temporal/evidence/replay contracts; Pro import normalization and
timeline service. Extend existing types rather than introducing a second graph.

Proposed query: `state(valid_at=T, known_at=K)`. Retain the current meaning of
`replay --at K`; introduce explicit flags/operations for the second question.
Document interval endpoints, unknown dates, correction precedence and timezone
normalization. Do not infer a known world date from an ingestion timestamp.

```text
ingest_evidence(raw):                    # Pro
    parsed = extract_explicit_dates_and_source_spans(raw)
    append_evidence_revision(
        proposition_id=resolve_subject(raw),
        valid_interval=parsed.interval_or_unknown,
        recorded_at=clock.now(),
        source=raw.reference,
        supersedes=explicit_previous_evidence_id_or_none)

state(valid_at=T, known_at=K):           # Core
    revisions = log.records_with_recorded_at_at_most(K)
    visible = resolve_corrections_as_known_at(revisions, K)
    applicable, unknown_time = partition_by_validity(visible, T)
    return evaluate(applicable), unknown_time, supporting_event_ids
```

**Gate:** a January reading learned in March is absent from a February
knowledge query but available to an April query about January. Arrival-order
permutations with the same final evidence produce the same retrospective
world-state answer; intermediate knowledge answers may legitimately differ.
Test interval boundaries, later corrections, unknown dates and restart.
Legacy records retain their documented meaning; do not invent missing dates.

### Day 5 — improve contradiction recall without restoring false alarms

**Research question:** can explicit participant/time binding restrict the NLI
residual enough to preserve the rule baseline's precision while finding more
implication and dictionary-gap cases?

**Touch:** `benchmarks/conatus/` first; Pro routing only after the gate. Reusable
deterministic comparison mechanics may enter Core through its promotion rule.

Use E86/E89 as baselines, rerun in the same evaluation. Add a new held-out set
covering different owners, different objects, nonoverlapping time intervals,
unit conversions, quantifier scope, negated predicates and paraphrases. Existing
published fixtures are regression sets, not a fresh blind holdout.

```text
compare(a, b):
    scope = bind_subject_predicate_object_time(a, b)
    if scope.proven_different:
        return NOT_COMPARABLE(scope.evidence)
    if scope.unknown:
        return REVIEW_REQUIRED(reason="unresolved binding")
    if bound_rule_matches(a, b):
        return CONTRADICTION_CANDIDATE(rule_id, exact_source_spans)
    if nli_available and residual_class_allowed(a, b):
        return REVIEW_CANDIDATE(nli(a, b), scope, exact_source_spans)
    return UNKNOWN
```

**Gate:** compare rules-only, NLI-only, naive union and routed arms. Report
false-positive counts, contradiction recall, abstention/coverage and latency per
case class. Unknown binding must not be scored as a correct negative. Require no
new false positives on locked consistent fixtures and a held-out recall gain
before promoting the route; small samples remain provisional. Model scores alone
never revise a belief. If the route fails, ship rules plus explicit review for
the residual. Timebox this investigation to one day.

### Day 6 — turn evidence into a reviewable Pro experience

**Touch:** Pro notebook/search, review service and existing UI. Reuse one view
for source spans, current standing, historical standing and proposed changes.

```text
review_card(subject, query_time):
    return {
        sources: resolve_citations(subject),
        current: core.state(subject, now, now),
        historical: core.state(subject, query_time.valid, query_time.known),
        candidates: pro.propose_changes(subject),
        limitations: missing_binding_or_time_or_model_information
    }

apply_review(decision, expected_revision):
    reject_if_subject_changed(expected_revision)
    event = append_attributed_decision(decision)
    core.apply_explicit_revision(event)
    return replay_and_current_readback(event.subject)
```

**Research deliverable:** export a blind, stratified second-rater packet for E21
(target: 60 items across populated and sparse cells, plus an ambiguity option).
Hide original labels and candidate rankings while collecting independent labels;
evaluate agreement and proposer top-k containment separately. A second independent
human is preferred for human-use claims. A model rater must be identified and
does not substitute for a human replication. Prepare the packet without waiting
for a rater; if labels do not arrive, report this experiment NOT MEASURED.

**Gate:** a review survives restart, cites the source and attributed decision,
and updates current state consistently with replay. Merely viewing a proposal
does not apply it. Do not rewrite the domain/mode inventory during this cycle.

### Day 7 — make agent memory about outcomes

**Touch:** Pro workspace history, action service and observer adapters. Keep
the existing observation contract; add explicit task/run/verification references.

```text
before_run(task):
    prior = recall_verified_outcomes(task.scope, task.intent)
    prediction = estimate_from_compatible_records(prior)
    append_intent(task.id, prediction, evidence_ids=prior.ids)

after_tool(run, tool_result):
    append_tool_result(run.id, exit_code=tool_result.exit_code)
    set_task_outcome(run.task_id, UNVERIFIED)

verify_task(task, artifact):
    checks = evaluate_declared_acceptance(task, artifact)
    if checks.incomplete:
        return UNVERIFIED
    return append_outcome(task.id, checks.verdict, checks.evidence)
```

**Research:** test whether remembered outcomes change a subsequent decision.
Replay matched tasks with memory enabled/disabled under the same harness, model,
budget and fixtures. Include a misleading/stale-memory control. Measure repeated
failed attempts, verified task completion and context cost together. Tool-error
prediction is a separate endpoint; E74 does not establish task-success gains.
Deduplicate sessions before splitting, and hold out whole sessions/workspaces.

**Gate:** a successful shell command cannot close an incomplete task. A stale
memory cannot overwrite newer evidence. Keep borrowed priors conditional on
explicit provenance and label compatibility. If no decision benefit is measured,
retain history as provenance and make no learning claim. Worker-launch expansion
is deferred; reservations are not worker execution.

### Day 8 — demonstrate the complete Pro workflow

Use a local fixture representing a maintenance folder and two machines. Include
an old threshold, a dated correction arriving late, an unrelated machine's
opposite reading, and a restart between ingestion and review. Build on existing
industrial and document services; no new live machine connector is required.

```text
fixture -> Pro.import -> Core.observations
question -> Pro.retrieve -> cited_evidence
cited_evidence -> Core.temporal_query -> Pro.review_card
review_decision -> Core.revision -> Pro.history
restart -> same_ids_and_evidence -> corrected_answer
```

**Gate:** CLI, MCP and UI return consistent evidence and standing; tenant or
workspace scope is enforced by the Pro service; retries preserve IDs. The two
machines must never become a contradiction solely because readings differ.
Generate a reproducible demo script and an expected-output fixture. Demonstrate
the unavailable-model path as well as the configured-model path.

### Day 9 — select one research follow-up from the remaining misses

Spend at most one day. Select by observed product failure, not novelty. Prepare
all experiment specifications, but run only the highest-value one this cycle.

| Trigger | Experiment | Controls and stopping rule |
|---|---|---|
| Directory grouping follows shared configuration files | **E87 follow-up:** deduplicate/cluster near-duplicate files, compare pooled, maximum, robust per-file vote and bounded top-k aggregation | Same frozen corpus and label granularity for all arms; raw and deduplicated views; label permutation; hold out whole project families. Stop if gains vanish when duplicate families are separated. Mixed-language subtrees need multilabel truth, not a forced root label. |
| Proposer remains unstable across representations | **E88 follow-up:** unchanged vectors vs training-fitted centering/component removal or clipping | Fit transforms only on training items; multiple seeds; permuted labels; matched random component-removal control; second representation when available. Lower anisotropy without better held-out proposal quality is not success. |
| Users need cross-project pattern suggestions | **E53/transplant follow-up:** candidate mapping followed by a deterministic task-specific output check | Popularity baseline, degree-preserving/function-scrambled null, common eligible held-out cases for all arms. Report ranking and valid generated outputs separately. If only ranking improves, ship only a shortlist. |
| Italian retrieval fails on Day 3 | **Representation comparison:** installed multilingual encoder vs current encoder and lexical baseline | Same corpus, tokenizer budget and held-out bilingual cases; record model checksums and fallback identity. Promote only the measured language-specific benefit. |

```text
spec = freeze(question, corpus_hash, split, arms, metric, acceptance, time_budget)
assert known_bad_control_can_fail(spec)
for arm in spec.arms:
    result[arm] = run_on_identical_cases(arm, spec)
report_paired_results_with_denominators(result)
if insufficient_cases_or_missing_arm:
    verdict = NOT_MEASURED_OR_INCONCLUSIVE
elif candidate_meets_predeclared_gate:
    verdict = CANDIDATE_FOR_PRODUCT_TRIAL
else:
    verdict = KEEP_BASELINE
```

No JEPA training, video encoder expansion or large-model dependency belongs on
the critical path. Those require a failure class that simpler interventions do
not address, plus a separately budgeted experiment.

### Day 10 — stabilize and produce a reviewable release candidate

1. Run the independent Core build matrix: library without default features,
   CLI, and optional local embedding backend when installed. Build in an isolated
   checkout/target directory to avoid stale executable evidence.
2. Run Pro's moved integration suites and the relevant feature combinations.
   Resolve duplicate module ownership and stale command/configuration references.
3. Run legacy-state readback, restart, cancellation, JSON/protocol errors,
   scope isolation and the Day 8 demo.
4. Update capability manifests with actual write/read paths. Inspect scanner
   findings; a textual reference is not proof of runtime reachability.
5. Produce an acceptance report: source revisions, dirty changes included,
   commands, feature sets, fixture hashes, failures and unmeasured paths.
6. Update active docs, including stale Core-MCP references in development guides.
   Keep historical research findings as history with links to superseding work.

**Release gate:** Core stands alone; Pro runs migrated surfaces; old evidence is
readable; the complete workflow passes; every promoted research change has its
control and limitations attached. This milestone prepares a candidate—it does
not include deployment, repository archival or publication.

## 4. Research discipline and daily record

Before each experiment, write the question, intervention, strongest simple
baseline, matched control, split, primary metric and pass/fail condition. Store
the specification before seeing held-out results. Keep an experiment identifier
that cannot collide with existing E-series reports.

Freeze source/data/model versions and record changed inputs. Group uncertainty
estimates by independent unit (project, session, document or participant), not
by thousands of correlated chunks. Predeclare one primary comparison; label
additional sweeps exploratory. No interval from a handful of related samples
justifies a broad claim.

An improvement to Core must be reusable without Pro infrastructure. Product
routing and learned operating policy stay in Pro even if the experiment works.
Use the existing [research agenda](2026-09-13-research-agenda.md) and
[resources](../../physis_resources.txt) as leads. Verify original papers before
borrowing specific methods or quoting them; old literature summaries and
philosophical analogies are not experimental evidence.

Each day's note should contain:

```text
intent; core_or_pro_owner; source_revision_and_dirty_diff;
changed_paths; commands_and_feature_sets;
fixture_hashes; observed_result; unresolved_cases;
research_verdict = supported | unsupported | inconclusive | not_measured;
next_action_with_evidence_references
```

Use the workspace history/remember service for development records, with one
consistent store. Keep development verdicts separate from classification-quality
penalties. During migration, the available legacy CLI can serve as the history
reader; after Day 1, verify the Pro command path before switching clients.

## 5. Scope cuts if the schedule slips

Protect Days 1–4 compatibility and evidence correctness, and the end-to-end
workflow. Cut Day 9 first. Keep unresolved NLI cases in manual review, keep the
existing retrieval baseline when research is inconclusive, and ship the rater
packet without claiming agreement was measured. Defer new connectors, worker
launching, model training and UI expansion. A smaller verified Pro workflow is
the acceptance target for this cycle.
