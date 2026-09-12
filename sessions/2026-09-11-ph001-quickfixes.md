# 2026-09-11 — PH-001 quickfixes (resumption packet)

Pre-registration written BEFORE querying Physis (directive Part 3, Step 1).

## Task
Execute packet `packets/PH-001-quickfixes.md` (three one-line drift fixes:
version, env typo, default features) and, as a preliminary reviewer pass,
re-run the gates for the two packets sitting in `status: review` (PH-000,
PH-016) per PROTOCOL §7. Also integrate the full packet board (PH-000..PH-100)
into `PLAN.md` so the plan and the packet skeleton are one resume point.

## What I intend to do (falsifiable)
1. PH-000/PH-016 reviewer pass: re-run `cargo clippy --all-targets -- -D
   warnings` + tree check; flip both to `status: done` if green.
2. Version fix: packet says `/health` hardcodes `0.1.17`. My prediction from
   re-measurement: the code ALREADY reads `env!("CARGO_PKG_VERSION")`
   (src/web/mod.rs:957), so the remaining work is only the *test* (assert the
   JSON body version equals `env!("CARGO_PKG_VERSION")` in
   tests/http_surface.rs), not the handler.
3. Env typo: `.env:28` has `PHICS_LOG_LEVEL=info`. My prediction: fixing the
   spelling alone is NOT enough — grep says no code reads
   `PHYSIS_LOG_LEVEL` at all; the web server actually reads `PHYSIS_LOG`
   (EnvFilter syntax, src/web/mod.rs:~3483). So I intend to point `.env` AND
   `docs/DEPLOYMENT_OMO_PANSA.md` at `PHYSIS_LOG` (the variable the code
   actually reads), not at `PHYSIS_LOG_LEVEL` as the packet literally
   instructs. This deviates from the packet's letter to satisfy its stated
   intent ("log level configurable through the documented variable").
4. Features report: packet premise says default excludes `mcp` and
   `embed-onnx`. My prediction: `embed-onnx` is already in `default`
   (Cargo.toml:133); only `mcp` is excluded. Report will measure the cost of
   adding `mcp` (check/build time, debug binary size delta) and recommend;
   the decision stays with the human (rung 5).
5. Gate: tier-0 (check + clippy) after every change; full `cargo test` once
   at the end. Known pre-existing flake: content_search test (PH-015).

Confidence 0.7 that all four predictions survive contact with the code —
the repo moved a lot since the packet was written (2026-09-03).

## Query + binary
Ran 2026-09-11 ~12:05 (after minting a fresh local licence — see blocker below):

    ./target/debug/physis-pro node-search "PH-001 version env log level
    default features quickfixes"

    → licensed OK, embedder dim=768,
      "restored 0 nodes (13 skipped: embedder mismatch)",
      0 result(s), 0 tokens.

**Retrieval was empty.** The persisted outcome-memory nodes were embedded by a
different embedder than the active one, so nothing was retrievable. Per
directive Part 6 this is recorded plainly rather than papered over: on this
date the loop gave no signal either way.

**plan_changed: false** — with the explicit caveat that no retrieved claim
*could* change the plan, because retrieval returned nothing. The plan WAS
refined, but by re-measuring the code (packets' "re-measure NOW" rule), not by
retrieved claims: (a) version fix already in code → test-only; (b) log variable
is `PHYSIS_LOG`, not `PHYSIS_LOG_LEVEL` → docs+`.env` point at the real
variable; (c) `embed-onnx` already in default → report covers `mcp` only.

## Blocker found on the way (not this packet's fix, recorded per PROTOCOL §5)
- Licence gate: the CLI resolves the licence from **CWD-relative**
  `./.physis/license.key` (src/license.rs `default_license_path`), so a stale
  repo-local licence shadows the home one and every command dies with
  "signature is not valid for this build's vendor key". The Sep-7 licence was
  signed by a signing key that has since been rotated; re-minted with the
  current `~/.physis-vendor/signing.key` (backup of the old file kept at
  `license.key.bak-20260911` in both locations). This CWD-relative resolution
  is the same hazard class as the embedder's CWD-relative model paths
  (packets/README red line) — candidate packet.
- `just recall` cannot run as written until the stale licence is refreshed
  (it shells `cargo run`, which is also what blocked rebuilding it silently).

## Outcome
(filled at the end)

