# HUD binding — CONTRACT.md v1 to the HUD first slice

Status: prose spec only, NOT IMPLEMENTED. Binds
`docs/semantic-computer/CONTRACT.md` v1 to the HUD first slice.
No code changes; `check_contract.py` must stay PASS; all
manifest task statuses stay `not_implemented` until a REAL local run.

## HUD first slice — key map

Keys are read-only-first. `o` and `P` never execute. `X` executes
only a registered, authorized, revision-checked capability.
`V` reports validator outcomes; it never invents success.

| HUD key | Meaning | Contract trait | v1 schema object | Pro primitive reused | Evidence | Validator ids |
|---|---|---|---|---|---|---|
| `o` | Objects — resolve intent to local object IDs, inspect world view | `IntentInterpreter.resolve` (candidates/clarification, never guessed execution) + `ObjectView.snapshot` + `Retriever.retrieve` | `physis.event.v1` with `operation: intent.resolve / objects.inspect`; no plan/result from this key | `nav::query` + `nav::read_object` (exact source, scored/grouped hits); `mvp/deps.rs::history_text` read-only past | `dry_run` for fixture slices; `real_local` only after a real local run transcript | `expected_object_ids`, `source_checksums`, `state_revision` |
| `P` | Plan — construct and inspect a proposed plan over registered capabilities | Planner `plan(goal, snapshot, capabilities, weights)` — typed preconditions only, frozen weights, deterministic tie-break by stable capability ID + canonical args | `physis.plan.v1` with `status: proposed`, plus one `physis.event.v1` (`operation: plan.propose`) | `capability.rs` registry projected onto v1 capability fields; `ported::act` recall as reliability prior source (read, not duplicated) | `dry_run` (plan inspection is never execution evidence) | `capability_compatibility`, `source_revision`, `pre_postconditions` presence |
| `X` | Execute — run one permitted step, log intent before + result after | `CapabilityAdapter.execute` + `EventStore.append` (intent event, then action-result event incl. failures) | `physis.event.v1` pair (`step.intent`, `step.result`); registered argv `["cargo", "test"]` + bound repo + env policy in `input` (bound path is a capability binding, never a test-name filter) | `ported::act` bounded invocation (idempotent-or-token retry only); `mvp/log.rs::Transition` is the projection target, not a second log | `real_local` for fs/shell families; `real_remote`/`real_physical` stay `not_implemented` until exercised there | `build_exit`, `test_report`, `source_revision`, `explicit_authorization`, `model_checksum`, `material_compatibility` |
| `V` | Validate — check effects with registered validators, rebuild view from same log | `Validator.validate` + `ObjectView.apply/invalidate` (checkpoint only after append succeeds) + benchmark result assembly | `physis.event.v1` (`step.validation`, status verified/failed/unknown/not_run) + `physis.benchmark-result.v1` (paired arm/ablation, execution_mode, event_ids, validations[].evidence_event_ids) | `mvp/deps.rs::bench_text` (measured scan/query/propagation) + `contradictions_text`/`dependents_text`; `TransitionLog::read_all` for rebuild-from-log | Modes kept distinct: `simulated`/`dry_run`/`real_local`/`real_remote`/`real_physical`; fixture-only is `dry_run`, never `real_local` | Per-task manifest validators (`approved_grouping`, `raw_sources_preserved`, `rename_manifest`, `recorded_task_reference`, `resolved_object_id`, `application_ack`, `browser_navigation_ack`, `artifact_checksum`, `remote_validation`, `four_completed_parts`); success needs every declared validator `verified` |
| `M` | Impossible machine — read-only view of the closed E52→E65 singular-series chain plus live artifact overlay | none (no trait; observation display only — values grade by gates G1/G2/G3, Heuristic never certifies) | none (no event emitted, nothing appended) | `mvp/deps.rs::impossible_text` (baked-in closed record from `physis-core/research/impossible_machine/SERIES_SUMMARY.md` + live `experiment{61,62,63,64,65}_results.json` fits + graded machines/propositions/disagreements/verdict via core `machines::ImpossibleReport`) | `dry_run` equivalent: display only; live fits shown alongside the closed rows, never replacing them | `series_closed` (E62/E63 inside 2C2 ±0.15), `boundary_kmax40`, `no_half` (no 1/2 in generation/scoring/gates) |

## Projection onto the existing log (NOT a duplicate engine)

v1 `physis.event.v1` projects onto `src/mvp/log.rs::Transition`:
seq/timestamp/state_before/state_after from recorded_at + content
hashes (`occurred_at` may be null, never fabricated); intent/action
from operation + run_id/step_id; CompactDelta path lists + counts
(`no_change` still recorded); validation/outcome verified/failed/
unknown/not_run; provenance raw_ref + sha256 (URI alone insufficient);
slice SliceCost (considered/selected, candidate/final tokens, provider,
paths); uncertainty carried, not threshold-tuned. Audit existing
`ledger`, `operational`, `address`, `system`, `act` types first.
No competing memory engine.

## 3-layer rule

1. Observers record. Append to the existing log, reference raw bytes,
   dedupe by source key. Never revise beliefs or choose actions.
2. Beliefs revise. Views replay the log into snapshots with
   `{revision, objects, facts, checkpoint}`; `invalidate()` propagates
   source changes. Views never execute or reinterpret intent.
3. Interpreter replaceable. Model sits only behind
   `IntentInterpreter.resolve` (typed-schema arguments, never a shell
   program or path). Fixture parser default; model edge swappable.
   The model never decides success — the environment does, via validators.

## Refusal list (the HUD first slice will NOT)

- No FS interception: no hooks/filters/overlays; only registered
  capabilities on bound handles.
- No model-as-index: retrieval is the shared hybrid scorer; the model
  never substitutes for index or log.
- No contradiction resolution: contradictions surfaced with evidence,
  never auto-merged.
- No unattended physical actuation: physical adapters need explicit
  capability + authorization + material/geometry + machine-state
  validation; no print job ordered here.

## Boundary checks (worker enforces; check_contract.py skeleton)

- Plan: unique step_ids, requires_steps subset of seen (acyclic, in
  order), registered capability version, non-empty authorization_ref,
  bound objects at state_revision, non-empty plan.
- Result: success only if validator set exactly matches required set,
  each verified, each with resolving non-empty evidence_event_ids.
- Manifest invariant: `scaffold_not_executed`, every task
  `not_implemented`, until a real local run transcript plus a
  rebuild-from-log comparison exists.
