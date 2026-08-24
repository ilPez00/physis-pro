# Physis Pro — Full Feature QA Verdict (2026-08-24)

> Tester: Hermes agent session. Method: isolated build (CARGO_TARGET_DIR=/tmp/qa-target,
> vendor pubkey stamped, dev licence minted via vendor tool), 648-test suite + live CLI
> and HTTP exercise with the tester as quality judge.
> NOTE: a sibling session was actively editing src/report.rs + admin_study.rs during QA;
> all findings below are against commit 1bb9a8d tree state at test time.

## 1. Test suite results — PASS

| Suite | Count | Result |
|---|---|---|
| physis_pro lib tests | 546 | ✅ all pass (2.9s) |
| integration tests (smoke, classification, industrial_demo, epistemic, holarchy…) | ~90 | ✅ all pass |
| http_surface (--features "web industrial") | 46 | ✅ all pass (66s) |
| physis-core | 116 | ✅ all pass |
| **Total** | **~648** | **0 failures** |

## 2. Licence gate — PASS (excellent)

- No licence → refuses to run ✅
- Wrong vendor key → `signature is not valid for this build's vendor key` ✅
- Valid licence signed by matching key → banner `licensed to Dev QA — 5 seats, expires 2027-12-31` ✅
- Vendor tool round-trip (keygen → sign → inspect → product validation) works end-to-end ✅
- Web server: refuses loopback bind without token or PHYSIS_DEV=1 ✅ (good security default)
- doctor reports licence status correctly ✅

## 3. CLI feature-by-feature verdict

| Command | Status | Quality judgment |
|---|---|---|
| `report --shift demo-shift.json` | ✅ | **Best-in-repo output.** Italian shift report: plant OEE 78.5%, per-machine tables, loss Pareto, anomaly 4.4σ, RUL ~2h10m, concrete countermeasures. Customer-ready as-is. |
| `classify <file>` | ⚠️ | Runs, but top cell FABRICATE×GUIDE "Shop Floor Lead" for a bearing-overtemp sentence is semantically WRONG; scores compressed (1.000 vs 0.992 vs 0.991 — no discrimination). Random-projection embedder limits; ONNX model absent. Judged: not sales-demoable without real embedder. |
| `machine chronos -i events.json` | ✅ | Clean JSON: detected weekly lubrificazione rhythm (period 7.0d), flagged slipping. Good pure-function design (no embedder/disk). Input envelope (`{"events":[{label,at}]}`) only discoverable from source. |
| `vault <dir>` | ✅ | 2 docs → 4 labeled nodes, dedupe working. |
| `node-search` | ✅ | Token-budgeted similarity search w/ rehydration from sled. Sensible ranking (0.799/0.709). |
| `node-edit <id> <label>` | ✅ | Exchange-parts edit: label re-embedded, structure preserved, coherence recomputed. Verified via search. |
| `pack <dir> <query>` | ✅ | 27/150 tokens, per-chunk line ranges reported, cache misses counted. Exactly the deterministic-RAG promise. |
| `scan <dir>` / `query` | ❌ UX bug | scan output renders fine, but trie is session-local: separate `query` invocation ALWAYS returns "No results" (cli.rs run_query reads self.mapper built in-process). Query-after-scan across invocations does not work. |
| `ingest` | ❌ crash | Classifies files into themes OK, then `Error: File exists (os error 17)` in VfsOrchestrator::sync_graph on re-run: symlink exists-check passes for DANGLING symlinks but create fails (vfs/orchestrator.rs:44 uses `.exists()` which follows symlinks). Second+ ingest always crashes. Also: no dir arg (uses cwd silently). |
| `timeline <events.json>` | ✅ | Incident view w/ summary counts, headline, window, suggested next action. Strict schema (event_key/observed_at/kind/status/provenance.connector required) — good for normalization, hostile for humans; needs doc example in --help. |
| `translate` | ✅ | Pirahã lense strips to causal tokens ("spindle bearing overheated lathe stopped"). Niche but works. |
| `pass`/`fail`/`quality` | ✅ | Failure persisted w/ cell penalty (FABRICATE␀CREATE −0.06). Cell separator renders as literal \u0000 in output — cosmetic bug. |
| `score <text>` | ⚠️ | Top-20 cells all ≥0.90 — same non-discrimination as classify under RP embedder. |
| `inventory` / `order` | ⚠️ | inventory reads sled fine. `order` is a STUB: prints suggestion text, persists NOTHING (main.rs:904-909), so inventory never shows it. Human-in-loop story incomplete. |
| `doctor` | ✅ | Clear sections, actionable messages (download_models.sh hint). |
| `dream` CLI | ⚠️ | Empty output (no Inert/Failure nodes in fresh store) — correct behavior, zero feedback to user though ("no dreams possible" would be kinder). |
| `wiki` search | ⚠️ | Honest empty result + helpful tip. But graph built by `ingest` never becomes searchable because ingest crashes before persisting theme_graph.json. |

## 4. HTTP surface verdict (physis-pro-web :19876, PHYSIS_DEV=1)

| Endpoint | Status | Notes |
|---|---|---|
| POST /api/v1/classify | ✅ 200 | Same RP-embedder quality caveat |
| POST /api/v1/embed | ✅ 200 | 384-dim vector |
| POST /api/v1/compress/logs | ✅ 200 | 3 log lines → joined compression, input_count echoed. Accepts array only (string→422 w/ clear serde error) |
| POST /api/v1/context/filter | ✅ 200 | Field named `input` (string) |
| POST /api/v1/goals | ✅ 200 | Tenant-scoped, embedding returned |
| GET /api/v1/pdca/plan · /stats | ✅ 200 | Plan visible tenant-scoped; stats honest zeros |
| POST /api/v1/dream/generate | ✅ 200 | Correctly REFUSES while PDCA active unless force:true — good domain logic. Forced dreams return interpolate/extrapolate proposals w/ ids for evaluation |
| GET /api/v1/coherence/nodes?q= | ✅ 200 | Score+cell+embedder provenance per node |
| POST /api/v1/coherence/edit | ✅ 200 | Auto-cell reassignment shown (before→after), coherence recomputed. Verified persisted |
| POST /api/v1/coherence/query | ✅ 200 | Typed question enum (WhyIsBelieved…); answers structured (supporting/contradicting evidence, coherence profile). "Subject not found" handled gracefully |
| **POST /api/v1/shifts/import** (NEW) | ✅ 200 | CSV raw body (text/csv) or JSON array/{shifts:[…]}. Partial acceptance: bad rows rejected WITH line numbers + reasons incl. supported-column list — genuinely good API citizenship |
| **GET /api/v1/shifts** (NEW) | ✅ 200 | count + machines list |
| **GET /api/v1/shifts/report** (NEW) | ✅ 200 | Per-machine OEE decomposition + plant rollup + loss Pareto by factor. Numbers internally consistent (verified availability/perf/quality math) |
| **POST /api/v1/shifts/clear** (NEW) | ✅ 200 | Cleared, verified count=0 |
| Tenant isolation (X-Physis-User) | ✅ | tenant2 saw ZERO of tenant1's shifts/goals/plans |
| Scan-root confinement | ✅ | `/tmp/qacorpus` outside server cwd → 403 `path outside scan root` — path-traversal guard works |
| GET /agent · /api/v1/agent/caps | ✅ 200 | Agent tool registry lists graph_search/node_search/classify… |

## 5. Bugs & defects found (ranked)

1. **[HIGH] `ingest` crashes on re-run** — VFS dangling-symlink EEXIST (src/vfs/orchestrator.rs:44 `.exists()` follows symlinks; use `symlink_metadata()`). Blocks wiki/graph features entirely after first run.
2. **[HIGH] `query` CLI can never find scanned data** — trie is process-local (src/cli.rs:194); cross-invocation persistence missing. Either persist trie or merge query into scan pipeline.
3. **[MED] `order` command is a stub** — prints approval message, persists nothing (src/main.rs:904). Inventory feature loop broken end-to-end.
4. **[MED] RP-embedder score compression** — classify/score return 0.88–1.00 for everything; top cell often semantically wrong. Fine for plumbing tests, NOT for customer demos. Mitigation exists: ship ONNX models (scripts/download_models.sh) — untested here, weights absent.
5. **[LOW] `quality` prints raw `\u0000` cell separator** in human output.
6. **[LOW] rachmaninov drift WARN spam** every 10s at sim=0.0000 when holarchy has no nodes — should rate-limit or stay silent below a node threshold.
7. **[LOW] timeline/machine input schemas undiscoverable** — error messages teach one field at a time; add example JSON to --help.

## 6. Overall judgment

**Core revenue path is REAL and works:** report generation (CLI + new shifts API), OEE math, anomaly σ-distance, RUL projection, Italian output quality — this is what Generalmeccanica/Moro/CB pitches sell, and it holds up. The new /shifts surface (PLAN 5.1/5.3) is the best-engineered part: partial acceptance, line-numbered rejections, tenant isolation, verified cleanup.

Licence gating is production-grade. Test suite is green and meaningful (not vanity tests).

**Not customer-ready yet:** knowledge-management layer (query/ingest/wiki chain broken by 2 HIGH bugs), order/inventory loop (stub), and any classify-dependent demo until ONNX weights are downloaded and validated (RP fallback compresses scores to noise).

Recommended fix order: #1 → #2 → #4 (download+wire ONNX) → #3, then re-run this QA suite.
