# opencode-jev-recorder

Two pieces that turn JEV (the `jev-review` MCP) into a repeatable loop for
developing Physis, and record what it found.

## 1. `plugin.js` — record every evaluation

Hooks `tool.execute.after`, parses the JEV `Evaluation`, and writes a compact
observation (scores, weakest metrics, priorities, comparison deltas) through
`physis system remember`. Query it back:

```bash
physis system --root <workspace> history jev_review
```

### Install

The effective global config on this machine is `/home/gio/.opencode/opencode.json`
(the copy under `~/.config/opencode/opencode.json` is not read for plugins here),
and plugin paths resolve relative to the config file's directory. So:

```bash
mkdir -p ~/.opencode/plugins
ln -sfn /home/gio/dev/physis-pro/clients/opencode-jev-recorder \
          ~/.opencode/plugins/jev-recorder
# add "./plugins/jev-recorder/plugin.js" to the `plugin` array of
# /home/gio/.opencode/opencode.json
```

For a single project, the absolute path works instead — physis-pro uses it in
`.opencode/opencode.json`:

```json
"plugin": ["/home/gio/dev/physis-pro/clients/opencode-jev-recorder/plugin.js"]
```

A symlinked directory needs `package.json` with `"main": "plugin.js"`; that file
is present.

Smoke test (drives the hook without OpenCode, then asks the log):

```bash
node clients/opencode-jev-recorder/smoke.mjs
```

## 2. `~/dev/jev-review/scripts/review.mjs` — run one review, no agent turn

Drives the same bundled MCP server directly, so a review is one deterministic
step a script or a loop can own. Saves the full Evaluation JSON and prints
comparison deltas when given `--previous`.

```bash
node ~/dev/jev-review/scripts/review.mjs \
  --path ~/dev/physis-pro/src/decisions.rs \
  --store ~/dev/physis-pro/.physis/jev \
  --record
```

## The loop

```text
review.mjs --path F --store .physis/jev     # baseline, saves .physis/jev/F.json
#   diagnose the weakest metrics, change the code
cargo check -p physis_pro --lib && cargo test -p physis_pro --lib <module>
review.mjs --path F --previous .physis/jev/F.json --record   # deltas
```

`.physis/jev/<file>.json` is the accepted state; `--previous` turns it into the
comparison baseline. Each cycle appends to `physis system history jev_review`,
so score movement is queryable without re-running JEV.

## Distillation rule

A score is a signal, not a target. When JEV reliably flags the same *class* of
finding (not the same number), compile it into a deterministic check — a unit
test, a clippy lint, a gate — so the next occurrence is caught without a model.
Worked example: `decisions.rs::mismatched_width_prior_does_not_falsely_dedup`.

Do not gate on raw scores: the free model's run-to-run variance is ~±0.3, so a
score ratchet flakes. Gate on findings and on `cargo`/`clippy`/tests.

## Boundaries

The plugin never blocks or modifies a tool call, never reads agent reasoning,
and never converts a score into a pass/fail outcome. The recorded outcome stays
`unverified` — JEV scored the change, it did not accept or reject it.
