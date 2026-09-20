# Changelog

Ritual: bump `version` in Cargo.toml, add a `## [x.y.z]` entry here,
run `bash scripts/release-check.sh`, tag `v<version>` on green,
push tag. Every merge to main stays releasable; tags never move.

All notable changes to physis-pro are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Fixed
- Ingest no longer loses a text file's body to a misclassification (PH-015).
  `office/invoices.md` matched the voice-file heuristic on the substring
  "voice", was routed to the audio-transcript stub, and ingested with an
  **empty body** — so a `bearing` search matched one file instead of two. The
  heuristic has matched whole tokens since `f383789`; the extractor now also
  refuses to return an empty body for a non-audio path, which turns any future
  misclassification into a wrong-*kind* bug instead of a silent-loss bug.
  Test: `ingest::tests::misclassified_text_file_keeps_its_body_ph015`.

### Fixed
- The semantic self-test gate can now fail loudly (PH-004). The probe and the
  `selftest` subcommand were already committed, but the packet's own gate
  `cargo test semantic_self_test` matched **zero** tests and exited 0. Two
  tests now carry that name: the unit test runs the random-projection arm
  unconditionally and asserts its own precondition before skipping the
  real-weights arm, and a child-process test runs the built binary with
  `PHYSIS_EMBEDDER=random-projection` and no licence file, requiring exit 1
  with a `FAIL` verdict. `packets/reports/PH-004.md`.

### Changed
- **MS-002 is `done`** after independent review: citations re-checked at both
  `ed8dc29` and `54b8a8d`, amendment A1 confirmed binding, the
  zero-production-caller and zero-instrumentation claims confirmed. Its
  runnable mutation matrix was deferred by the report and is now carried by a
  new packet, **`MS-002-harness`** (`review`), which names the oracle semantics,
  six mutation arms and properties P1–P5.
- **MS-003 returned to `todo`** after independent review. Its report describes
  the `Transform::apply` acceptance gate wrongly (it names
  Counterfactual/Substitution/Analogical and says all five dimensions must be
  `1.0`; the code sets logical/empirical/temporal/causal/procedural and
  hard-requires only logical and empirical, `transform.rs:633-639`) and
  misquotes PH-051 ("0/5 cleared cross-representation transfer" against
  PH-051's own 4/5 transfer, 0/5 arithmetic-specific, 0/5 held-out). Four
  citation fixes required; no re-execution.
- `.env.example` documents `PHYSIS_LOG` — the only log-level variable any code
  reads. It had been absent since the tracked `.env` was purged, so a fresh
  deployer could not discover it (PH-001).
- `default` features: **no change made**, but the `mcp` question is now
  measured rather than assumed — +35.9 MiB (486,490,640 → 524,115,912 B) and
  +31 s per incremental build, gating only the legacy `serve` listener while
  the MCP surface in use (`system serve`) ships in the default build.
  Recommendation on record in `packets/reports/PH-001.md`; the decision is
  PH-006's input and stays human.

### Documentation
- `packets/reports/PH-001.md` — the PH-001 report, written nine days after the
  claim (the packet had been left `claimed` with its work committed).
- `packets/reports/MS-001.md` §"Verification note (2026-09-20)" — reviewer
  pass with **amendment A1**: `OntologyDeltaReport.node_deltas` order and
  `net_coherence_delta`'s float summation order are unspecified, because
  `find_neighbors` returns HashSet iteration order. Compare as a sorted set,
  floats to epsilon 1e-6.
- `packets/reports/PH-021.md`, `PH-022.md` — reviewer passes; both gates hold
  in full (the two `notebook.rs` clippy errors they cited are gone).
- `packets/README.md`, `PLAN.md`, `AGENTS.md` — packet board state re-measured
  2026-09-20; MS-001 done, MS-002/003 released, PH-023 released, PH-015 and
  PH-001 in review.
- `packets/reports/PH-004.md`, `MS-002.md` §"Verification note",
  `MS-003.md` §"Verification note" — the PH-004 report and two independent
  reviewer passes.
- `packets/MS-002-harness.md` — new packet carrying MS-002's undelivered
  runnable oracle.
- `PLAN.md`, `packets/README.md`,
  `docs/plans/2026-09-20-queue-after-reviews.md` §1b/§3/§4 — board state
  reconciled after the follow-up pass: PH-004 `done`, MS-002 `done`,
  MS-003 `todo`, MS-004 blocked on MS-002-harness, MS-005 on MS-003.

## [0.1.21] - 2026-09-19

### Added
- `physis-pro onboard [--max-sessions]`: prepopulate the store from this
  machine — shell histories, Claude sessions, opencode sessions, codex
  threads, hermes memory + plans, mesh fleet memory (traps/decisions).
  Re-runs dedupe by label.
- `history <file>` accepts shell histories and Claude session JSONL directly.
- `/health` gains `image_embedder: {kind, degraded}` mirroring the text slot.
- `scripts/bench-pack.sh` (PH-007): cold/warm pack benchmark with integrity
  check and assert-under thresholds.
- `scripts/check-test-tiers.sh` (PH-009): fast-tier floor (≥1300 tests),
  zero bare `#[ignore]`, prints the buried heavy tier.
- `scripts/release-check.sh` (PH-010): the release ritual as a gate
  (version > last tag, changelog entry, licence consistency, clippy, full
  suite, tiers, bench-pack smoke) with `--self-test`.

### Changed
- Image endpoints (`/embed`, `/classify` with image) resolve the registry
  image slot first, then the text embedder's own capability, else answer
  **503 + machine-readable reason** — never a silent text fallback (PH-008).
- `EmbedderRegistry` honors the dedicated `config.image` slot; new
  `VectorEmbed::handles_images()` probe (clip/jina).
- Test suite 790 s+ → ~105 s (PH-009): process-wide shared classifier
  (http_surface 253 s → 54 s), explicit config-RP honored without gate,
  embedder-agnostic CLI tests offline (22 tests: 185 s → 3 s).
- Confidence inversion closed (PH-005): shared `EmbedderHealth`, all surfaces
  read it; degraded embedders are reported, never hidden.
- Agentic workspace P1–P3: effective settings with origin, cancellable
  studio ask, trail-recorded nav intent + HUD resume, staged-diff handoff
  with HEAD stale-base check.
- JEV (r2-reasoning) provider: config knobs + env overlay + cascade entry,
  keys redacted in `config`/settings surfaces.

### Fixed
- Decimal-number dead branch (PH-022, verified by gate recheck).
- Log level was unconfigurable: the server reads `PHYSIS_LOG` (EnvFilter), and
  the repo named neither that variable nor the right one — the deployment docs
  said `PHYSIS_LOG_LEVEL` and `.env` had a `PHICS_LOG_LEVEL` typo, so setting
  the level did nothing. Both docs now name `PHYSIS_LOG` (PH-001).
- Shell history import tolerates non-UTF-8 bytes (lossy read).
- UTF-8-safe truncation in all history parsers (byte-slice panic).

### Known limitations (packets, not regressions)
- PH-006 Pro-tier gating: BLOCKED on the missing PH-001 features decision.
- PH-023 NLI discriminator: gate unmet, no local NLI server in this env.
- First tagged release pending human go-ahead (PH-010 step 3).

## Earlier history (pre-existing entries, kept verbatim)

Note: entries below use an older numbering span (0.5.0–0.9.7) that does not
line up with the crate version (0.1.21); that drift is recorded, not rewritten.


Published as `physis_pro` 0.1.9 (physis-core 0.1.8).

**Security — a remote error body could permanently disable a shared LLM
provider.** In 0.1.6 through 0.1.8 a provider was taken out of the cascade by
substring-matching the text of a failed HTTP response. OpenAI-compatible
endpoints quote the offending request back inside `error.message`, and that
request carries the caller's prompt, so a prompt containing a phrase like
`does not exist` that provoked any 4xx would match. The removal was permanent
— `mark_unavailable()` wrote a flag that only `Provider::new()` ever cleared —
and the cascade is process-wide, so the loss hit every tenant, not the one who
caused it. Repeating it across configured providers reaches
`No provider available` for the life of the process.

Detection now reads only the machine-written `error.code` / `error.type`
fields against a fixed set of vendor codes, never the message prose, and skips
bodies that are not JSON. `invalid_request_error` is excluded — it is the
envelope for ordinary bad arguments and says nothing about provider health.
The park became a bounded 15-minute cooldown rather than a latch, applied to
the `401`/`403` path as well, so a rotated key or a corrected model id takes
effect without a restart.

`0.1.6`, `0.1.7` and `0.1.8` are yanked. A yank stops new dependents from
selecting them but does not touch a project whose `Cargo.lock` already pins
one, so anyone on those versions must upgrade to be fixed. Full text in
[`security/advisories/`](security/advisories/physis_pro-provider-availability.md);
`physis-core` is unaffected, the provider cascade living only in Pro.

**Accessibility — the dashboards now meet WCAG 2.2 AA on contrast and target
size.** Measured in a browser rather than read off the stylesheet. Six defects,
five of them a rule that belonged to the shared system being left to each page
to remember: `ui-shell.js` emits `<span class="err">` on every page while
`physis.css` defined only the compound `.chip.err` / `.toast.err`, so Pulse and
Agent — the two pages that never grew a private copy — rendered API errors in
the muted body colour at 3.76:1. Product-nav pills measured 22px against the
24px SC 2.5.8 minimum (the web figure; the 44pt one is native guidance and
would have cost the density this surface is built on), shared controls 23.2px.
Raw hex `#5c6f92` and `#5a6a8a` for muted text failed AA on four pages, and
`.k-error` — the badge marking a failed agent run — was white on rose at
2.68:1. All six pages now report zero contrast and zero target-size failures.

**Also.** Unknown tool names in an agent run return 400 instead of silently
running with an empty registry. Provider discovery reads the conventional
vendor key names (`GROQ_API_KEY`, `GEMINI_API_KEY`, and so on) alongside the
Physis-specific ones, and the retired groq/cerebras model ids and moved GitHub
Models endpoint are corrected.

## [0.9.6] — 2026-08-24

Published as `physis_pro` 0.1.8 (physis-core 0.1.7).

**Classification precision restored — max scoring ships (PLAN 8.8,
physis-core 0.1.7).** A strategy sweep under bge-base-en-v1.5 showed the
adaptive top-k smoothing (shipped 2026-08-18) had become a dilution machine
after the 2026-08-14 ontology expansion: averaging dragged precise anchors
toward cell means. Cell score is now **max cosine to any member entry**
(what the docs always claimed): CORE top-1 cell 0.636 → **0.782**, ITALIAN
top-1 domain 0.667 → **0.867**, domain accuracy unchanged (0.891).
`TopKStrategy::Adaptive/Fixed` stays reachable for recalibration sweeps, and
the sweep itself ships as an ignored diagnostic test. Accuracy floors carry
their full measurement provenance.

**Phase 2 complete — pins fully explicit.** `physis-pro node-edit --clear-pin`
retracts a pin from the CLI (`PinEdit::Clear`), closing the last seam after
the web API's `"cell": null`. Verified: identity survives the swap, pin echo
works, Clear removes it.

**Decision Planner: versioned scenarios with impact analysis (PLAN 7.2–7.4).**
New `web/scenarios.rs` over the shared tenant sled:
`POST /api/v1/planner/scenarios/save` persists a finished simulation as an
immutable draft version; every changed item becomes a dated signifier shift
(`hypothesis: true`, never a silent fact); `GET /planner/scenarios/:id` lists
versions newest-first per tenant; `POST …/:id/promote` flips status explicitly
and reversibly — promoting another version demotes the previous one.
Each version stores an **impact analysis** (PLAN 7.2): newly covered /
uncovered records at the confidence threshold, per-cell population deltas
(the report lines whose numbers would move), and mean confidence drift.
Coverage gaps are now first-class scenario output.

Also: PLAN.md statuses synced against reality — Phase 2 and Phase 5B verified
complete; Phase 7 complete except the product surface.

## [0.9.5] — 2026-08-24

Published as `physis_pro` 0.1.7.

**Query recalls persisted nodes (QA 8.2).** The trie is process-local — only
an in-process `scan` fills it — so `physis-pro query` answered "No results"
even with a full node store on disk. It now also searches the persisted
coherence nodes of the `_default` tenant (what /scan, watch mode, vault and
history imports register), printing `[score] label (id)` lines under any trie
hits; embedder-mismatched nodes stay excluded via existing provenance checks.

**Orders persist with cooldown (QA 8.4).** `physis-pro order <item> <qty>`
printed a line and vanished. It now goes through `InventoryTracker::
place_manual_order`: registers unknown SKUs on first sight, stamps the
order-cooldown clock (duplicate orders inside the window are suppressed with
the remaining time), appends to a sled-persisted order log, and never touches
stock levels — ordering is not receiving. `physis-pro inventory` renders the
last ten orders and hides internal bookkeeping keys from the item dump.

**Quality cells render readably (QA 8.5, physis-core 0.1.6).**
NUL-joined cell keys (`DOMAIN\0MODE`) leaked invisible glyphs into
`physis-pro quality` output; summaries now display `DOMAIN/MODE`.

**No phantom drift on an empty holarchy (QA 8.6).** Below three nodes the GNN
context vector is degenerate and every drift warning was noise:
`tick_holarchy` skips PDCA until the graph holds >=3 nodes, and Rachmaninov
ignores zero-norm vectors outright.

**Machine schemas discoverable (QA 8.7).** `physis-pro machine <name>
--schema` prints a ready-to-edit example input JSON — valid enough to pipe
straight back into `machine <name>` (a test runs every example through its
reducer). Works pre-licence like `--help`. `timeline --help` documents the
OperationalEvent record shape.

Also: physis-core studio_lab playground + `native_dim()` helper.

## [0.9.4] — 2026-08-24

Published as `physis_pro` 0.1.6.

**Interest daemon (new `daemon` feature + binary).**
`physis-interest-daemon` observes local work and distills it into an interest
profile over the ontological grid. Three feeds: clipboard polling deduped by
content hash (only a 280-char preview + SHA-256 persist, never the full
text), notify watchers on configured directories, and document analysis —
`.pptx`/`.docx` unzip to text runs plus OOXML core properties (title,
creator, dates, slide count), plain text family read directly. Observations
land in a per-tenant sled store; the profile classifies each item through the
shared CellClassifier and applies a 14-day half-life decay, so it answers
"what has this user been into lately". Read API on loopback:
`GET /api/v1/interest/profile`, `/api/v1/interest/events`, `/health`.
The feature stays out of `full` — arboard wants a display server at runtime.
Verified end-to-end: file drop → analyze → store → profile query.

**Semantic interests.** The daemon now runs the same embedder cascade as the
web server: BGE-base via ONNX when `--features embed-onnx` is on and the
model passes the semantic self-test, deterministic random-projection
otherwise (`PHYSIS_EMBEDDER=rp` forces offline). Ontology seeds come from the
shared embedding cache, so restart after the first boot skips ~650
re-embeddings; classifier ready drops from minutes to seconds. Provenance is
logged at startup — every interest score knows which embedder produced it.

**Durable shift history (PLAN 5.1 + 5.3).** Shift records now persist per
tenant in sled (`shifts:history:{uid}`, newest-capped at 5000) instead of
living only inside the stateless report call. `POST /api/v1/shifts/import`
accepts CSV or JSON with partial acceptance: valid rows land, rejected rows
come back with line numbers and the reason (bad numbers, good > total, unknown
stop reasons, non-RFC-3339 timestamps). `GET /api/v1/shifts` lists history
(`?machine=` filter), `GET /api/v1/shifts/report` aggregates every stored shift
into one cumulative OEE report per machine plus the plant rollup, and
`POST /api/v1/shifts/clear` resets a tenant. Zero-valued downtime cells are
treated as "no stoppage" — machine exports use them constantly.

**Report provenance + evidence export (PLAN 5.2).** Reports can now answer
"where do these numbers come from?". `ReportMetadata` (source, generated-at,
shift/machine counts, data window) renders as a line under the title of both
the Markdown and HTML shift reports and rides on the JSON responses;
the live-board handler stamps "live-board", history reports stamp
"shift-history". `GET /api/v1/shifts/export` returns a self-contained JSON
evidence bundle — raw records plus per-record OEE recomputed at export time,
aligned index-for-index — so a customer can re-derive every headline figure
without trusting our aggregation. The history report endpoint also gained
`?format=md|html` for customer-ready rendered documents straight from stored
history.

**VFS re-ingest crash fixed (QA 8.1).** `sync_graph` checked link existence
with `.exists()`, which follows symlinks — a dangling link (source renamed or
deleted) reported "absent" and `symlink()` then failed with EEXIST, crashing
every ingest after the first. Existence is now checked with
`symlink_metadata`, stale links are re-pointed at current targets, and links
left over from removed files are pruned so repeated ingests converge.

## [0.9.3] — 2026-08-23

Published as `physis_pro` 0.1.5.

**Agent surface hardening.** The OpenCode sandbox's containment check relied on
`Path::starts_with`, which compares whole components without normalizing —
`scan_root.join("../../etc")` passed it and escaped at the OS level. `..` is
now rejected outright and both paths canonicalized; caller flags precede `--`
so an untrusted task cannot arrive as an argument; abort requires the owning
tenant, and unknown ids are indistinguishable from foreign ones.

**Agent answers judged.** When `agent.harness_gate` is on, every final answer
runs through `CoherenceHarness::evaluate` and the verdict streams as its own
SSE event beside the answer. Reports only — no suppression, no auto-revision,
no self-grading hypothesis registration.

**Dashboard fetches unified.** Console, Demo and Study carried local
`getJSON` copies that built `//api/v1/...` protocol-relative URLs at the site
root; all pages now share `ui-shell.js`'s `apiUrl()`. The agent trace renders
error events instead of discarding them and explains an empty stream rather
than failing silently.

**Build repair.** Duplicate `life_os`/`rachmaninov` module declarations broke
`--all-features`; `ai::harness` imported `crate::web` unconditionally,
breaking the licence-generator build. Both fixed; clippy is clean under
`-D warnings`.

## [0.9.2] — 2026-08-23

Published as `physis_pro` 0.1.4 and `physis-core` 0.1.5.

**On-disk format.** `physis-core` 0.1.5 writes `nodes.json` as a versioned
object rather than a bare array of nodes, because the array shape could only
hold nodes and silently discarded hypotheses, contradictions and the audit
trail on every save. Reading is backward compatible — an existing
array-shaped file loads unchanged and is upgraded in place on the next save —
but anything parsing `nodes.json` externally will see the new shape. The Rust
API is unchanged: `to_json`/`from_json` keep their signatures.

### Added
- `physis` — a single front door over both editions, installed by `physis-core` alongside `physis-core`. `physis -h` is one help screen covering Core and Pro, marking which side is installed; every other argument is forwarded unchanged to the executable that owns it (`exec`, so subcommand `--help`, exit codes and stdio are identical to invoking it directly). Names present on both sides resolve to Core, so they mean the same thing whether or not Pro is installed.
- `physis upgrade`, and a **Pro / Upgrade** tab in the studio backed by `GET /api/edition`: what Pro adds, whether it is installed, and where its executables are. Core links nothing from Pro — the two are joined at runtime by locating the Pro executable on disk — so Core keeps working, and keeps its licence, without it. The CLI banner and the studio panel read from the same detection and cannot drift.
- `physis_core::edition` — side-effect-free detection of the installed executables, preferring a build sitting next to the running one so the front door works from a plain `cargo build` tree.

### Changed
- Classification scores are rescaled by the maximum instead of clipped at it. `classify` multiplies a blend of two cosines by a population weight of up to 1.2, so a well-populated cell scored above 1.0; consumers clamp there, and because the overflow is common the leading cells all flattened onto exactly `1.000`. The CLI printed a ten-way tie with identical full bars over scores that were in fact distinct and correctly ordered (1.0748 … 1.0482). Dividing by a positive constant is order-preserving, so rankings and the argmax are unchanged — only the ceiling returns to 1.0 and the spread survives.
- The studio header reports `entry_count()` rather than `all_domains().len()`. It read 730 entries against the CLI's 623 because `all_domains()` folds in `machine_domains`, a set `classification_domains()` never yields, so 107 of those "entries" could not be scored or reached from any cell in the grid displayed beneath the number.
- `physis-core classify` states that the CLI builds a random-projection embedder unconditionally, draws its bars relative to the rows shown rather than an absolute 0..1 scale they all saturate, and reports the margin over the runner-up — flagging a field tighter than 0.02 as unresolved.
- `physis-core search` reports the number of matches found rather than the cap requested, keeps both ends of a long label so hits under one long directory are distinguishable, never cuts inside a file name, and collapses labels to one line so a row stays a row.
- `physis-core discover` marks a truncated sample list, which previously made a cluster of three read as a miscount.
- `physis-core replay --subject` resolves an unambiguous id prefix, so the 8-character ids `hypothesis list` prints can be pasted straight in; unknown subjects are now distinguished from subjects with no recorded belief.

### Fixed
- `PhysisCore::to_json` serialized only `self.nodes`, so every save silently discarded hypotheses, contradictions, provenance chains, typed edges, process cycles and the epistemic audit trail. `physis-core hypothesis create` printed "Registered hypothesis [id]" and the next `hypothesis list` reported zero; `audit` and `contradiction list` were empty for the same reason. Three advertised features could not survive a process exit.
- Studio links were rendered in the user-agent default blue, near-unreadable on the dark background; the studio had never had a link before the upgrade panel.
- The Console dashboard shipped ~950 lines of JavaScript after the closing html tag — 19 functions covering hypotheses, contradictions, epistemic audit, explanation, discovery, the coherence harness and the file browser. The browser ended the document there and painted the source as visible page text, while every panel those functions drive rendered and did nothing. Restored into the script element, with the five `$('#id')` lookups, two mis-quoted template-literal URLs and one bad plural inside it repaired.
- `format_heatmap_table` padded with `{:<8}`, a minimum width rather than a maximum, so "BRAINSTORM" ran into "MAINTAIN" as one word and a full-length bar pushed its row out of step with the header.
- The web server printed its "listening on" line before binding, so a port clash announced success and then panicked on a raw `unwrap`. It binds first and names the address it could not take.

## [0.9.1] — 2026-08-23

Published as `physis_pro` 0.1.3 and `physis-core` 0.1.4. (This file numbers the
product; the crate versions on crates.io are their own, lower sequence.)

### Added
- `PHYSIS_CORE_DIR` overrides the physis-core state directory (`nodes.json`, `quality.json`, `custom_ontology.json`). Both the CLI and the studio resolve the graph through one function, so previously there was no way to work on a per-project graph, mount a volume in a container, or keep a test run away from the real `~/.physis-core`. Unset, the shared `$HOME/.physis-core` default is unchanged.
- physis-core studio rebuilt as a six-tab app (Classify / Semiotic grid / Ontology / Corpus / Discover / Quality) sharing the Physis Console design language: header chips, nav tabs, status pill, toasts, `?` overlay, `1`–`6`, `Ctrl+K`, `Ctrl+Enter`
- Studio API for the coherence graph, previously CLI-only: `/api/core/snapshot`, `/api/core/nodes`, `/api/core/scan`, `/api/core/search`, `/api/core/assert`, `/api/core/dream`, plus `/api/health`
- Studio classification now returns the nearest ontology entry, per-cell facets, and the corpus passages closest to the same text
- Studio assets split out of the HTML into `app.css` / `app.js`, served from the same binary
- `PHYSIS_STUDIO_HOST` to opt the studio out of loopback-only binding
- `physis-pro doctor` command — validates config, data dir, scan root, embedder, and license
- `--version` flag on all binaries
- `ui-shell.js` — shared runtime for Console, Demo, and Study dashboards
- Welcome overlays with onboarding on first visit
- Grouped console navigation into Signal / Meaning / Work sections
- Hover tooltips on technical jargon
- Empty-state CTA buttons ("Seed demo data")
- Last-updated timestamps on status pills
- Keyboard shortcuts overlay (press `?`)
- Active tab persistence in `localStorage`

### Changed
- `/api/ontology/upsert` accepts a minimal payload: `axis_kind`, `axis_name`, `unit` and `hints` now default to empty, so a scripted upsert needs only `name` plus the two cell coordinates. `domain` and `mode` stay required — they are the cell key, and defaulting them to `""` would mint a nameless cell that then appears as a real axis in the grid.
- `physis_pro` no longer declares `license = "MIT"`. `LICENSE.txt` is a proprietary EULA enforced by the Ed25519 gate in `src/license.rs`, so the MIT tag was not merely redundant alongside `license-file` (which cargo warns about) but stated terms the software is not offered under. `physis_pro` 0.1.2 carried that tag on crates.io and has been yanked.
- Removed `QualityTracker::penalty_scale`: private, initialised to `1.0`, never read — a documented tuning knob that did nothing.
- `data_dir` defaults to `./.physis` (current working directory) instead of `~/.physis`
- `scan_root` defaults to parent of `data_dir`
- Loopback auth bypass now requires explicit `PHYSIS_DEV=1` environment variable
- `build_app()` returns `Result` for testability
- Replaced deprecated `rand` APIs (`gen` → `random`, `gen_range` → `random_range`, `from_entropy` → `from_os_rng`)
- `ort` updated to `2.0.0-rc.13`
- `rand` updated to `0.9`, `rand_core` updated to `0.9`
- Added `tracing` + `tracing-subscriber` for structured JSON logging
- Added `tower-http::TraceLayer` for HTTP request tracing
- Added `trace_tenant` middleware to attach `X-Physis-User` to tracing spans

### Fixed
- `/api/v1/embedder` reported a random-projection embedder as `semantic: true`. It derived `semantic` as `!degraded`, but the two were deliberately split: `degraded` is the alarm (a trained model was expected and silently emitted garbage) while `semantic` is the capability. Explicitly requested offline mode is non-semantic yet correctly not degraded, so the endpoint whose purpose is making degradation observable was reporting the offline embedder as a real model. It now reads the `semantic` flag that `SharedState` already holds and `/health` already used.
- `cargo check --all-targets` did not build: the two `empedocles` examples import a module behind an off-by-default feature and now declare `required-features`, so cargo skips rather than fails them.
- `ui-shell.js` is now actually served (`GET /ui-shell.js`, public — a `<script src>` sends no Bearer token) and no longer collides with the page-local `$`, `S`, `save` and `toastTimer` declarations it was loaded alongside; that collision was a SyntaxError that killed the entire inline script on Console, Demo and Study
- Studio grid axes are derived from the loaded ontology instead of the `HumanDomain`/`HumanMode` enums, so entries on any other domain or mode are visible and editable rather than scoring invisibly
- Studio directory scan splits files into passages (whole files over 5000 characters were silently dropped), skips build/VCS directories, embeds outside the write lock, and caps one scan at 2000 passages
- Removed stray `#[cfg(feature = "spanner")]` and `#[cfg(feature = "mcp")]` attributes that were incorrectly gating non-feature code
- Fixed `classify_handler` brace balance corrupted during module extraction
- Fixed `heatmap_handler` placement and brace balance in wiki_semiotic module

## [0.9.0] — 2026-08-18

### Added
- Industrial backoffice intelligence: OEE tracking, maintenance baselines, inventory
- Document ingest: classify and process structured/unstructured documents
- Vault import: Obsidian/markdown, plain notes, git log → labeled core nodes
- Personal history import: browser bookmarks, history, OPML feeds, chat JSONL
- Praxis backfill: notebook entries → behavioural records with verdicts
- Spanner Graph sink: durable-memory mirror
- File theme graph: ontology-driven file browsing
- Study log + Elenchus argument trainer
- MCP server support
- Voice binary support
- PostgreSQL row-delta ingestion
- SQLite incremental ingestion
- Schema discovery for SQL databases
- Watermark scans for incremental processing
- Per-ontology interpretants
- Salience: interpretant evidence that biases classification
- Offline licensing with vendor generator wrapper

### Changed
- Split `src/web.rs` (7,195 lines) into per-domain handler modules
- Auth gating: loopback bypass requires `PHYSIS_DEV=1`
- CI matrix expanded with voice+tui feature check

### Fixed
- Scope salience by tenant
- Stop losing concurrent observations
- Resolve bundled models by search path
- Make embedding cache durable against failed writes

## [0.8.0] — 2026-07-15

### Added
- Semantic self-test for embedder at startup (P4)
- PDCA stagnation detection
- Dream loop with stochastic generation
- Coherence trie for token-bounded search
- Fixed-token node search with budget

### Changed
- Replaced llmwiki subprocess with in-process Tantivy BM25 (wiki-embedded feature)
- Embedder selection with self-test cache

## [0.7.0] — 2026-06-01

### Added
- Holarchy ingest ring
- Shared knowledge graph
- Ontology overlays (sector/client/project)
- Faceted ontology queries
- Multiple embedder support (ONNX, Jina, CLIP)

### Changed
- Multimodal sensory bus (audio, vision, acceleration, gyroscope, thought capture)
- Single ingestion path into holarchy

## [0.6.0] — 2026-05-01

### Added
- 70-cell semiotic grid (5 domains × 14 modes)
- Coherence tracking (+1 success / 0 inert / −1 failure)
- Exchange-parts semiotic editing
- Quality feedback loop
- Dream replay for corrective actions

### Changed
- Unified embedding space for text, images, video, voice
- Classification against human + machine + office ontologies

## [0.5.0] — 2026-04-01

### Added
- PDCA cycle orchestrator
- Goal tracking in vector space
- Linguistic lenses (Wenyan, Piraha, Sanskrit)

### Changed
- Ontology mapper with trie and vector search
- CLI with scan, query, dream, evaluate commands

## [0.1.0] — 2026-03-01

### Added
- Initial release
- Basic ontological mapper
- CLI interface
- Web server with REST API
