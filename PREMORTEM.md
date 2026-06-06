# Physis Pro — Premortem

> Method: assume it is 6 months from now and the project **failed**. Work
> backward to the causes. Each item: failure mode → why it happens → mitigation.
> Ordered by expected damage × likelihood. Written 2026-06-04.

---

## P0 — Silent embedder degradation in production (most likely killer)

**Failure:** `physis-pro-web` deploys to Railway, `/health` returns `ok`, every
endpoint returns `200`, demos "work" — but classification is **random noise**.
Customers see a cooking photo classified as `BOND×REST 0.04` and conclude the
product is snake oil. We never notice because nothing errors.

**Why it happens:**
- `models/` is **git-ignored** (`.gitignore:4`). Railway builds from the GitHub
  push, so the 1.5 GB of ONNX weights are **not in the build context**.
- `Dockerfile:8` does `COPY models ./models`. On a checkout without `models/`
  the build either fails or copies an empty tree.
- `web_server.rs:830-872` falls through Jina → CLIP → MiniLM → **RandomProjection**
  and the final `unwrap_or_else` *always succeeds* with `RandomProjectionEmbedder`.
  Degradation is **silent**: a single `log::warn!` per image, nothing at startup,
  nothing in `/health`, nothing in `/api/v1/stats`.
- Centroids are then computed from the *same* random projection, so cosine sims
  look plausible (0.0–0.3 spread) instead of obviously broken.

**Mitigation (this is the highest-value dev work right now):**
1. Make degraded mode **loud**: startup banner at `error` level + a
   `degraded:true` flag surfaced on a `/api/v1/embedder` endpoint.
2. Give production a real model-delivery path: `docker-entrypoint.sh` downloads
   weights from `MODEL_BASE_URL` (bucket) when `MODEL_DIR` is empty — because
   they can never ride in git.
3. Stop pretending the image bundles models: drop the dead `COPY models` from the
   Dockerfile and document the bucket path.

---

## P1 — Single global `Mutex<AppState>` serializes the whole server

**Failure:** Under any real concurrency the web API throughput collapses; one slow
ONNX embed blocks every other request including `/health`, so Railway's
healthcheck times out and restarts the container mid-load → crash loop.

**Why:** `type SharedState = Arc<Mutex<AppState>>` (`web_server.rs:56`). Every
handler takes the *same* lock. `embed`, `classify`, `scan`, `chat` all hold it
across CPU-heavy work. Worse: handlers `.unwrap()` the lock (e.g. `:201`, `:230`),
so a single panic **poisons** the mutex and every subsequent request 500s
permanently until restart.

**Mitigation:** split state — `Arc<Embedder>` and `Arc<OntologyLoader>` are
read-only after startup and need no lock; only the mutable `goals`/`core`/`actor`
need a lock (ideally `RwLock` or per-field). Replace `.lock().unwrap()` with
poison-tolerant handling. Not urgent for a single-user demo, fatal at launch.

---

## P2 — Scope sprawl: 60 files / ~14.7k LOC, many half-built holons

**Failure:** Energy spreads across gantt (stub), distributed MCP mesh (stub),
graph-aware dreaming (linear only), industrial MQTT/Modbus (TCP ping only),
SigLIP/ImageBind/Gemma (unstarted) — and **nothing** reaches production quality.
The project dies of "90% done × 10 subsystems."

**Why:** two competing roadmaps (`DEVELOPMENT_PLAN.md` engine-research vs
`PLAN.md` industrial-SaaS) with no single thread of value. Solo maintainer.

**Mitigation:** pick **one** vertical (text+image classification API as a
dependency for Praxis/Agora — the thing already integrated and consumed) and
freeze the rest behind feature flags. Stop adding holons until the classify path
is provably accurate (see metrics, none are measured today).

---

## P3 — No accuracy measurement → can't tell working from broken

**Failure:** We ship "improvements" to ontology/centroids that silently regress
classification, because there is **no labeled test set** and no top-1 accuracy
gate. The `DEVELOPMENT_PLAN.md` targets (>0.85 text, >0.80 image) are aspirational
— nothing computes them. Combined with P0, we literally cannot distinguish a
loaded Jina model from random projection by looking at outputs.

**Mitigation:** a small curated `tests/classification_fixtures.json` (50–100
text→cell labels) + a test that asserts top-1 accuracy floor. This *also* catches
P0 in CI (random projection fails the floor).

**✅ Done (2026-06-04):** `tests/classification.rs` — always-on pipeline
invariants (deterministic RP, runs in CI: every cell scored, sorted, finite,
non-empty) + an `#[ignore]` accuracy floor on a real model. Classify path
extracted to `classify::CellClassifier` (lib) so it is testable and no longer
duplicated in the web bin. **First measurement exposed two problems:** Jina
`model_q4f16.onnx` won't load under the current `ort` (→ P4), and MiniLM scores
only top-1 domain 0.556 / cell 0.333 with a BOND bias — well under the 0.85
target. The number now exists and is gated; improving it is the next quality arc.

**Follow-up win (2026-06-04):** the low score was a **scoring bug**, not just a
weak model. Mean-centroid scoring let densely-populated cells (BOND) win via the
embedder's hub direction. Switching to **nearest-entry** scoring (top-2 member
cosines) lifted the same MiniLM from 0.556 → **0.852 domain / 0.741 cell** — at
the 0.85 target. Floor raised to 0.75. This also lifts both downstream consumers
(Praxis goal-tagging, Agora quality scoring) at once.

---

## P4 — ONNX runtime / model format drift breaks builds silently

**Failure:** `ort = "2.0.0-rc.12"` is a release candidate. A new rc or a
mismatched opset in the exported `.onnx` breaks inference at runtime (not compile
time), and the fallback chain masks it as "merely degraded" (see P0).

**Mitigation:** pin `ort` exactly (already `rc.12` — keep it pinned, no `^`),
add a startup self-test embed of a known string with a sanity-check on vector
norm/spread, fail loudly if the loaded model produces garbage.

**✅ Done (2026-06-04):** `ort` pinned `=2.0.0-rc.12`; a shared `load_onnx_session`
logs the real `ort` error instead of swallowing it; startup runs a semantic
self-test (dog≈puppy must beat dog≈tax-spreadsheet) and marks the embedder
`degraded` if a loaded model emits non-semantic vectors — surfaced on
`/api/v1/embedder` as `self_test`. **Root cause now visible:** Jina
`model_q4f16.onnx` fails to load under rc.12 with
`SimplifiedLayerNormFusion / InsertedPrecisionFreeCast ... GetIndexFromName` —
the 4-bit quant export trips this rc's graph optimizer. Fix path: export/obtain a
`model_fp16.onnx` or full `model.onnx` Jina, or upgrade `ort` once a stable line
loads the quant. Until then the engine correctly runs on MiniLM (self-test
green) at 0.556 domain accuracy.

**Resolved (2026-06-04):** root-caused and fixed. Jina fp16 *loads* once ort
graph optimization is disabled, but emits float16 tensors the f32 extractor reads
as random — caught by the self-test. The selection is now **self-test-gated**: a
model that loads but fails the semantic probe is skipped. Swapped in
**BGE-base-en-v1.5 (768-d)**, which lifts CORE top-1 to **0.974 domain / 0.923
cell** and the HARD tier to **1.000** — comfortably past target. The engine runs
on BGE; Jina is deferred (needs f16 handling or the fp32 export, only adds image
embedding).

---

## P5 — Legal/compliance on the consumer side (Agora)

**Failure:** The classify engine is fine, but the product that monetizes it
(Agora aggregator) ships a ToS-non-compliant model and gets API-revoked + C&D'd,
taking the whole monorepo's revenue story with it.

**Why / status:** already documented and pivoted — see
`agora/LEGAL_FINDINGS.md` and memory `agora-legal-constraint`. Tracked, not
re-litigated here.

**Mitigation:** keep physis_pro a **neutral classification utility** with no
platform-content dependency, so its fate is decoupled from Agora's legal risk.

---

## P6 — Secrets / unauthenticated public API

**Failure:** `physis-pro-web` binds `0.0.0.0:8080` with **no auth** in-process.
`/api/v1/chat` proxies to paid LLM providers via `ProviderCascade::from_env()`.
A public Railway URL = an open relay burning the API budget; `/api/v1/scan`
takes an arbitrary server-side `dir` (path traversal / info disclosure).

**Why:** auth is delegated to an `oauth2-proxy` sidecar that is documented
(`MANUAL_CONFIG.md`) but **not enforced** — if the operator forgets it, the API
is wide open. `.env` holds keys and is git-ignored (good) but there's no guard
that refuses to start the public bind without an auth front.

**Mitigation:** require a shared-secret bearer token (env `PHYSIS_API_TOKEN`) on
mutating/chat routes, default-deny when binding non-loopback without it; sandbox
`/scan` to an allow-listed root.

**✅ Done (2026-06-04):** constant-time Bearer middleware (`require_auth`) on all
routes except `/health` + `/api/v1/embedder`; fail-closed — the server exits if
`PHYSIS_HOST` is non-loopback and `PHYSIS_API_TOKEN` is unset; `/api/v1/scan`
canonicalizes the path and rejects anything outside `PHYSIS_SCAN_ROOT` (403).
Railway vars + `MANUAL_CONFIG.md` updated.

---

## P7 — Spanner sink: untested external write path + auth/cost/consistency

**Failure modes** (the sink is a scaffold, `--features spanner`, default off):
- **Untested REST flow.** session-create → commit-mutations is coded to the
  Spanner v1 REST shape but has **never run against a live instance**. The
  mutation `values` are stringified floats/ints (Spanner's wire convention) — a
  type/format mismatch surfaces only at first real commit.
- **Auth is a bare env token.** `GOOGLE_OAUTH_TOKEN` from `gcloud auth
  print-access-token` expires in ~1h. No refresh, no service-account JWT minting.
  A long-running sync dies after an hour with a 401. Real deploys need ADC /
  signed-JWT token minting.
- **Cost / runaway writes.** A full `insertOrUpdate` of every node on each sync,
  with no batching cap or backoff. A large wiki = a large single commit (Spanner
  has a ~80k mutation / commit-size limit) → `InvalidArgument`. No chunking yet.
- **No deletes / no incremental.** Today it only upserts. Nodes removed locally
  (or after a future eviction) linger in Spanner — divergence over time.
- **Embeddings not shipped.** Intentional (large, re-derivable), but it means
  Spanner can't serve ANN; a later "vector column" decision is deferred, not free.
- **Id bridge missing for edges.** Holarchy edges key `RawNodeKey`s, not
  coherence ids — there's no stable mapping, so the graph half of "Spanner
  **Graph**" isn't actually exported yet (nodes only).

**Mitigation before this leaves scaffold:** (1) integration-test against a real
Spanner emulator (`gcloud emulators spanner`) in CI; (2) replace the env token
with service-account JWT mint + refresh; (3) chunk mutations (≤ ~1k rows/commit)
with retry/backoff; (4) decide deletes (tombstone vs full-resync); (5) only then
wire a server background sync — never on the hot path.

**Status:** ⬜ scaffold only. Do NOT enable in prod until (1)–(3) land.

---

## Decision: proceed with **P0** now

P0 is highest damage × highest likelihood, blocks honest evaluation of everything
else (P3 depends on it), and is fixable in-repo without external infra. P1/P6 are
launch-blockers queued next. P2 is a discipline change reflected in the plan.
