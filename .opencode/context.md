# Project Context

> Current programme (2026-09-20): `docs/plans/2026-09-20-physis-hud.md` — the HUD
> programme (one world, many projections), which composes
> `docs/plans/2026-09-19-impossible-computer.md`. Two steps shipped on branches,
> awaiting review: PH-024 (`physis-hud` views / editable settings / `--setup`) and
> PH-025 (guided execution Phase A, `src/guided/`, `physis-pro guided`).
> The status notes below are historical (2026-09-11) and may be stale.

## Current Status
- Repository: `/home/gio/dev/physis-pro`; branch `agent/PH-001`.
- User asks to continue estimate-command work; do not commit or push.
- `src/main.rs` currently has inline estimate helpers/`run_estimate`, `Commands::Estimate`, early dispatch before licence gate, and estimate tests. `src/estimate.rs` is absent and there is no `mod estimate;`.
- Current `git status`: dirty `.mcp.json`, `.opencode/context.md`, `.opencode/status.md`, `.opencode/todo.md`, `.opencode/work-log.md`, `Cargo.lock`, `PLAN.md`, `physis-core`, `physis.sh`, `src/main.rs`, `src/cli.rs`, `src/lib.rs`, `src/web/mod.rs`, plus untracked `.mcp/`, `assets/brain/`, `sessions/`, `src/bin/ui/brain.html`, `src/brain.rs`, `src/web/brain.rs`, `tette`.
- Existing public APIs: `physis_pro::pack::{resolve_root, pack_cached, EmbedCache}`, `physis_pro::scanner::scan_project`, `physis_pro::indexer::FileIndex`, `physis_pro::rag::count_tokens`, `physis_pro::embed::{RandomProjectionEmbedder, VectorEmbed}`, `physis_pro::config::PhysisProConfig`.
- No reachable estimate history was found in git refs or unreachable objects.
- `.opencode/sync-issues.md` is absent.
- Loom plan unavailable: `ModuleNotFoundError: No module named 'jieba'`; direct inspection used.

## Pending Tasks
- Create `src/estimate.rs` containing the inline estimate logic, public `physis_pro` API usage, and focused tests.
- Add `mod estimate;` to `src/main.rs`; remove inline estimate implementation/tests; retain `Commands::Estimate`, early return before licence gate, and harmless unreachable arm.
- Run `cargo check --features mcp` and `cargo test --bin physis-pro estimate`; fix only estimate/main integration issues.
- Preserve all unrelated dirty paths. No commit or push.
- Active delegated tasks: `task_8c70ad8c`, `task_527c12df`, `task_96e4b354`, `task_8b15a0e3`, `task_378354d2`, `task_0bd4317c`, `task_e074603a`, `task_b1a92919`, `task_18c904fe`, `task_8df68041`.
