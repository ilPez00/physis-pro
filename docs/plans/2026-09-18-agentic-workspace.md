# Physis Workspace — agentic work with durable meaning

> Sequencing update, 2026-09-19: [Physis as the impossible computer — agent-native
> semantic world](2026-09-19-impossible-computer.md) is now the sequencing
> authority for the agent-native workspace. It supersedes this plan's sequencing
> and UI-expansion priority. This plan retains its market analysis, packaging
> experiments, Settings memory manager and primitives editor, and its P0–P5
> gates, which still apply where that programme touches the same surfaces.

Date: 2026-09-18. Status: proposed development programme; no new runtime
capabilities delivered by this document. Scope: `/home/gio/physis_pro`, branch
`agent/MS-005-q6`, inspected at `464ca7c` with existing uncommitted changes.

This was the **product sequencing authority** for the agentic workspace and
Studio demo from 2026-09-18 until the 2026-09-19 update above. It supersedes the
sequencing, UI expansion and Python-gateway priority in the same-day Studio and
operator-crystallization plans. Those files retain useful designs and historical
acceptance records. The industrial product remains supported; it is not the
first market for this programme.

Architecture authority remains [WHAT_PHYSIS_IS](../WHAT_PHYSIS_IS.md): an open
domain axis × fourteen modes, nested facets, and six operations over meaning.
The workspace is an application of those operations. It must not reduce the
engine to a context compressor, chat client or ledger.

## 1. Best possible outcome

The computer becomes a place where work has continuity. A person returns to an
intent, sees its evidence and unfinished consequences, chooses an agent, reviews
a bounded change, and later sees whether the change worked. Files, notes, runs,
decisions and source passages remain navigable together across sessions and
agent vendors. The user owns the files and can inspect or export the record.

“Neurosemantic” means learned representations help find relationships, while
explicit identities, provenance and observed outcomes anchor decisions. Similarity
is a navigational clue; it cannot certify truth or successful execution.

The long-term opportunity is a personal and team work environment above the
filesystem. First earn that position inside real repositories and their docs.
Keep ordinary editors, shells, Git and agents usable. An operating-system fork,
new editor engine, universal process supervisor and distributed graph database
are not prerequisites for this experience.

**North star:** independently verified work completed per human review hour,
with continuity across sessions. Track model cost and machine cost alongside
quality; cheaper incorrect work does not improve this metric.

## 2. Market position and route to value

Initial user: terminal-native engineers and small teams already using coding
agents, with recurring context reconstruction, repeated investigations and
difficult review. Initial buyer: the individual or engineering lead paying for
that lost time. First jobs: understand an unfamiliar change, resume an interrupted
task, review an agent patch, and recover why an earlier decision was made.

Agent interaction itself is competitive territory. Zed documents external agents
through ACP, with agents retaining their runtime and configuration; Warp offers
terminal and agent modes and an agent CLI. Sources checked 2026-09-18:
[Zed external agents](https://zed.dev/docs/ai/external-agents),
[Warp documentation](https://docs.warp.dev/),
[Warp agent CLI](https://www.warp.dev/agent-cli).
These establish overlapping surfaces, not a comprehensive competitor audit.

Our differentiation hypothesis: **a portable, evidence-bearing workspace that
survives changing agents and reconnects actions to outcomes**. Demonstrate this
advantage against the user's current editor + agent + shell, not only a naive
full-repository prompt. The TUI is a fast entry point; shared CLI/MCP contracts
make the value available without requiring a permanent UI switch.

| Stage | Offer hypothesis | Evidence required before expansion |
|---|---|---|
| Design partner | Guided local workspace on an actual repo; user brings their agent | Five target users; three repeat the full workflow without coaching and return within a week |
| Individual | Local workspace, source navigation, review, durable session handoff | Repeated use plus willingness to pay; test €15/€30 monthly offers, not a published price promise |
| Team | Shared reviewed decisions, handoffs, policies and reproducible run evidence | At least two teams request and trial shared work; test €30/€60 per seat monthly with support cost measured |
| Private deployment | Managed installation, approved providers and policy controls | Paid deployment demand and operational acceptance; quote from support burden |

Prices and conversion gates above are experiments, not valuation or market-size
claims. Keep inference BYO initially; avoid unbounded bundled token liability.
Do not change current licensing through this plan. Validate packaging against
the actual Core Apache-2.0 manifest and Pro EULA before any offer is published.

Prioritize repeated pain over total feature count. Interview while building;
record failed onboarding, missing evidence, return use, review minutes, verified
task completion and customer-requested next work. Stop a packaging experiment
that only earns praise for the graph and no recurring use.

### Where the monetary ceiling could rise most

These are strategic hypotheses, not measured market valuations. The TUI is the
entry point; the valuable asset could become the customer's durable record of
intent, execution, evidence and outcome, available across tools and agents.

| Possible role | Who pays and why | Evidence needed |
|---|---|---|
| Personal semantic workspace | Individual pays for faster resumption, navigation and review | Repeated solo use and measured time saved |
| Team memory for ongoing work | Engineering lead pays to reduce repeated investigation and brittle handoffs | A second person/agent completes work using the record; fewer repeated mistakes |
| Control layer for agent execution | Platform lead pays to govern changes and obtain verifiable outcomes across agents | Independent worker integrations, enforceable scope, recovery and independent checks |
| Change-impact and evidence system | Teams pay to identify consequences across code, documents and decisions | Useful impact candidates with measured misses/false alarms and source provenance |
| Learned workflow library | Operations owner pays for recurring tasks completed with less supervision | Learned operators beat maintained handwritten recipes on total cost and quality |
| Operational model of a business | Domain owner pays to connect decisions with observed business outcomes | One validated domain workflow, explicit limits, human review and reliable integrations |

Highest near-term expansion potential: combine team memory and execution control
around one recurring engineering workflow. This shifts the purchasing reason
from a nicer terminal to reliable ongoing work. It requires adoption and proof;
calling it an operating system does not itself create value.

The strongest defensibility hypothesis is a growing, customer-owned history of
which actions worked under which conditions, plus integrations and trusted review
practice. Portability should be a feature: earn retention through usefulness,
not inaccessible records. A graph, embedding model or collection of prompts is
insufficient by itself. Do not promise a network effect until cross-user benefit
has actually been demonstrated without violating data boundaries.

Expansion trigger: two teams use Physis for live repeated work, prefer its
handoff/review record to their previous workflow, and will fund broader coverage.
Then prioritize shared identity, policy, multi-user concurrency and durable
execution over more personal UI surfaces. Do not build enterprise administration
in anticipation of that evidence. Long-term, this can become a semantic operating
environment spanning applications; the first proof is a small task that survives
agent replacement, interruption, review and restart with its meaning intact.

## 3. Source-backed starting position

This table records code inspection, not a fresh build or performance certificate.
Older benchmark and test results retain their dates and limitations.

P0 cost/quality baseline, frozen 2026-09-18 on pansa (debug binary
`physis-pro 0.1.21`, dev licence + `PHYSIS_ALLOW_DEV_LICENCE=1`, local
bge-base embedder, no provider keys — extractive/degraded paths only):

| Measure | Result | Conditions |
|---|---|---|
| `pack` fixture copy, budget 1200 | 174/1200 tokens, 4 chunks over 4 files, 0 dropped; top score 0.836 (`src/context.rs`) | 3.4 s wall (18.1 s user — cold ONNX in-process) |
| `notebook` fixture, "what is the retrieval budget" | EXTRACTIVE answer, correct (500, `[3]` cites `retrieval_budget`), 4 docs, 174 context tokens, 193 ms retrieval, backend bge-base | 0.77 s wall; DEGRADED banner (no oracle/provider keys) as designed |
| `physis-hud --prescan-only` fixture copy | 8 objects, semantic true (bge-base), +8 embeddings in ~2.9 s, workspace 174 tok | cold index; later starts incremental |
| `system find/pack` on this checkout | find ranks plan + history records; pack 1092/1200 tokens | 5034 docs indexed, 8.4 MB content bytes; `Cargo.toml.bak` (tracked, stale) excluded from evidence |
| Licence gate | dev-build licence minted with all-zeros-seed key (`PX-P0BASELINE-20260918`); gated commands run only with `PHYSIS_ALLOW_DEV_LICENCE=1` | prior `.physis/license.key` was signed by `~/.physis-vendor` key ≠ dev fallback → BadSignature; replaced |

| Area | Current evidence | Gap to target |
|---|---|---|
| Workspace service | `src/ported/system*.rs`; launcher selects Pro; CLI history/read work locally; MCP stdio verified 2026-09-18 (obs:35) | Connected MCP configs pointed at `/home/gio/dev/physis-pro` until this P0 slice — fixed to `/home/gio/physis_pro`; `.mcp.json` now `system serve`; reconnect clients to take effect |
| Navigation | `nav::workspace::NavWorkspace` lib + `bin/hud.rs`; index, relations, trail, lazy embedder (fixture prescan: 8 objects, bge-base, rc=0) | **No `nav` subcommand in the 2026-09-18 binary** — `nav …` CLI lines are the P2 target surface, see `docs/SEMANTIC_NAVIGATION.md`; Studio GRAPH uses separate `FileThemeGraph`; no single end-to-end Studio navigation/action loop |
| Studio | `bin/studio.rs`: STATUS, GRAPH, NOTEBOOK, DIFF, MACHINES, CONFIG; `--snapshot` renders real frames | NOTEBOOK calls `ported::notebook::answer()` on key path (studio.rs:441) with `MAX_NOTEBOOK_DOCS=200` pre-selection (studio.rs:81,402); DIFF is review-only — `diff::{diff_lines,diff_words}` hunks with no `edit_node`/quality write path |
| Context and answers | `pack.rs::pack/pack_cached`, `rag.rs::retrieve/retrieve_from_texts`, `ported/notebook.rs::answer` | A source list is not proof each generated claim is supported; need coverage and citation validation |
| Action and observation | `mvp/` loop, `ported/act.rs::run/bearing_on`, `sessions.rs`, workspace observations | Confirm boundaries and recovery before connecting a worker to Studio; reservation is not execution |
| Review | `diff.rs` hunks (Studio, review-only) vs `core.rs::edit_node/delete_node` (exchange-parts, tested) vs `adaptive_document/` revisions | Raw-file patch, document revision and coherence edit have different ownership/undo semantics |
| Settings | `config.rs::PhysisProConfig`, `ported/config_run.rs::RunConfig`, Studio's five knobs (studio.rs:200) | No verified unified effective configuration; `NavWorkspace::embedder()` constructs `PhysisProConfig::default()` (workspace.rs:73) instead of reading effective settings |
| Build weight | Cargo defaults = cli/voice/web/tui/sql/postgres/email/embed-onnx; `physis-core` dep explicitly enables `embed-onnx` (Cargo.toml:57) | `--no-default-features` alone does not prove a lightweight dependency closure — default ONNX pulls `ort+ndarray+tokenizers` unconditionally |
| Research | transform/propose/discovery primitives; crystallization plan; `pyo3` optional behind `instructor` feature (Cargo.toml:102,155, off by default) | Operator learning, promotion and composition gateway are not demonstrated end-to-end |

Known runtime issues outside this programme: shift demo overwrites report files
inside a multi-CSV loop; shift import can report accepted rows despite failed
persistence. Preserve these as explicit backlog defects; do not sell affected
flows as verified while focusing the workspace work.

## 4. The demo: six minutes that feel like a complete product

Use a small real repository plus notes and a deliberately outdated document.
Create a disposable Git worktree containing a known, repairable bug. Keep the
original checkout untouched. Fixture has exact expected change and a check that
fails before the repair and passes afterwards. Labels identify demo data.

| Time | User action | Visible proof and delight |
|---|---|---|
| 0:00–0:40 | Open workspace, resume “make this setting actually work” | Instant usable shell; retained intent, pending work and last outcome; clear indexing progress |
| 0:40–1:30 | Search by meaning, zoom through local graph, open why/source | Same object selected across map, source and history; user/deterministic/inferred edges distinguishable without colour |
| 1:30–2:10 | Ask what needs changing | Bounded evidence packet with exact passages, excluded coverage, real backend and expandable reasoning for selection |
| 2:10–3:20 | Hand task to one installed agent | Actual worker identity, allowed scope, budget and cancellation; streamed progress; bounded staged patch |
| 3:20–4:20 | Review hunks, inspect affected callers, run check, accept | Baseline failure → candidate success; revision checked before apply; rejected hunk changes nothing |
| 4:20–5:20 | Observe consequences | Changed files and affected documents highlighted; outdated assertion becomes a review candidate with source evidence |
| 5:20–6:00 | Close and reopen; hand off to another session | Intent, accepted change, evidence and remaining work survive; next agent receives a compact handoff |

The strongest moment: the map changes because real work changed, and the next
session remembers why. A large animated graph is optional; causal continuity is
the spectacle. A second supported agent can inspect the same evidence in the
extended demo without needing a second simultaneous writer.

### Demo completeness contract

- Ship one entry point and one coherent shell. Workspace/map, source/notebook,
  runs/review and history are connected views with shared selection. Settings is
  a drawer; machine examples are optional discovery material. Final labels may
  reuse existing tabs; do not spend a phase merely renaming them.
- Each primary action works: open, find, inspect, ask, hand off, cancel, review,
  validate, apply, undo where safe, resume. No decorative buttons or fake jobs.
- Command palette, visible key help, back trail, source jump, useful empty/error
  states, terminal resize, text edge legends and reduced-motion option.
- Offline navigation/history/review remain useful without a model or server.
  Extractive fallback is labelled. A recording can demonstrate an agent run
  without credentials, but must say REPLAY and never count as live execution.
- One supported live worker is mandatory for agent-demo acceptance. A second
  is required for claims of demonstrated cross-agent continuity.
- Real provider usage if available; estimated tokens labelled as estimates.
  Never display whole-corpus reduction as measured money saved.
- Seed, snapshot and scripted key sequence exercise production paths. No demo
  branch with different retrieval, persistence or acceptance semantics.
- Almost complete means the entire bounded journey passes. It does not mean
  every research feature or enterprise surface is present.

## 5. Reuse architecture and ownership

### Settings as a workspace control room — required demo scope

Settings contains **Runtime · Memory · Primitives · Appearance**. Opens as a
drawer, expands to a full workspace for inspection, and returns to the previous
selection without losing task state. Memory manager and primitives editor are
required planned capabilities, not optional polish or currently shipped features.

**Memory manager.** One searchable projection over existing stores; no new
parallel memory database. Group records by purpose: working intent/session,
durable user notes/decisions, agent observations/outcomes, and derived indexes/
embedding caches. Show actual workspace/store, record counts, measured bytes,
source, actor, timestamp, revision, retention and retrieval eligibility. Never
equate disk size or similarity with usefulness or truth.

- Browse, filter, inspect original evidence, pin, annotate, export, and exclude
  an item from future retrieval. Show why an item entered a context packet.
- Corrections create linked revisions; contradictory accounts remain inspectable.
  A summary points to its sources and never replaces the original silently.
- Preview retention/cleanup by category and affected dependencies. Derived-cache
  eviction is distinct from forgetting user knowledge. Forget first excludes
  retrieval; explicit permanent purge identifies the exact scope, warns about
  irreversibility and explains export/backup limits. Preserve audit structure
  with tombstones without retaining purged sensitive content. Do not promise
  removal from external providers or unmanaged backups.
- Pause collection per source, show excluded paths, and reveal memory actually
  shared with each worker. Avoid collecting secrets by default; export previews
  show scope and redaction. Workspace isolation applies to search and mutations.
- Budget memory and disk separately; display measured usage versus configured
  cap. Prune derived caches first. Large collections use paginated/virtualized
  lists and cancellable background queries; opening Settings loads no model.

**Primitives editor.** A typed inspector/editor for the building blocks available
to this workspace: ontology entries/facets, supported relation definitions,
declared actions and their parameter/policy metadata, and saved deterministic
recipes. List only primitives actually registered in the active build. Distinguish
compiled implementation, editable metadata, workspace override and experimental
composition. Editing metadata cannot create a missing executable capability.

- Search by name, role or domain; inspect input/output types, implementation
  provenance, consumers, permissions, examples and active version.
- Edit supported data-backed fields through typed forms and a validated raw-data
  view. Built-in code and unsupported schemas stay read-only with a reason.
  No arbitrary Rust/Python/shell execution from a settings field.
- Draft → validate → preview impact/diff → explicitly apply → inspect outcome.
  Validate references, cycles where prohibited, argument types and policy bounds.
  Check the base version before applying; save atomically and retain a reversible
  prior version. Active runs retain their starting version; updates affect new runs.
- Dry-run only where the operation has a genuinely side-effect-free preview.
  Otherwise offer static validation and a clearly scoped fixture/worktree trial.
  “Validated schema” never means “proven correct”; replay evidence stays separate.
- Preview changed consumers, cache invalidations and proposed reclassification.
  Ontology edits do not silently rewrite past assertions. New permissions require
  explicit authorization; workspace overrides cannot widen the enclosing policy.
- Ship one real editable data-backed primitive and one reusable deterministic
  recipe in the demo. Learned operator authoring remains gated by §10; the editor
  works before a crystallization registry exists.

### Art direction: calm, precise, visibly alive

Deep charcoal base, warm readable text, restrained cyan for focus, amber for
unresolved evidence, muted green for verified outcomes. Meaning always has a
text/glyph equivalent; support light, high-contrast and monochrome terminals.
Use generous spacing, aligned columns, consistent borders and one dominant
selection. Avoid gradients, blinking dashboards and decorative metric overload.

```text
 Workspace / setting repair                          LOCAL · 2 pending
 ┌ Settings ──────────┬ Memory / Decisions ───────────┬ Evidence ───────┐
 │ Runtime            │ / search                      │ source: …      │
 │ Memory        ◀    │ pinned  timeout rationale     │ actor / date   │
 │ Primitives         │ stale   old API assumption    │ revision / why │
 │ Appearance         │ kept    successful repair     │ related runs   │
 └────────────────────┴───────────────────────────────┴────────────────┘
 Enter inspect   / filter   Esc return           actions shown by state
```

Primitives uses the same master/detail grammar: catalogue → typed editor →
impact/evidence preview. Narrow terminals collapse to one pane with retained
back trail. Show loading, empty, error, stale, dirty and saved states deliberately.
Animation only explains a transition; reduced motion disables it. Keyboard focus
is obvious; typing never triggers global destructive shortcuts. Snapshot checks
cover wide/narrow layouts and text-only status; interactive checks verify actions.

**Demo addition:** within the six-minute journey, inspect and exclude one stale
memory, then edit a recipe parameter, preview its bounded effect and save a new
version. Reopen to show both persisted. Use an optional two-minute guided extension
for retention/export and primitive rollback; do not race through core proof.

**Acceptance:** memory exclusion changes the next retrieval and restores safely;
cross-workspace records never leak; export round-trips supported record types;
cleanup preserves pinned/user evidence unless explicitly selected; invalid or
stale primitive edits leave active state unchanged; a valid edit affects the next
run only; rollback restores prior behavior. All controls show actual effects.

```text
CLI / MCP / Studio
       │ shared workspace identity, selection and effective settings
       ├─ NavWorkspace → objects, source locations, relations, trail
       ├─ Workspace service → history, packs, task records, bounded runs
       ├─ notebook / retrieval → evidence packet + optional synthesis
       └─ existing action/review paths → staged change → checks → observation
                                      │
                     source changes → incremental invalidation → updated views
```

Extend these existing boundaries only where the demo needs coordination. Do not
introduce a parallel all-purpose Workspace trait, second executor or new graph
store. FileThemeGraph remains a thematic projection; it must not become another
authoritative file identity layer. Source spans resolve against a content hash;
historical evidence remains historical if its source changes.

Keep user relations, deterministic facts and inferred proximity separate. Store
explicit outcomes separately from similarity. Pass workspace/root/revision into
operations; never rely on whichever directory an agent happened to launch from.
One intent can link source evidence, run IDs, review and observed results without
copying every store into one database. Reuse IDs where semantics match; add only
the missing links, with schema versioning and old-state loading checks.

For agents, use the installed CLI plus existing MCP first. Study ACP for a later
adapter if it removes real per-agent code; the demo does not depend on it. Native
agent authentication/configuration remains native. Ingest only explicit session
events; never claim a shell exit code proves task success.

Action boundary: inspect and plan freely within the selected root; show mutation
scope before dispatch. Stage writes in a worktree, preserve user's dirty changes,
verify base hashes before applying, and refuse stale/conflicting patches. Record
the actual command, return code and independent task check. A failed check or
cancelled run cannot become accepted success. Review acceptance is not permission
to deploy or publish. Tool/document content cannot enlarge granted authority.

## 6. Configurable and light execution

### One effective configuration, progressive disclosure

Reuse the existing serde configuration and loaders. Proposed precedence:
built-in defaults < named profile < user config < explicitly trusted workspace
config < environment < CLI/session overrides. Show each effective value and its
origin, validate bounds and unknown fields, redact secrets, and distinguish
hot-reloadable settings from settings requiring restart/reindex. Workspace
configuration cannot silently enable network, new commands or broader roots.

Begin with three presets and an Advanced view. These are proposed profiles,
not currently supported flags:

| Setting | Lite | Local semantic | Connected |
|---|---|---|---|
| Retrieval | lexical, no model | lazy local embedder + lexical | same local retrieval |
| Generation | off | explicit local provider | explicit selected external/local provider |
| External model access | off | off; approved loopback only | opt-in endpoint allowlist |
| Indexing | on demand | bounded background incremental | bounded background incremental |
| Worker concurrency | 1 | 1 | 1 default, configurable bounded pool |
| Idle work | none | pause after queue drains | pause after queue drains |

Expose knobs with an actual tradeoff: root/excludes, symlink policy, content-size
limits, context/output budget, queue depth, timeout, concurrency, cache disk cap,
model/backend, network policy, ONNX thread count, debounce and watch/poll interval.
Expose model dimension as validated model metadata, not a casual slider that
silently makes existing embeddings incompatible. Never reduce security or
correctness because Lite is selected.

### JEV reasoning provider (added 2026-09-18)

JEV is one more OpenAI-compatible entry in the existing `ProviderCascade`
(`src/ai/provider.rs`), not a parallel generation path. It earns its place the
same way every provider does: configured, listed, tried in order, parked on
fault, never silent when absent.

- **Settings surface.** `AgentRuntimeConfig` gains `jev_api_key`
  (secret, redacted in every display), `jev_base_url` (default JEV endpoint),
  `jev_model` (default `r2-reasoning` — exact id pinned at implementation),
  plus `jev_enabled` (default false) and `jev_max_tokens`/`jev_timeout_secs`
  with the same bounds discipline as the other providers. All five flow
  through `load_effective` (builtins < file < env: `JEV_API_KEY`,
  `JEV_BASE_URL`, `JEV_MODEL`), appear in `physis-pro config` + `doctor`
  with origin, and ship in `/api/v1/settings` as presence (has-key) +
  model + origin — never the key value. Unknown-field and bounds rejection
  apply unchanged.
- **Cascade wiring.** `ProviderCascade::from_env` adds the JEV entry when
  enabled with a non-empty key, priority above the keyless proxies and below
  explicit keyed providers. Supports tools false unless verified live;
  vision false. `providers_list` shows it like any other entry. A retired
  model id or rejected key parks it via the existing `is_config_fault`
  codes — no JEV-specific fault path.
- **UI.** `/settings` providers card counts it; configuration section shows
  JEV model + origin + has-key state. No key material in HTML, logs, or
  transition records.
- **Acceptance.** Key absent/disabled → cascade unchanged, `/ask` degrades
  with note as before. Key present → `providers_list` contains `jev`,
  a live `/ask` reports `provider: jev`, a bad key parks it (15-min
  cooldown, recovers without restart). `cargo test` provider unit tests
  cover config parse, redaction, and park-on-fault; no live-key test in CI.

Status 2026-09-18 (branch `agent/MS-005-q6`): settings surface, env layering
(`JEV_ENABLED/API_KEY/BASE_URL/MODEL/MAX_TOKENS/TIMEOUT_SECS`), redaction in
`config`/`doctor`/`/api/v1/settings`, `/settings` state row, and cascade
inclusion are implemented and unit-tested
(`config::tests::effective_config_jev_layers_and_bounds`,
`ai::provider::tests::jev_cascade_follows_enable_and_key`, settings HTTP test).
Live `/ask` against a real JEV endpoint and park-on-bad-key are **NOT
MEASURED** — no live key exists in this environment. The exact model id
(`r2-reasoning`) and base URL (`https://api.jev.ai/v1`) are defaults to be
pinned against the real service before release.

Missing model, unavailable feature and empty corpus have distinct messages.
Lowering a context budget reports omitted coverage; it never silently drops
required evidence. Switching providers does not invalidate unrelated index data.
Changing embedding identity/version/tokenizer/pooling/dimension invalidates the
matching embedding cache. A restart reuses compatible content-hash results.

### Make unused capability cost little

1. Render before model initialization; move scan/embed/generation off input path
   with bounded queues, cancellation and latest-query-wins updates.
2. Retrieve candidate passages before synthesis; avoid loading the first arbitrary
   200 documents. Report indexed/excluded/unreadable counts and stale results.
3. Incrementally update changed content; debounce bursts; reuse existing embed
   cache; prevent oversubscription between workers and model threads.
4. Bound cache/log growth, file descriptors and queued work. Prune only derived
   caches automatically; user notes, decisions and evidence follow explicit
   retention/export policy. Measure embedding memory separately from TUI memory.
5. Ship tested lean and semantic bundles. Inspect transitive Cargo features:
   currently Core's unconditional `embed-onnx` selection defeats a naive lean
   build. Make optional dependencies truly optional without breaking existing
   defaults first; default changes require a migration note.
6. Keep Python/Instructor, OCR, SQL/Postgres, voice, server and experimental repair
   pipeline out of the lean bundle unless its workflow needs them. Spreadsheet
   extraction should not require Tesseract merely because features are coupled.
7. No daemon/server required for one local user. Introduce shared model hosting
   only if measurements show duplicate resident models dominate cost.

### Performance gates (targets, NOT MEASURED)

Pin a representative 10k-file mixed repo and a lower-powered target machine;
record CPU, RAM, disk, build, features, model and cold/warm state. Run at least
20 repetitions for latency distributions; report indexing separately.

| Measure | Initial target |
|---|---|
| First useful frame, persisted workspace | ≤500 ms p95 |
| Keyboard response during indexing/generation | ≤50 ms p95 |
| Warm lexical search | ≤200 ms p95 |
| Warm local semantic retrieval | ≤1 s p95, excluding answer generation |
| Visible cancellation acknowledgement | ≤250 ms; actual worker termination measured separately |
| Lite steady-state RSS on fixture | ≤150 MiB; no loaded model |
| Lite idle CPU after queues drain | <1% of one logical core over 60 seconds |
| Source/save observation | visible stale marker immediately; reindex latency measured by changed bytes |

Binary size, clean/incremental build time, cold embedding throughput, cache growth
and local semantic RSS get measured baselines before numeric promises. Tune from
profiles and dependency evidence; do not chase a small binary by removing checks.

## 7. Development and code management

- Develop/build/test on pansa through Randomesh. One coordinator owns this plan
  and integration; independent workers receive bounded tasks and acceptance
  evidence. Reservations do not launch workers. Avoid multiple Cargo jobs on
  the same target directory and overlapping writers on Studio/config files.
- First record branch, HEAD, dirty paths, Core revision, binary and MCP root.
  The other `/home/gio/dev/physis-pro` checkout has different changes; do not
  merge, copy or discard them implicitly. Use explicit worktrees for new work.
- Map existing ownership before moving modules. Core primitives, Pro workspace
  services and UI rendering have different responsibilities. Extract only code
  needed by a second current caller; no broad crate split ahead of the demo.
- Keep primary paths library-driven, rendering free of blocking I/O, and event
  handlers small. Reuse the installed dependencies; no new plugin framework.
- Test the narrow change, then its workflow: configuration precedence, stale
  patch refusal, cancellation, persistence/restart, root isolation, real source
  jumps and no-provider fallback. Keep snapshot tests for layout, not evidence
  that an action worked.
- CI: quick formatting and affected tests per patch; feature-matrix smoke for
  lean/semantic/connected changes; full appropriate suite before release.
  Reuse existing Cargo cache, lockfile and test profile; profile before adding
  distributed builds or more CI infrastructure.
- Each patch includes relevant doc delta and a concise handoff: objective,
  changed paths, exact checks, result, limitations and next dependency. Record
  Physis lookups and conventional fallbacks in the same workspace store.
  Shipped 2026-09-18 (P1 slice): `physis system runs|usage` plus three read-only
  MCP rollups (`physis_list`/`physis_runs`/`physis_usage`) and the opt-in
  `PHYSIS_MCP_AUDIT=1` call trail, so this instruction is measurable rather
  than aspirational — a lookup leaves either a note or an audit row.
- Never use a new abstraction to hide an existing duplicate. Retire legacy UI
  code only after the replacement reaches parity; preserve command aliases
  during transition. Keep shipped experiments outside default hot paths.

## 8. Delivery sequence and acceptance

Estimates: 6–8 weeks for one experienced developer with bounded agent help;
planning estimates, not a commitment. Inspect and baseline first. Parallelism
can help independent evidence/docs work; it cannot remove integration dependencies.

| Phase | Estimate | Deliverable | Exit gate |
|---|---|---|---|
| P0 — authoritative baseline | 2–3 days | Checkout/MCP/build identity; corrected entry docs; frozen demo fixture; baseline cost and quality | Same known file/hash through CLI/MCP; root/store diagnosed; critical-path capabilities classified as implemented, partial or planned |
| P1 — light usable shell | 4–5 days | Effective settings; Runtime/Memory/Primitives/Appearance shell; lean build; cancellable background work | Responsive input under slow backend; invalid settings rejected; Settings loads no model; feature closure inspected |
| P2 — connected workspace | 5–7 days | World/source/notebook share focus, source identity, trail and evidence; persistent intent | User finds unfamiliar evidence, follows source and resumes after restart without guided commands |
| P3 — real agent change | 6–8 days | One worker through existing boundaries; staged diff, independent checks, stale-base handling and cancellation | Seeded bug fails before/passes after; failure never auto-accepts; dirty source preserved; tested apply and safe undo |
| P4 — consequences and continuity | 7–10 days | Incremental change view, memory manager, typed primitives editor, durable handoff; second-agent read integration | Memory and primitive acceptance above passes; consequences persist; second agent reads exact evidence |
| P5 — complete demo and release | 4–5 days | Reproducible six-minute journey, install/update/rollback, concise guide, failure-mode rehearsal | Runs on pansa and target client; snapshot and interactive smoke; no dead primary controls; offline and disconnected paths usable |
| P6 — design-partner iteration | 1–2 weeks, overlaps P2 onward | Real-user workflows and paid-offer experiments | Repeat-use/customer gates in §2; retained benefit on independent tasks, not just fixture |

Critical path: P0 → P1 → P2 → P3 → P4 → P5. User interviews and fixture review
run in parallel. At every gate, fix the blocker before expanding tabs.

Memory/primitives scope adds roughly 3–4 days to the previous estimate; rebaseline
to 7–9 weeks after P0, subject to existing store mutation/export support. The
required demo editor is intentionally bounded to supported data-backed types.

If estimates slip, cut simultaneous multi-agent writes, generic OPS, cloud sync,
voice and additional visualizations first. Preserve resume, source evidence,
real agent execution, review, memory manager, primitives editor, settings,
cancellation and restart recovery.

## 9. Evidence and release scorecard

Freeze representative tasks before tuning: unfamiliar-repo lookup, interrupted
task resume, bug repair, source-backed documentation update and agent handoff.
Include misses and unanswerable questions. Compare user's normal agent/editor
workflow, Physis lexical, and Physis semantic; keep task/model/budget equal where
the comparison requires it. Randomize task order or use matched tasks to reduce
learning effects. Count indexing/setup and human correction time.

Report completed independent checks / attempted tasks, source-hit rate,
unsupported-claim rate, stale-context errors, review minutes, total latency,
measured provider usage, estimated local tokens, CPU/RAM and failure recovery.
Separate fixture results, real-user results, replay and live execution. Store
commands, revisions, corpus identity and artifacts so another session can rerun.

Release gate: all demo acceptance checks pass, baseline regressions explained,
no unresolved data-loss/root-isolation defect, installation and rollback verified,
known limitations visible. A green screenshot or exit 0 alone is insufficient.

## 10. Crystallization and the larger computer

Keep the ambition, earn each layer:

1. **Workspace memory:** observed tasks and explicit outcomes are reusable now.
2. **Suggested recipes:** surface repeated successful sequences for human review.
3. **Earned operators:** compare ordinary agent, handwritten deterministic recipe
   and learned operator on held-out varied tasks. Include discovery and upkeep
   cost, false reuse, failure rate and actual reuse frequency.
4. **Adaptive work environment:** promote only operators that beat the simpler
   alternative; version, expand, replay and demote them. No model confidence
   shortcut. Reuse transform/execution primitives if their semantics fit.
5. **Personal/team computer layer:** federate approved workspaces and add documents,
   tools and sensory inputs incrementally after identity, permission and retention
   boundaries survive real use. Team sync is a new product gate, not file-copy glue.

Timebox the initial operator experiment to 2–3 days after P4, using real run
traces. If a handwritten recipe wins, ship it. Python/Instructor remains optional
and dormant until a concrete proposal-adapter requirement justifies maintenance.
Per-cell structural tables and certification research do not block the demo.

## 11. Documentation contract and drift closure

| Question | Authority |
|---|---|
| What is Physis structurally? | `docs/WHAT_PHYSIS_IS.md` |
| What should this programme build next? | This plan |
| What actually runs here? | Current source + dated, reproducible verification record |
| How do I build/connect/use it? | `docs/SYSTEM_INTERFACE.md`, `docs/SYSTEM_DELEGATION.md`, `docs/SEMANTIC_NAVIGATION.md` |
| What did an older experiment/plan establish? | Its dated report; retain corpus, revision and limitations |
| What is licensed? | Package manifests and referenced licence texts; README describes them |

This planning change corrects front-door documentation for service ownership,
build commands, licensing metadata, video implementation status, harness count
and speculative priority; annotates older Studio/World plans; and links current
direction from README/AGENTS. It does not certify every historical document.

P0 establishes a small critical-path capability table with producer, consumer,
feature gate, command and dated evidence. Generate inventories/defaults from
existing metadata only when that removes duplicate ownership. CI checks local
links and documented executable names/flags for supported builds. Semantic doc
drift is a review candidate with changed source evidence, never an automatic
rewrite of history. Mark historical docs at entry points; preserve their results.

Observed dogfood on this planning task: local history returned no matching
workspace-roadmap record; local pack used 1193/1200 heuristic tokens and included
a stale `Cargo.toml.bak`; exact local `system read` resolved Studio's blocking ask
path. Connected MCP targets the other checkout. Conventional `rg`/`sed` supplied
the focused evidence. No matched token-saving or latency claim is made. Exclude
backups and identify workspace provenance as P0 retrieval requirements.

## 12. P3 gate (next, not yet started)

Staged-diff handoff over reused primitives: `System::snapshot`
(git HEAD + porcelain + binary diff `HEAD`) supplies the stale-base check in
`ported/system_delegation.rs`; `diff::diff_lines` supplies the staged preview.
A seam records the source fingerprint at review start and refuses to apply
when files changed while the diff sat pending; the seeded-repo check fails
before the fix and passes after. Worker dispatch remains an operator command
(never auto-run). Out of scope here: a second simultaneous writer.
