# Physis as the impossible computer — agent-native semantic world

Date: 2026-09-19. Status: proposed development programme; this document is the
**sequencing authority** for the agent-native workspace. It supersedes the
sequencing and UI-expansion priority of
[the agentic workspace programme](2026-09-18-agentic-workspace.md). That plan
retains its market analysis, packaging experiments, Settings memory manager and
primitives editor, and its P0–P5 gates; those gates still apply where this plan
touches the same surfaces. Nothing here is a shipped capability; planned
acceptance gates are not shipped capabilities.

Goal: one TUI in which objects, actions, relations, deltas, intent, agents and
machines are the same navigable world. Agents are first-class, visible,
inspectable, callable, interruptible, routable and composable from inside the
interface. Expensive reasoning (JEV, Hermes, OpenCode, remote models) produces
structured output that crystallizes into deterministic Physis structure. The
whole machine remains usable offline, and every model-dependent surface reports
its mode instead of failing silently.

The MVP is one coherent loop in `physis-hud`:

```text
discover objects → classify → graph → semantic zoom → select object
→ inspect relations/provenance → invoke agent (@jev) → structured result
→ task graph visible → similarity suggestions → perform action
→ compute delta → preview propagation → verify → navigate again
```

---

## 1. Best possible outcome

A person works inside one semantic computational world. They travel from the
highest abstraction (life, goals, projects) to the lowest implementation detail
(function, expression, machine instruction) without changing interface. Agents
live in the same world as objects: selecting one shows its state, host,
capabilities, context, tools, cost, permissions and current task. A conversation
with an agent is an object. Every handoff is an object/action/delta. Nothing
important is invisible orchestration.

The optimization target is **repeated cognition**, not tokens. When expensive
reasoning has solved something, the result becomes structure — a relation, a
rule, a claim, a template, an invariant — and Physis needs JEV less for the
next identical situation.

What must remain true at every step:

- Files are objects. Applications are projections.
- Commands are actions with preconditions, effects, cost, reversibility and a
  verification path.
- Devices and machines are objects with typed actions.
- Agents are objects with identity, capabilities, context, state, permissions,
  cost and current work.
- Conversations, claims and tasks are objects.
- Models are interchangeable cognitive tools; JEV is a callable high-value
  reasoning agent whose useful output can be crystallized.
- Hermes is a resident conversational/operator agent. OpenCode is a resident
  implementation agent. Verifiers are callable agents.
- Physis owns the world model. The human owns intent.

---

## 2. What already exists (reuse map)

This is a code-inspection table, not a build or performance certificate. Line
references are to the checkout at `eef298f` plus working tree.

| Brief concept | Existing implementation | State |
|---|---|---|
| Object, identity, provenance | `src/nav/mod.rs`: `PhysisObject` (:103), `ObjectId` = origin + content hash (:70), `ObjectKind` (:42) | Shipped |
| Typed relation + origin | `nav::Relation` (:1149) with `RelationOrigin{User,Deterministic,Inferred}` + `RelationStatus`; sled `nav:relations`; `relations_of`, `relation_id` | Shipped |
| Plural abstraction parents | `nav::semantic_parents` (:1286, `fs`/`kind`/`note`) | Shipped |
| Navigation façade | `src/nav/workspace.rs`: `NavWorkspace` (index, store, lazy embedder, query, resolve, read, pins, relations, trail, retained intent `last_intent` :324) | Shipped |
| Ranked retrieval with inspectable why | `nav::query` (:543) + `query_with_relations` (:1336); `RetrievalScore.components` per-hit; `explain_hit` (:476) | Shipped |
| Magic textbox | `src/bin/hud.rs`: `parse_hud_input` (:444), `HudInput` (:304), prompt mode labels (:518); shell/open/edit/cd/global-search/search all resolve deterministically | Shipped |
| Object/action registry | `src/registry.rs`: `Registry::call(oid, verb, kwargs)` (:66), capabilities by object kind + verb, executors `fs.read`/`fs.write_draft`/`shell.execute`/`git status/diff` (:143), validators `v_exit_zero`/`v_path_exists` (:351), reliability `alpha/beta`, `seed` (:381). `src/capability.rs`: `Capability{object_kinds,verbs,tier,risk,cost,reliability,undo_link}` (:4), `TEN_VERBS` (:63), append-only `Observations` (:88) | **Built, unwired** |
| Action + belief check | `src/ported/act.rs`: `run()` records intent before outcome after (:431), `bearing_on_with` surfaces `Contradicted` claims (:313); measured limits in the module header | Shipped |
| Symbolic state/delta engine | `physis-core/src/transform.rs`: `WorldState` (:252), `Transform` (:294), `apply` (:416), `TraceStep` (:381), `ConstraintKind::check` (:228), `find_homomorphisms` (:148) | Shipped |
| Task model | `physis-core/src/process.rs`: `ProcessTask` (:42), `TaskState` (:53), `StateTransition` (:74), `ProcessCycle` (:203) | Shipped |
| Observation log | `physis-core/src/observe.rs`: `Observation` (:48), `append` (:106), `promote` (:657), `cited_by` (:702); watchers fs/git/shell/agent/`/proc`/browser in `src/ported/observe_adapters.rs` (:28–:352) | Shipped |
| Semantic deltas + transition log | `src/mvp/model.rs::SemanticDelta` (:189), `src/mvp/log.rs::Transition` (:30) with before/after state refs, validation, outcome, provenance, uncertainty | Shipped |
| Dependency propagation | `src/dependency/mod.rs`: `DependencyGraph`, `propagate_delta`, `PropagationTrace`, staleness classify/explain | Shipped |
| Verification | `src/ai/harness.rs::CoherenceHarness` (:161, `evaluate` :173), `src/grounding.rs::apply_gate` (:74), `src/machines/apodeixis.rs` (Lean proof obligations), workspace checks/fingerprints (`src/ported/system.rs:484`), stale-base seam (commit `eef298f`) | Shipped |
| Agent loop + tools + providers | `src/ai/agent.rs` (ReAct), `src/ai/tools.rs::ToolRegistry`, `src/ai/provider.rs::ProviderCascade`, `src/ai/session.rs`, `src/ai/memory.rs` | Shipped |
| OpenCode execution | `src/web/agent.rs`: SSE run, abort, sandboxed `opencode run` tool, `/agent` dashboard | Shipped |
| Delegation contract | `src/ported/system_delegation.rs`: `Mode`/`Kind`/`Cost` (:32), `Policy` (:53), `Request` (:83), `Worker` (:102), `Registry` (:118), `Plan` (:132) with correlation id, source fingerprint, invocation argv, `Reservation`, `TaskView`; CLI `system delegation list/show/plan/create/reserve` | D0/D1 shipped, execution not |
| Common ground (claims) | `src/ported/ground.rs`: `Claim`, `OpenDisagreement`, `Unscored`, `Ground::read/render` (:93, :122, :207) — no model, no summarising, disagreements stay open | Shipped |
| Declarative agent routing | `src/routes.rs`: `AgentRoute` (:21), `RouteTable` (:42), `for_cell` (:104); wired to `/api/v1/routes` (`src/web/mod.rs:3465`) | Shipped |
| Attention inputs | `src/salience.rs` (interpretant evidence, decay), `src/interest.rs` (`InterestProfile` :348, `update_profile` :364), `src/axes.rs` (`decayed_activation` :553, `AxisStore` :583), `src/quality.rs` (`cell_standing`), `nav` score components | Shipped |
| Life → goals hierarchy | `src/life_os/tracker.rs`: `LifeOSGoal` (:60), `Charge` (:24), `AlchemicalPhase` (:87), `TodayPlan` (:315), `DriftReport` (:452), sled persistence | Shipped |
| Notebook projection | `src/notebook.rs`: `WidgetDef` (:66), `Notebook` (:372), `validate` (:407); cells carry **references, never markup**; registry is code, notebook is data | Shipped |
| Plural meanings + overlays | `src/context.rs`: `Resolved` (:179), `Overlays::active_definitions` (:289), `meanings` (:323), `contested_names` (:350) | Shipped |
| Crystallization substrate | `src/discovery.rs` (corpus → draft ontology JSON), `src/decisions.rs` (:26 `fold_episodes`, :92 `mine_decisions`, :161 `PriorDecision`) + `src/sessions.rs` (Claude Code/Codex adapters), `src/adaptive_document/` (patch/proposal/undo), `src/planner.rs::simulate` (counterfactual, no mutation) | Shipped |
| Machine/device partials | `src/network.rs` (fs watch/scan), `src/monitor.rs` (sensors → primitives), `src/vfs/` (ontology symlink view), `src/moonraker.rs` (3D printer), `src/machines/` (20 reducers), `src/industrial.rs` (MQTT/Modbus/serial/REST adapters) | Shipped |
| MCP surface | `src/ported/system_mcp.rs` (15 workspace tools, no shell/launch exposed, opt-in audit rows), `src/mcp.rs` (Pro tool palette) | Shipped |
| JEV | `src/config.rs:468–502` (`jev_enabled/api_key/base_url/model/max_tokens/timeout`), `src/ai/provider.rs:574` cascade entry; live path **NOT MEASURED**, no local key | Config + cascade shipped |

### 2.1 Dead-but-built: wire it, do not rebuild

These modules are complete and tested but have **no callers outside
themselves**:

- `src/registry.rs` + `src/capability.rs` — the object/action system. No
  `Capability::new` call site exists.
- `src/ported/world.rs` — the log-order navigational substrate (`World` :41,
  `resolve` :246, `linked`/`lexical`/`blind` retrieval :282/:299/:310,
  `nearest_to_query` :329, `cmd_observe` :437). It still owns a `main`
  (:485) and is not referenced by the CLI or HUD.
- `src/ai/orchestration/agent_registry.rs` — 26-line `Agent{id,capabilities,
  current_coherence}` stub.

Reviving a module with no caller is a real risk: it can rot silently. Every
step below that wires one of these adds an end-to-end caller and an integration
test in the same change.

---

## 3. The gap

1. No `Agent` object, registry, lifecycle, permissions or routing ladder in the
   world model.
2. No agent surface in the TUI; no `@agent` grammar; no pipelines or delegation
   syntax.
3. No task graph as objects. `system_delegation::Plan` exists but is never
   executed and never rendered.
4. No conversation objects. Transcripts are ingested (`sessions.rs`) but not
   projected as nodes.
5. No fleet or host objects; no semantic remote execution.
6. No JEV structured-result contract (`JevResult`) and no accept-path that
   crystallizes it into relations, notes, decisions or ontology drafts.
7. Zoom stops at directory/file. No symbol level. No agent-at-scope annotation.
8. Intent is "last query" only. No editable
   `Intent{goal,constraints,priorities,subgoals}` object.
9. No single importance function shared by UI ranking and compute routing.
10. Similarity is adjacent pairs only. No "similar past changes / reusable
    repair trajectory".

---

## 4. Architecture (minimal)

Keep the existing layers. Add one module tree and wire the dead ones.

```text
physis-core            object/relation/ontology/delta/provenance/invariants (exists)
                       transform::WorldState + Transform + TraceStep  (the one executor)
                       process::ProcessTask + TaskState              (the one task type)
                       observe::Observation + watchers               (the one log)

physis-pro
  nav/                 objects, relations, trail, retrieval, semantic_parents  (exists)
  registry.rs          object/action registry: call(oid,verb,kwargs)          (wire)
  capability.rs        capability + verbs + reliability + observations        (wire)
  routes.rs            declarative cell → harness action table               (exists)
  ported/ground.rs     claims, disagreements, unscored predictions            (exists)
  ported/act.rs        run + belief check                                     (exists)
  ported/system*.rs    workspace service, MCP, delegation plan/reserve        (exists)

  agents/              NEW
    mod.rs             Agent as a world object (ObjectKind::Agent)
    spec.rs            AgentSpec, permissions, output schema
    registry.rs        roster (agents.json) + lifecycle state machine
    lifecycle.rs       spawn|call|pause|resume|inspect|pin|detach|kill|clone
                       |handoff|restrict|promote|archive
    adapter.rs         trait AgentAdapter { spec, probe, invoke }
    adapters/          jev | hermes | opencode | local | verifier
    routing.rs         deterministic-first ladder, explicit overrides
    tasks.rs           Task over ProcessTask; graph = fold over observations
    parse.rs           @agent grammar (pure, total)
    view.rs            pure render strings for the agent pane + task graph
    intent.rs          editable Intent object
    attention.rs       importance(x) — one function, two consumers
    similar.rs         deterministic-first similarity proposer
    crystallize.rs     grounded agent output → relations/notes/decisions/ontology
    fleet.rs           (later) hosts as objects, @agent@host routing
```

Rules:

- One executor: `physis-core::transform::apply`. One task type:
  `physis-core::process::ProcessTask`. One log: `physis-core::observe`.
- One context builder: the HUD's existing `context_text()` shape
  (INTENT / PRIMARY / DEPENDENCIES / TESTS / WHY). Agents receive that packet;
  no second context system.
- Agent definitions are data (`agents.json`, versioned JSON). Observed agent
  state is append-only observations. Never a second database.
- Permissions are deny-by-default and expressed over object kinds, verbs,
  scopes and hosts — not filesystem paths alone.
- A `reported` task can never become `accepted` without a verifier record.
- Every write is human-confirmed. No autonomous mutation. No auto-merge.

---

## 5. MVP loop and demo

The loop above is the product. The demo is the loop on a disposable worktree
containing one known, repairable defect plus one deliberately outdated document.

```text
> demo
Physis shows the project at high abstraction.

> (zoom) Physis → overlay → semantic navigator → module → function
> why is this here?
Provenance: requirement → decision → commit → file, each hop with evidence.

> @jev inspect this architecture
JEV appears inside the TUI over the selected graph scope.
JEV returns a structured JevResult; candidate nodes/relations appear in the graph,
not only as chat text.

> crystallize useful parts
Candidates are previewed; one is accepted. A relation and a note object are
written with provenance; the ontology draft is queued for review.

> @opencode implement selected repair
The task graph becomes visible: USER → JEV → OPENCODE → VERIFIER.
Affected files light up; OpenCode edits; Physis observes deltas.

> @verifier verify
Checks and invariants run; outcome recorded as an independent verification.
The architecture graph has changed.

> (disconnect the network)
JEV and Hermes show ○ unavailable with reasons. Crystallized knowledge remains.
Graph navigation, notebook, search, classification, similarity, verification,
history, actions and the local agent all continue.

> @hermes@omo inspect worker state
Remote placement is inspectable, never hidden.
```

A recording may demonstrate the agent path without credentials, but it must say
REPLAY and never count as live execution.

---

## 6. Delivery sequence

Estimates are planning estimates for one experienced developer with bounded
agent help, not commitments. `Phase 1` is the demo-critical slice: deliver it
first.

| Phase | Step | Estimate | Deliverable | Exit gate |
|---|---|---|---|---|
| 1 | **S0 — wire the action registry** | 2–3 d | Capabilities loaded from `config/actions.json`; objects registered from the nav index; `physis-pro action list/show/call`; HUD action line | Capability resolves by kind+verb; unknown verb is `unavailable`, never silent success; refusals recorded `ok=false`; reliability moves only through `record`; one end-to-end call in a test |
| 1 | **S1 — agent world model** | 2 d | `src/agents/{mod,spec,registry,lifecycle}.rs`; `ObjectKind::Agent/Task/Conversation/Claim/Intent/Host`; roster `agents.json` | Roster round-trip; illegal lifecycle transition rejected; permission defaults deny; old nav index fixture still loads; `ai/orchestration` stub deleted |
| 1 | **S2 — adapters** | 3–4 d | `AgentAdapter` trait + `jev`/`hermes`/`opencode`/`local`/`verifier`; `JevResult` schema | Probe never mutates; absent binary/key reports reason; `@jev` with no transport is `unavailable`; schema-invalid output reported, not guessed; each adapter has a fixture-process test |
| 1 | **S3 — routing ladder** | 1–2 d | `agents/routing.rs`; consumes `capability`, `routes::RouteTable`, delegation `Cost`/`Policy` | Table tests: offline roster chooses local or human only; cost ceiling respected; explicit override wins; no silent paid/remote substitution |
| 1 | **S4 — task graph** | 2 d | `agents/tasks.rs` over `ProcessTask`/`TaskState`; graph = fold over observations | Fold replay-stable; expire creates a child with parent + reason; `reported` never `accepted` without verifier record |
| 1 | **S5 — `@` grammar** | 1–2 d | `agents/parse.rs`; `HudInput::AgentCall`; interpretation shown before dispatch | Grammar table (valid, malformed, quoted scope, pipeline, `@all`, `@agent@host`); malformed is `Invalid(reason)`; existing command-box tests unchanged |
| 1 | **S6 — HUD agent surface** | 4–5 d | AGENTS strip, agent pane, task-graph overlay, conversation objects, `--snapshot` frames | Snapshot frames wide/narrow with text-only status; pane names its context; agent navigable as an object; `--snapshot` runs in CI |
| 2 | **S7 — editable intent** | 1–2 d | `IntentObject` as a Note-kind object; `life_os` goals above it | Edits provenance-stamped, visible in `why`/packet, survive restart; packets name the intent id |
| 2 | **S8 — attention + similarity** | 3 d | `agents/attention.rs` (seven terms, per-term readout), `agents/similar.rs` | Term readout matches its source store; offline returns deterministic legs only; attention ordering test; no similarity claim where only lexical matched |
| 2 | **S9 — crystallization accept-path** | 2–3 d | `agents/crystallize.rs`; grounding gate; preview → explicit accept | Ungrounded fields dropped and counted; reject changes nothing (byte-compare); accepted relation visible in `relations_of` and after restart |
| 3 | **S10 — zoom to symbol level** | 2–3 d | Zoom stack fleet→project→dir→file→passage→symbol; agent-at-scope | Zoom round-trip returns the same selection; symbol level degrades to passage, labelled |
| 3 | **S11 — offline contract** | 1–2 d | One truth table rendered from live probes | With JEV transport removed and embedder forced to random projection, the whole loop except agent reasoning works; every degraded surface labelled |
| 4 | **S12 — fleet + devices** | 3–5 d | `ObjectKind::Host`; `@agent@host`; printer as device object with typed actions | Host appears/disappears with reachability; remote failure explicit, never silent local substitution |

Total: roughly 30–40 developer-days. Phase 1 (S0–S6) is roughly 14–18 days and
is the first deliverable that feels like the new computer.

### 6.1 First task (start here)

S0, in one change:

1. Add `config/actions.json` and a loader that builds `Capability` values.
2. Register objects from `NavWorkspace` (`fs:*`) plus one host object and one
   device object placeholder.
3. Add `physis-pro action list|show|call` and a HUD action line that calls
   `Registry::call` and renders `CallResult` + the observation.
4. Add the integration test: a `fs.read` call through the HUD path returns the
   file and appends one observation with `ok=true`; an unknown verb returns
   `unavailable` and appends `ok=false`.

Do not start S1 until that test is green.

---

## 7. Do not build

- A second object/action registry, executor, `WorldState`, trace-step type,
  context builder, graph store or event bus.
- A new UI framework, a new TUI, or a parallel agent framework. The HUD is the
  surface; `physis-core::transform` and `physis-core::process` are the engine.
- An ACP adapter, auto-merge, autonomous writes, or a distributed database.
- Operator promotion outside the
  [crystallization plan](2026-09-18-operator-crystallization.md) and its E92
  gate. Crystallization here stops at grounded relations, notes, decisions and
  ontology drafts.
- A second agent registry. Delete `ai/orchestration/agent_registry.rs` in S1.

---

## 8. Test and evidence strategy

Gates (from `FLEET.md`; run on pansa):

- Fast iteration: `cargo check --all-targets` (~33 s).
- Done gate: `cargo clippy --all-targets -- -D warnings` + `cargo test`
  (~790 s). **Clippy is currently RED — 9 mechanical warnings. Fix it before
  S0; it is CI's own gate.**
- Focused tests per step before the full suite.

New tests, by step:

| Step | Tests |
|---|---|
| S0 | capability resolution, unknown verb refusal, validator refusal recording, reliability transition |
| S1 | roster round-trip, illegal lifecycle transition, deny-by-default permissions, old-index load |
| S2 | adapter probe/availability truthfulness, `JevResult` schema validation, fixture process per adapter |
| S3 | routing ladder table (offline, cost ceiling, override) |
| S4 | task-state legality, graph fold determinism, acceptance gate |
| S5 | `@` grammar table |
| S6 | snapshot frames wide/narrow, text-only status, key handling |
| S7 | intent edit provenance + restart |
| S8 | attention term readout, offline deterministic legs, similarity labelling |
| S9 | grounding gate, reject-changes-nothing, accept persists |
| S10 | zoom round-trip, symbol-level degradation |
| S11 | degraded-mode integration |
| S12 | host reachability, explicit remote failure |

Manual acceptance: the demo sequence in §5 on a disposable worktree, once per
phase, with the outcome recorded.

Dogfooding (per `AGENTS.md` and `docs/PHYSIS_DEV_LOOP.md`): start engineering
tasks with `physis_history`; use `physis_pack` and `physis_read` for evidence;
call `physis_remember` with a concise outcome and an explicit actor after checks.
Log Physis-assisted operations and their conventional fallbacks (tokens,
latency, hits/misses, fallbacks) so normal development produces benchmark data.

---

## 9. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Reviving dead code that then rots | Every wiring step adds a caller and an integration test in the same change (S0 is the pattern) |
| Two registries / two executors | Delete the `ai/orchestration` stub in S1; hard rule in §7 |
| `@jev` through OpenCode Zen is not the JEV service | Agent card shows transport (`zen:<model>` / `jev:<model>` / `unavailable`); no claim of JEV-the-service unless a JEV key is configured |
| Scope explosion | Phase 1 first; explicit do-not-build list; stop at green gates |
| `hud.rs` is 4119 lines with tests | All logic in `src/agents/`; HUD renders pure strings from `agents/view.rs`; add `--snapshot` |
| Autonomous mutation | Deny by default; capability `risk` gates write verbs; explicit confirmation; stale-base fingerprint refusal (`eef298f`); no auto-merge |
| Prompt injection via repo content | Content cannot widen scope; scope checked in the broker, never in a prompt (`system_delegation` discipline; `notebook.rs` inertness rule) |
| Parallel writers on one clone | One coordinator owns `agents.json` schema and `hud.rs`; re-check `git status` before destructive steps |
| `opencode run` startup too slow for interactive `@jev` | Measure in S2; keep a warm `opencode serve` and fall back to `run` if the numbers demand it |

---

## 10. Decisions locked

1. TUI target is `physis-hud`. Studio remains the demo product.
2. Agent adapters are CLI subprocesses first; MCP/ACP later if measured need.
3. JEV is reached through the OpenCode Zen free tier; a direct `jev_*` provider
   is used when a key exists. Transport is always shown.
4. Order: Phase 1 (S0–S6) first, then S7–S9, then S10–S11, then S12.
5. This document is the sequencing authority for the agent-native programme.

Defaults taken (change if wrong):

- S0 is its own step rather than folded into agent work; agent actions need the
  registry anyway.
- The Zen model id is pinned in config (`PHYSIS_JEV_ZEN_MODEL`, default from the
  live free-tier list) so the transport is visible and swappable without a code
  change.
- `ported/world.rs` is wired in S8 (its `trace`/`why` verbs feed attention and
  similarity); it needs the object layer from S1 first.

## 11. Open unknowns

- Whether `physis-hud` currently has a snapshot mode (Studio does; HUD appears
  not to). Confirm before S6.
- Exact Zen model ids available on the day (fleet notes name
  `oczen/laguna-s-2.1-free`; trust the live list at implementation).
- Whether `hermes` and `opencode` CLIs are on PATH on the dev box for adapter
  tests (fleet notes say yes via `.zshenv`; confirm in S2).
- Cost and latency of `opencode run` startup per call; measured in S2.

---

## 12. Documentation contract

| Question | Authority |
|---|---|
| What is Physis structurally? | `docs/WHAT_PHYSIS_IS.md` |
| What should this programme build next? | This plan |
| Market, packaging, settings/memory/primitives | `docs/plans/2026-09-18-agentic-workspace.md` |
| Learned-operator promotion and its E92 gate | `docs/plans/2026-09-18-operator-crystallization.md` |
| Delegation broker states and adapters | `docs/plans/2026-09-14-agent-delegation.md` |
| What actually runs here? | Current source plus a dated, reproducible verification record |
| What is licensed? | Package manifests and referenced licence texts |
