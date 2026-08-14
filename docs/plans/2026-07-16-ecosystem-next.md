# Ecosystem Next Steps — mirror rescue + Phase-8 Task 5 + Praxis→physis + Agora pivot

> Created 2026-07-16. Covers: (A) old-disk rescue, (B) Task 5 revival,
> (C) Praxis→physis wiring, (D) Agora ToS pivot, (E) cleanup of mis-wired stubs.

## A. Old-disk rescue (DONE 2026-07-16)
- OLD (degrading) disk: `/media/gio/7d36e554-5f5a-4fe2-9fcc-8b3c536141681`
- NEW (main) disk: `/media/gio/c459b156-2cc1-4b0b-a695-3db1923219bf2`
- **Gap found + fixed:** `philosophical_machines/` was MISSING on NEW (0 .py);
  rescued from OLD → 54 .py files including `physis-machine-template/physis_machine/machine.py`
  (the canonical `Machine` base Task 5 needs) + all 6 machines' `machine/` source.
- `physis_models/` (417M) rescued OLD→NEW.
- All other OLD top-level dirs present on NEW (OK-same). `System`/`Volume`/`Information`/
  `__ssd_unique_holding__` on OLD are Windows volume metadata — skipped (not content).
- `backup`/`mamma`/`nonna` appeared in a race-y `ls` but do NOT exist on OLD — no loss.

## B. Phase-8 Task 5 — REVIVED (was SUPERSEDED, now unblocked)
Source exists on NEW at `philosophical_machines/physis-machine-template/physis_machine/machine.py`.
The `Machine` base already does the Phase-8 pattern by construction: `reduce()` yields
primitives, `remember()` → `physis.register()` + `physis.recall()` via `PhysisClient` (HTTP to
physis-pro). So Python is **already pure perception**; the engine elaborates. Task 5 = verify
the rescued machines' `reduce()` yields primitives and register/recall go through the client,
then close DONE (no fake thinning needed — the architecture already matches).
- TDD: import `physis_machine.Machine`; a sample subclass with `reduce()` yields primitives;
  `remember()` calls client.register/recall (mock client). Mark plan Task 5 DONE.

## C. Praxis → physis wiring
Wire `praxis_webapp` to the `physis-pro` engine for meaning-based goal/journal search.
- Use existing `physisProClient.ts` (`X-Physis-User` header, `/coherence/*` endpoints) if present;
  else add a thin client to the physis-pro HTTP API.
- Add a search service that embeds a goal/journal text, queries physis recall, returns echoes.
- TDD: a goal text → physis recall returns related forms (mock or live dev engine).

## D. Agora ToS pivot
Current Agora aggregates third-party content + ads → non-compliant. Pivot:
- Own-content only + subscription before further feature build.
- Audit `agora`/`agora_mobile`/`agora_android` for third-party aggregation + ad SDKs;
  list removals. Add a subscription gate (stub or real paywall) behind own-content.
- Decision task + refactor; no new features until pivot lands.

## E. Cleanup mis-wired stubs (NEW disk)
`/media/gio/.../solon/noe-philosophy/{euclid,logos,nous,empedocles,pythagoras,physis-kairos,
physis-machine-template}` are all mis-wired to `agora.git` with EMPTY `machine/` stubs.
The REAL source now lives at `philosophical_machines/` (rescued). Action: remove the
` solon/noe-philosophy/*` stub dirs (or re-point to real remotes) so future searches don't
re-chase them. Keep `philosophical_machines/`.

## Status (FINAL, 2026-07-16)
- **A. Rescue — DONE.** `philosophical_machines/` (54 .py, incl. `physis_machine/machine.py`
  base + 6 machines) was MISSING on NEW disk; rescued from OLD (degrading) disk.
  `physis_models/` (417M) rescued. All other OLD dirs present on NEW. `System`/`Volume`/
  `Information` leftovers are Windows volume metadata (skipped).
- **B. Task 5 — DONE (revived).** Real `Machine` base imported + verified: `reduce()`
  yields primitives (pure perception), `run_once()` fail-soft via `PhysisClient`.
  Phase-8 all 6 tasks complete; `cargo test` 412 pass.
- **C. Praxis→physis — DONE.** Added `recallCoherence()` to `physisProClient.ts` +
  `MeaningSearchService` (maps `goal:`/`journal:`/`note:` recall hits to typed refs).
  2 jest tests pass; tsc clean. Pushed `c5cff8e` to praxis mirror.
- **D. Agora ToS pivot — DOCUMENTED.** Agora already has Stripe + premium infra.
  Pivot = disable third-party aggregation (`youtube_worker.ts`) + own-content gate.
  Decision recorded in `agora/AGORA_TOS_PIVOT.md`; pushed `e0bb9a95` to agora.git.
- **E. Cleanup — DONE.** Removed 7 mis-wired empty stubs under `solon/noe-philosophy/`
  (all were agora.git-wired with 0 real files). Real `philosophical_machines/` intact.

## Verification gate
- A: counts match ✅. B: import+subclass test ✅. C: jest 2 pass + tsc ✅.
- D: pivot doc + audit pointer ✅. E: stubs removed, real machines intact ✅.

## Commits
- physis_pro: `168a6e2` (plan) → `2408265` (ecosystem plan) → `7d894bc` (Task5 DONE)
  + this status update. Pushed origin (physis-pro) + drive mirror.
- praxis_webapp: `c5cff8e` meaning-search. Pushed disk mirror.
- agora: `e0bb9a95` ToS pivot doc. Pushed agora.git.
