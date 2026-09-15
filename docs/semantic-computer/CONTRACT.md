# Semantic computer: implementation skeleton v1

Status: contract and scaffold, not a working product. Implements the boundaries
in [the directive](RESEARCH_DIRECTIVE.md). Initial owner: coordinator. The Pro
worker takes implementation ownership after its migration gate passes.

## First complete slice

Input: `find the thesis diagrams` or `build and test Physis` in an explicitly
selected local workspace. Resolve intent to local object IDs and registered
capabilities; construct and inspect a plan; execute permitted adapters; append
observations; validate effects; rebuild the world view from the same log.

Start with a deterministic intent fixture and an optional model interpreter
behind the same interface. An unknown or ambiguous request yields candidates
or a clarification requirement, not guessed execution.

All application modules are in Pro. Reuse Core observation/evidence/retrieval
primitives where they fit. The event envelope below is a Pro operational record
to project onto the existing Core log, not permission to create a competing
memory engine. Audit existing `ledger`, `operational`, `address`, `system` and
`act` types before implementing another type with the same semantics.

## Modules and worker ownership

| Module | Responsibility | Implementation owner |
|---|---|---|
| contracts | Wire schemas, typed refs, conditions and outcomes | Coordinator skeleton, then Pro |
| observation adapter | Append to existing Core log, reference raw content, deduplicate source events | Core primitives + Pro adapter owners |
| object views | Replay/checkpoints, source revision checks, representation references | Pro |
| capabilities | Registered adapter IDs, pre/effects/validators, versioned reliability | Pro |
| intent | Goal/candidate objects; deterministic fixture parser, optional model edge | Pro |
| planner | Search over applicable registered capabilities; deterministic tie-breaking | Pro |
| executor | Preconditions, bounded adapter invocation, postcondition evidence, local repair | Pro |
| benchmark | Common task/result contracts, baseline arms, paired comparisons | Coordinator skeleton, then research |

## Stable wire identities

- `object_id` identifies a resource independently of its display name or current
  path. `revision` binds a plan to observed state. Preserve move/rename evidence.
- `capability_id` and `version` identify a registered implementation. The model
  supplies arguments to a typed schema, never a shell program or executable path.
- `event_id`, `run_id`, `plan_id`, `step_id` join intentions and observations.
- `raw_ref` points to immutable local source bytes with a checksum. A URI alone
  is not enough to replay after the original changes. Content retention policy
  must be explicit; absent content is a replay limitation.
- `occurred_at` can be unknown; `recorded_at` is mandatory. Views expose both
  `valid_at` and `known_at` where applicable. Do not fabricate occurrence time.

## Planning and action contract

```text
resolve(intent, workspace_snapshot) -> GoalResolution
    resolved(goal, bound_objects) | ambiguous(candidates) | unsupported(reason)

plan(goal, snapshot, capabilities, weights) -> ProposedPlan
    select only capabilities with satisfiable typed preconditions
    compare normalized risk/latency/cost, freeze weights for benchmark
    tie-break by stable capability ID and canonical argument encoding
    preserve object revisions, required authorization and expected effects

execute_step(plan, step):
    require applicable authorization for bound resources/action
    require current revision == step.expected_revision
    require all preconditions mechanically verified
    append execution-intent event
    invoke registry[capability_id, version](validated_arguments)
    append action-result event, including transport/adapter failures
    validate observed effects using registered validators
    append validation event with source evidence
    update reliability only from comparable validated terminal outcomes
    update/checkpoint materialized views after append succeeds
```

An adapter receives a resolved resource handle and typed arguments. Shell builds
use registered argv templates, a bound repository and environment policy;
browser/robot targets use registered handles. Rechecking state is required even
when planning was deterministic. Log partial failure before repair. Retry only
idempotent steps or steps with an explicit deduplication token/compensation rule.

## Minimal implementation interfaces (pseudocode)

```text
trait EventStore:
    append(event, source_dedup_key) -> event_id
    read_after(sequence) -> events

trait ObjectView:
    apply(event) -> changed_objects
    snapshot() -> {revision, objects, facts, checkpoint}
    invalidate(changed_source_ids) -> affected_view_ids

trait IntentInterpreter:
    resolve(text, candidate_objects, allowed_capabilities) -> GoalResolution

trait CapabilityAdapter:
    descriptor() -> Capability
    execute(bound_arguments, operation_token) -> AdapterResult

trait Validator:
    validate(expected_effects, actual_state, adapter_result) -> ValidationResult

trait Retriever:
    retrieve(query, snapshot, budget) -> cited_objects_and_dependencies
```

Skeleton artifacts:

- [JSON Schema](../../schemas/semantic-computer/v1.schema.json): events,
  capability descriptors, plans, and benchmark results.
- [Benchmark manifest](../../benchmarks/semantic-computer/manifest.json): initial
  task families, all required arms/ablations, metrics and evidence status.
- [Contract tests](../../benchmarks/semantic-computer/check_contract.py): validate
  example payloads and reject known-bad result/plan data. They do not prove an
  adapter, planner or model works.

## Evidence levels

`proposed -> implemented -> fixture_validated -> real_machine_validated`.
Keep `simulated`, `dry_run`, `real_local`, `real_remote` and `real_physical`
execution modes distinct in results. A plan for four brackets is not a printed
part; accepted printer submission is not a validated completed print.

An effect can be `verified`, `failed`, `unknown` or `not_run`. A task succeeds
only when its declared validators pass. A missing arm is missing, not zero.
Record denominator and outcome for every attempted task; bootstrap/group tests
by the independent task family, machine or session appropriate to the design.

## Handoff to workers

1. Pro completes CLI/MCP migration, then binds this contract to existing services.
2. Core removes application collectors but retains reusable log/replay semantics.
3. Research validates primary sources and typed fixtures, then adopts the result
   schema for controlled evaluation. Do not tune final holdout thresholds.
4. Coordinator reviews contracts and integrated behavior before claiming success.

Next implementation packet must produce an actual local run transcript plus a
rebuild-from-log comparison. Browser, remote and physical adapters remain
explicit unimplemented tasks until exercised in their real environments.
