---
id: PH-024
track: physis-pro
phase: 0
status: review
rung: 2
depends: []
gate: "cargo clippy --all-targets -- -D warnings && cargo test --features tui --bin physis-hud && cargo run --features tui --bin physis-hud -- . --setup && cargo run --features tui --bin physis-hud -- . --snapshot=100x30 --view settings"
commit: "branch agent/PH-024; conventional commits; per PROTOCOL §6"
claimed_by: cline-pansa-2026-09-20
claimed_at: 2026-09-20
---

# physis-hud is the one front door: every surface a view, settings editable, setup a screen

## Why (the failure this prevents)

The impossible-computer programme locked `physis-hud` as *the* TUI
(`docs/plans/2026-09-19-impossible-computer.md` §10.1: "TUI target is
`physis-hud`. Studio remains the demo product."). Measured on this checkout,
the HUD is not yet that surface:

- `graph_view()` (`src/bin/hud.rs:2941`), `show_notebook()` (:2534),
  `HudCommand::Tasks` (:2197), `:agents` (:2238) and `:settings` (:2256) **all
  write into `self.reading`** — the 35 %-height reading pane — so "the task
  graph", "the settings" and "the notebook answer" are indistinguishable from
  reading a file, and the next `Enter`/query destroys them.
- Settings are **read-only** in the HUD (`Settings::load().render()`), while the
  same values are editable in `physis-pro-tui` and Studio. An operator inside
  the HUD must leave it, remember the dotted key, and use the CLI.
- The five config knobs are **defined three times** (`src/bin/tui.rs:249` and
  `:266`, `src/bin/studio.rs:210` and `:227`, and would be a third copy in the
  HUD) — the exact drift the comment above `edit_field` warns about.
- Setup is prose. A new machine gets no answer to "what is missing, and what
  do I run next?" inside the surface the operator is told to live in.

## Context (measured 2026-09-20, branch agent/PH-024 from 3b29780)

- `src/bin/hud.rs` 4795 lines; keys are handled in one `match` (:3842–:3905),
  commands in `run_hud_command` (:2191), rendering in `draw` (:3359).
- `src/settings.rs` already ships `Settings::{load, user_path, save, get, set,
  render}` — the editor needs no new storage.
- `src/config.rs` `PhysisConfig` + `load_effective()`/`save`-able JSON at
  `data_dir/config.json` (Studio's `save_config`, `src/bin/studio.rs:202`).
- `--snapshot[=WxH]` already renders through the real `draw` (:131) — the
  artifact a CI claim can be made against.
- No licence gate in `hud.rs`, so the gate needs no `PHYSIS_ALLOW_DEV_LICENCE`.

## Steps

1. Add `View` (World · Graph · Notebook · Tasks · Agents · Settings · Setup ·
   Help) with `ALL`, `label`, `from_name`, `from_key`. Views are projections,
   never new stores.
2. Route the existing surfaces into views: `g`/`graph`, `n`/`notebook`,
   `tasks`, `agents`, `settings`, `help`, `:setup`; `Esc` returns to the world;
   `Tab`/`BackTab` cycle. A view renders as one full-width body in place of the
   reading split, titled with the view label and the keys that work there.
3. Make SETTINGS editable inside the HUD: rows over `Settings` providers and
   commands plus the five config knobs; `+`/`-` steps toggles/choices/knobs,
   `Enter`/`e` edits a text value through the existing command box (a real
   prompt mode, not a second editor), `s` saves both files, `r` reloads,
   `d` resets the selected row to its shipped default. Values that are secrets
   stay names (`key_env`), never values.
4. Add SETUP: a pure `setup_report(facts)` naming what was detected (root,
   index, embedder, semantic mode, config/settings paths, agent transport) and
   the exact next command for each gap; `physis-hud --setup` prints it before
   the alternate screen and exits 0.
5. Move the five knobs into the library as one table (`config::CONFIG_KNOBS`)
   and point `tui.rs` and `studio.rs` at it, so the three panels cannot
   disagree.
6. Add `--view <name>` so `--snapshot --view settings` is a CI artifact.

## Tests you must add

- **Unit:** `settings_rows` lists every provider, every command and every knob
  in a stable order; `setting_value` renders a secret as the env **name**;
  stepping a toggle/choice/knob changes exactly that row; `apply_text` rejects
  an unknown dotted key and accepts a real one; `View::ALL` labels are unique and
  `from_name`/`from_key` round-trip; `setup_report` names the offending piece and
  its next command when semantic is off, when config.json is absent and when no
  agent is reachable; the settings body marks the selected row.
- **Integration:** `config::knobs` test in the library proving each knob's
  step moves its own field and clamps at the floor.
- A bug fix without a regression test is not done.

## Gate

```bash
cargo clippy --all-targets -- -D warnings
cargo test --features tui --bin physis-hud
cargo run --features tui --bin physis-hud -- . --setup
cargo run --features tui --bin physis-hud -- . --snapshot=100x30 --view settings
```

## Commit strategy

One commit per step group (views / settings editor / setup / knob table), each
referencing PH-024. `git add` named files only.

## Report

Append packets/reports/PH-024.md per PROTOCOL §5.

## Never

- No second editor, no second storage, no LLM in the settings path.
- No behaviour removed from the world keys; `--snapshot` keeps rendering the
  same `draw`.
- Do not print a secret value, ever; only the env var name.
- Do not add a sixth copy of the knob list.
