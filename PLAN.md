# Physis Pro Evolution & Monetization Strategy

## Goal
To evolve Physis Pro from an internal intelligence engine into a commercially viable, industry-standard "Industrial Backoffice" platform, and to define a clear path to monetization.

## Implementation Plan

### Phase 1: Technical Perfection (The "Intelligence" Upgrade)

#### 1A — Scanner & Ingestion Pipeline
- [x] **Scanner overhaul**: `WalkDir` directory traversal + `rayon::par_iter()` parallel file processing + streaming SHA256 (64KB buffer).
- [x] **Pipeline dedup**: Extracted 160-line duplicate Ingest/RebuildGraph code into `run_ingest_pipeline()`.
- [x] **Regex caching**: 4 `once_cell::sync::Lazy` statics replacing repeated `Regex::new()` calls.
- [ ] **Disk indexer**: `physis-pro index <mount-point> [--output F]` — generate a structured file catalog (tree depth, sizes, dates, type breakdown, largest files, total/human-readable). Outputs Markdown, JSON, or plain-text. Reuses scanner's `WalkDir` + `rayon` infra but skips hash/embedding (fast path). `--watch` mode: track changes across runs via a persisted manifest. Covers all mounted volumes in one invocation with `physis-pro index --all`.

#### 1B — Error Handling & Code Quality
- [x] **Critical unwrap elimination**: `file_name().unwrap()` → `unwrap_or_default()` in `vfs/orchestrator.rs`.
- [x] **CI/CD pipeline**: Rust CI with `cargo check` (3 feature sets), `clippy` (deny warnings), `cargo test`.

#### 1C — Feature Completeness
- [x] **Inventory command**: Real sled DB access showing item count + first 20 key/value pairs.
- [x] **Process command**: TCP connectivity checks behind `#[cfg(feature = "industrial")]`.
- [ ] **Real-time Industrial Adapters**: Move from TCP pings to full MQTT/Modbus subscribe/publish loops feeding `HolonicGraph`.

#### 1D — Embeddings & Ontology
- [x] **Upgrade Embeddings**: BGE-base-en-v1.5 (768-d) ships as the production embedder, self-test-gated with MiniLM → RandomProjection fallback (see PREMORTEM P0/P4). Accuracy 0.974 domain / 0.923 cell on the curated set.
- [x] **Automated Ontology Discovery**: `discover` mode (`src/discovery.rs`) embeds a corpus, flags texts the grid covers poorly (best-entry cosine below an operator-tunable threshold), clusters the gaps, and emits a ready-to-edit ontology JSON draft. Surfaces the best-entry cosine distribution so the threshold is calibrated from real numbers (a fixed default is useless — a dense ontology scores ~0.64 against out-of-domain text). CLI `physis-pro discover <dir> [--threshold T] [--json-out F]`, HTTP `POST /api/v1/discover`; the legacy LLM-synthesis path is kept behind `--ai`.
- [x] **Config dedup**: the 10 copy-paste `load_builtin_*` functions collapsed into a `BUILTIN_ONTOLOGIES` table + one `load_builtin(name)` lookup.

#### 1E — Packaging
- [x] **`setup.py` + `install.sh`**: pip-installable packaging with binary installation.
- [x] **`monitor_resource.sh`**: cgroup-based CPU throttling for heat-throttled environments.

#### 1F — GPU support (investigated 2026-08-12 — **the `rocm` feature is dead configuration**)

**Finding: ONNX Runtime removed the ROCm execution provider upstream.** ORT 1.24.2
(the version `ort` 2.0.0-rc.12 links) has no ROCm EP: `build.py` offers
`--use_migraphx` but no `--use_rocm`, and `onnxruntime_providers_migraphx.cmake`
is the only AMD provider file in the tree. The `ort` crate still exports a
`rocm` feature and a `ROCMExecutionProvider` type, so our `rocm` feature and the
`physis-pro-web-rocm` binary **compile and silently produce a CPU build**:

```
ERROR ort::ep] register `ROCMExecutionProvider`: ROCM execution provider is not enabled in this build.
WARN  ort::ep] No execution providers from session options registered successfully; may fall back to CPU.
```

Confirmed empirically: full suite under `--features rocm` ran `1451% cpu` (~14.5
cores) for 333s with `rocm-smi --showpids` reporting no KFD processes throughout.
Every run since the "enable ROCm ONNX prebuilt download" commit has been on CPU.

Ruled out along the way:
- **Host ROCm SDK build** — ORT's AMD EP needs MIOpen, which Debian does not
  package at all (`apt-cache search miopen` = 0 hits); the host stack is also an
  incoherent HIP 5.7 / rocBLAS 5.5 / rocm-cmake 6.3 mix.
- **AMD `onnxruntime-rocm` wheel** — only published at 1.22.x; `ort` rc.12 needs
  the 1.24 C API, so it cannot initialise against it.
- **Container build** (`rocm/dev-ubuntu-22.04`, ROCm 7.14, MIOpen present, both
  GPUs visible via `/dev/kfd`) — got as far as a clean ORT 1.24.2 checkout before
  hitting the upstream removal above. MIGraphX is not in that image's repos
  either (773 ROCm packages, none named migraphx).

Open options, none started:
- [ ] **MIGraphX EP** — needs AMD's separate `rocm/migraphx` image, an ORT build
  with `--use_migraphx`, and an EP-dispatch change in `src/embed/mod.rs` behind a
  new `migraphx` feature. Risk: unsupported ops silently partition back to CPU.
- [ ] **WebGPU EP** (`--use_webgpu`, Dawn/Vulkan) — vendor-neutral, needs no ROCm
  userspace, and `ort` rc.12 exposes a `webgpu` feature. Most portable option;
  op coverage for BERT-shaped graphs is unproven.
- [ ] **Decide the fate of the `rocm` feature** — it currently promises
  acceleration it cannot deliver. Either delete it with the
  `physis-pro-web-rocm` binary, or keep it strictly as the seam for a future EP.
- [x] **Resolved (2026-08-13): deleted.** The `rocm` feature, the
  `physis-pro-web-rocm` binary, and `build_rocm_test.sh` are gone. Dead
  configuration that promises acceleration it cannot deliver is worse than no
  configuration — and a future MIGraphX/WebGPU EP is a *new* feature with a
  truthful name, not a resurrected one. The finding above stays in `PLAN.md §1F`
  so the investigation isn't lost.

**Payoff check before spending more on this:** the embedder serialises inference
behind a Mutex (`src/embed/mod.rs`), so a GPU cuts per-embed latency, not suite
throughput — realistic gain on the test suite is 333s → ~200s. Low priority
against the TUI browser and the industrial wedge.

#### 1H — Goals carry their text, and persist (2026-08-13)

`Goal` was documented "State of a goal in vector space. **No human-readable
qualia**", and `new_vec` stamps `domain_name: "unknown"`. All five dream
descriptions interpolate that field, so every proposal over an HTTP-created goal
read *"Mutate unknown with noise_std=0.31"* — the engine unable to name what it
was reasoning about. `CoherenceNode` had already abandoned that doctrine (its
`label` exists precisely so a node "can be browsed by content"); `Goal` simply
never got the same treatment. It now has `label`, set from the prompt, and
`name()` used by the proposals.

Goals were also the last tenant state lost on restart — coherence nodes,
LifeOS, OEE, maintenance and axes all persisted, goals did not, so a redeploy
silently answered `"No goals exist"` and dreaming stopped. Now written through
to sled per tenant (one blob per tenant: goals are a bounded working set, unlike
the unbounded node graph that needs per-key writes).

**Structural finding worth acting on:** no test in the suite boots the HTTP
router, and every wiring bug found this session was invisible to all 429 tests —
the rate limiter never layered in, three tenancy readers on the wrong tenant,
`/dream/generate` dropping `description`, and the `load_goals` call site missing
from `with_user` (it compiled clean and persisted nothing). The engine is well
covered; the surface everyone actually calls is not.
- [x] **Done 2026-08-13.** The server moved from `src/bin/web_server.rs` into
  `src/web.rs` (feature `web`); the bin is now 9 lines calling
  `physis_pro::web::serve()`. `build_app()` returns `(state, Router)` so
  `tests/http_surface.rs` drives the real router in-process via
  `ServiceExt::oneshot` — no port, no network, 0.25s for the suite.
  Seven tests cover the bug classes that shipped this session: tenancy
  isolation on reads, verdict storage and correction, readable dream
  proposals, failure-replay selection, holarchy edge visibility, and full
  grid coverage. Each was mutation-checked — reverting the holarchy tenancy
  fix turns `holarchy_edges_are_visible_to_the_tenant_that_wrote_them` red
  with "the writer can see its own edge: left 0, right 1".
  Note for future tests: `oneshot` supplies no `ConnectInfo`, so the rate-limit
  middleware rejects every request until the test injects a peer address —
  the same 500-on-everything the middleware comment warns about.

  **Growth (2026-08-13):** the suite now has ten tests. A persistence test
  (`goals_survive_a_restart`) tears the router down and rebuilds it in-process,
  and it immediately exposed two real bugs no boot-ever test could see:
  - `build_app` never created the data dir before `sled::open`, which *fails
    rather than creates* when the parent is missing — so a fresh install ran
    entirely in-memory (all persistence silently disabled) until something else
    created `~/.physis`. Fixed with `create_dir_all` (`src/web.rs`).
  - Two unconditional background tasks (graph updater thread + 10s Rachmaninov
    tick) held a strong `Arc<AppCtx>` forever, keeping the sled `Db` open so a
    second in-process app (or the test) couldn't reopen the same data dir. Both
    now hold a `Weak` ref and exit when the app state is dropped.
  Two more tests lock in earlier-only assumptions: `scan_is_confined_to_scan_root`
  (canonicalize + prefix check) and `bearer_token_gates_protected_routes` (RBAC
  token format `role:secret`). Full suite: 429 lib + 10 HTTP-surface green, zero
  clippy warnings.

### Phase 1I — Live product demo (2026-08-13)

Asked for by the prospect work: a demo dashboard, not a dev console. Shipped as
`src/bin/ui/demo.html` (self-contained single-file page) served at **`/demo`**
and launched with **`scripts/demo.sh`** (one command; DEMO.md is the runbook).
Not a chart mock: "Seed demo data" drives the real API — OEE board, goals +
dream, a 14-day maintenance backfill ending in an anomaly with a live RUL
projection and a coherence node, and the day's documents linked into the
holarchy graph. Every panel (plant OEE + six-big-losses Pareto, per-machine
cards, live shift report, operational timeline, 5×14 semiotic heatmap, holarchy
graph, classify probes) is computed in-process from the running engine.

Findings that came out of making it live:
- `maintenance/observe` folds into the baseline but **never writes the trend
  history** — RUL needs a series of `check` calls, which is exactly the monitor
  loop, but the asymmetry wasn't documented. `ReadingRequest` now accepts an
  optional `timestamp` (defaults to now) so history can be backfilled — the
  seam a real connector needs to serve RUL with realistic numbers.
- The prod rate limit `60 req/min/IP` is **right for deployments and wrong for
  a seed+load burst (~70 req/min)**. `demo.sh` sets `PHYSIS_RATE_LIMIT=600/60`;
  the prod default is unchanged and the env is documented so nobody "fixes" it
  in prod.
- `/holarchy/ingest` pushes to the sensory ring, it does **not** create graph
  nodes — the demo registers coherence nodes (`/coherence/register`) and links
  them (`/holarchy/edge`) instead.
- One test added (`demo_and_console_ui_are_served`) + `send_text` helper; HTTP
  suite now 11.

### Phase 1J — physis-schola: Study Log + Argument Trainer (2026-08-13)

The learning/teaching application, built on the same move as the demo: new
self-contained page, new stateless machine wired to the real API, zero new
engine concepts.

**Study Log (metadata-for-context layer)** — every study action is a
`StudyEvent` carrying **date/time (`observed_at`), action (generated / edited /
opened / deleted), file path, directory, file type, and place** (room / library
/ lab). Persisted per tenant in sled (`study:log:{uid}`), rendered on
**`/study`**:
- `POST /api/v1/study/log` — append events (capped newest 5000).
- `GET  /api/v1/study/session` — aggregates into a `StudySession`: an
  `OperationalTimeline` (oldest→newest, delete → Warning → incident flag), file
  tallies by type / directory / action / place, and **chronos study rhythms**
  ("edit notes/topology.md every 1.0d").
- `POST /api/v1/study/clear` — demo reset.
- Events project onto `OperationalEvent` (provenance connector `study`), so the
  timeline pipeline is reused untouched; the study connector never mixes with
  shop-floor data.

**Recorder** — `physis-pro study <dir> [--place <where>] [--interval N] [--once]`.
Pure, unit-tested `snapshot_dir` + `diff_snapshots` (mtime/size diff: new path →
generated, changed → edited, gone → deleted; `--once` records the on-disk files
as "opened") poll the directory and POST the events to the running engine. Order
is preserved via sub-second timestamps within a batch.

**Elenchus trainer (slice 3)** — on the same `/study` page: paste an essay →
split into sentences → tag each as claim / warrant / rebuttal (target the claim
warrants/rebutes) → `POST /api/v1/machines/elenchus` → each claim's standing
(supported / contested / unsupported) with the press-here / recalibrate-here
lists. The auto-tag heuristic is a draft convenience, deliberately not magic —
the student owns the Toulmin tagging.

Tests: 5 new unit tests in `study.rs` (diff, context metadata, ordering,
tallies, daily rhythm) + `study_log_sessions_are_served_and_isolated_per_tenant`
(HTTP round-trip + per-tenant isolation: a teacher never sees a student's log).
HTTP suite now 12; lib suite 434; clippy 0 warnings.

### Phase 1.5 — Session 2026-06-12/13 architecture changes

Shipped this arc (all on `main`, classify hot path untouched unless noted):
- [x] **P1 lock split finished** — last two handlers (`context_filter`,
  `maintenance_check`) embed off the global state lock (`spawn_blocking`).
- [x] **Config dedup** — 10 `load_builtin_*` fns → `BUILTIN_ONTOLOGIES` table.
- [x] **Spanner P7** — hard deletes + `sync_full` mirror sync (orphan pruning,
  edges-before-nodes).
- [x] **Embedder cascade hardened** — `embed::semantic_self_test` shared by web +
  CLI + tests; CLI was ungated (Jina fp16 noise → 0.06 verdicts) and used
  pre-P3 mean-centroid scoring. Both fixed; CLI prints embedder provenance.
- [x] **BGE-M3** — measured, **rejected** (CORE 0.923/0.769 vs bge-base
  1.000/0.949, Italian tie, 2.5× slower). Opt-in only. `PoolingStrategy` knob
  (mean|cls) + `token_type_ids` signature detection kept.
- [x] **Italian classification** — same-language ontology anchors lifted the
  Italian tier 0.733/0.600 → **0.867/0.867**, CORE unchanged. Gated at 0.80.
- [x] **Shift report** (`report.rs`) — Italian OEE/anomaly/RUL/countermeasure
  report, Markdown + self-contained HTML; `report --shift`, `POST/GET
  /api/v1/report/shift?format=html`. The sellable artifact.
- [x] **Ontology discovery** (`discovery.rs`) — embedding-gap detector with
  operator-tunable threshold + surfaced sim distribution (a dense ontology
  scores ~0.64 vs out-of-domain, so a fixed threshold is useless); clusters
  gaps → draft ontology JSON. CLI `discover`, `POST /api/v1/discover`; legacy
  LLM path behind `--ai`. Closed Phase 1D.
- [x] **Empedoclean dyad** (`empedocles.rs`, `docs/EMPEDOCLES_DYAD.md`) —
  RESEARCH module, P2-quarantined, wired to nothing. A node field under
  Eros (attraction + synthesis) vs Nemesis (repulsion + decay). The
  `empedocles_criticality` falsifier **confirmed a phase transition**: a
  critical band (heavy-tailed cluster sizes) sits between Love-collapse and
  Strife-scatter. Open: it is *tunable* criticality, not *self-organised* —
  coupling decay/synthesis back into the ratio to self-tune to the edge is the
  next experiment. Promotion gate: a mechanism enters the live coherence core
  only after it shows organising work AND a test proves no classify regression.

#### 1G — TUI wiki browser (shipped 2026-08-12)
- [x] **WIKI tab in `physis-pro-tui`** — browse the UberWiki by *meaning*: describe
  what a thing was about, get cosine-ranked nodes back. Built as a fourth tab
  rather than the standalone binary in PHYSIS_PRO_USECASES.md §2, reusing the
  existing HTTP/event-loop scaffolding; extract it later if it outgrows the tab.
  Uses only endpoints that already existed (`/coherence/nodes`, `/reconstruct`).
  Each row shows the **asserted verdict** as a colour dot (green worked / red
  blocked / yellow inert / dim unjudged) with derived density as a number beside
  it — one colour cannot honestly carry both axes. Respects `PHYSIS_USER`, since
  coherence state is per-tenant and `_default` is empty when nodes were written
  under `aion` / `praxis`.
- [x] **Live re-rank** — results follow the query as you type, debounced on a
  350ms pause with a 3-char floor. Per-keystroke would cost an HTTP hop plus an
  ONNX embed per letter; a pause gives the same feel for one request.
- [x] **Graph walk** (`g` descend, `⌫` climb out) — browses holarchy edges
  rather than similarity. Needed a new `GET /api/v1/holarchy/neighbors`: the
  existing `/holarchy/graph` exports packed `RawNodeKey`s, so a client could see
  that an edge existed but not what it joined.
- [x] **Semiotic heatmap pane** (`m` focuses the map, arrows move the cell
  cursor, Enter scopes the search). Nodes carry no cell tag, so scoping cannot
  filter existing hits without re-classifying every row; it searches with the
  cell's own entry names instead, reaching the same region of the shared vector
  space in one round-trip. **Found while building it, now fixed:**
  `/semiotic/grid` reported entries for only 27 of 70 cells because it
  hand-enumerated eight ontology collections and omitted `custom_domains` —
  where the 20 always-on `EXTRA_ONTOLOGIES` land, and what
  `classification_domains()` scores against. Text genuinely landed in
  `FABRICATE×MAINTAIN` ("Machine Maintenance") while the grid called that cell
  empty; `office_domains` and `machine_domains` were missing for the same
  reason. The handler now builds from the existing `all_domains()` helper:
  **70/70 cells populated, 927 entries** (was 27/70), and 63 lines of
  copy-pasted loops became 32.
- [x] **Dream view** (`d`) — it was NOT blocked on a missing capability, as
  first assumed. `Dream` has always carried `dream_type` and `description`; the
  handler serialised `id/source/embedding/grade` and dropped both, sending two
  768-d vectors per dream instead of the proposal. It also called the 2-arg
  `generate_dreams`, which passes `None` for the scheduler, so the schedule-drift
  and countermeasure generators could never fire over HTTP. Both fixed; vectors
  now opt-in via `embeddings:true`.
  Added `POST /api/v1/dream/replay`, which is the view the browser needed:
  `/dream/generate` works over *goals*, which are pure vectors with no text, so
  its proposals can only ever say "mutate unknown". `core.dream()` runs over
  *coherence nodes*, which carry labels and verdicts. Verified: it replays two
  reported failures and skips a reported success — including a node with density
  1.0 and verdict −1.0, exactly where the two axes disagree.

### Phase 2: User Experience (The "Backoffice" Polish)
- **Web-based Dashboard**: Build a React/Next.js dashboard (using the existing `web` feature) that visualizes the `FileThemeGraph` as an interactive force-directed graph.
- **Natural Language Interface**: Implement a chat-based interface (using the `ai` module) that allows users to query the graph in plain English (e.g., "Show me all invoices related to the Q4 budget").
- **Quality Dashboard**: Create a visual UI for the `QualityTracker` to show failure trends and ROI metrics for the business.

### Phase 3: Monetization Path (The "Business" Model)
- **Tiered Licensing**:
    - **Physis Core (Open Source)**: The engine and basic ontology.
    - **Physis Industrial (SaaS)**: Includes the web dashboard, industrial adapters, and automated quality feedback loops.
    - **Physis Enterprise (On-Prem)**: Dedicated support, custom ontology development, and air-gapped deployment for high-security manufacturing.
- **Industry-Specific "Ontology Packs"**: Sell pre-configured ontology packs (e.g., "Physis for Aerospace," "Physis for Food Processing") that include domain-specific machine health states and regulatory compliance checks.
- **ROI-Based Pricing**: Charge based on the number of "Quality Failures" prevented or the volume of inventory optimized, aligning the cost directly with the value provided.
