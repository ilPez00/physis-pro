# Progress and ship record — 2026-09-19

Dated verification record for the impossible-computer programme. Per the
documentation contract in `2026-09-19-impossible-computer.md` §12, this answers
"what actually runs here?" with commands that reproduce the claims. It is not a
plan and not a capability list: if a command below does not produce the stated
output on this checkout, the claim is wrong.

Branch: `agent/MS-005-q6`. Box: pansa. Dev licence: `PHYSIS_ALLOW_DEV_LICENCE=1`.

---

## 1. What ships tonight

| Surface | State | Verified by |
|---|---|---|
| `physis-hud` TUI with an AGENTS strip and `@agent` cards | shipped | `--snapshot` (below) |
| OpenCode integration: `physis-pro agent call jev "<prompt>"` | works, ~43 s–3 min | live call, below |
| Editable settings: `physis-pro settings show\|path\|set` | shipped | settings 6/6 |
| Saved packs: `pack --save` then `agent ask --pack` | shipped | pack 14/14 |
| Object/action registry: `physis-pro action list\|show\|call` | shipped | S0 tests |
| Agent roster, lifecycle, permissions | shipped | S1 tests |
| Routing ladder: `physis-pro agent route` | shipped | S3 tests |

### Settings are editable

Two layers, user wins: `config/settings.json` (shipped) and
`~/.physis/settings.json` (or `PHYSIS_SETTINGS`). Both command syntax and
provider configuration are editable without a rebuild:

```sh
physis-pro settings show
physis-pro settings set providers.jev.model oczen/mimo-v2.5-free
physis-pro settings set commands.ask.aliases q,question
physis-pro settings set providers.jev.key_env MY_JEV_KEY
```

A key is stored as the **name of an environment variable**, never the secret, so
a settings file can be shared or committed. Model and bin resolve as
env override > settings > roster > built-in default.

### Pre-pack and save for later

The measured pack cost is why this exists: a cold cache over the whole tree ran
10+ minutes. A pack can be built once and reused.

```sh
physis-pro pack . "routing key" --max-files 40 --budget 900 --save /tmp/pack.json
physis-pro agent ask "What does the routing ladder do?" --pack /tmp/pack.json
# [physis] loaded saved pack /tmp/pack.json (887 tokens, 4 chunks)  — no embedding
```

`pack --max-files N` bounds the walk (`0` = unbounded). Note the bound is on the
walk, so a small N may miss relevant files; use a saved pack or `0` for full
quality.

Not in tonight's ship: S7–S12, and the HUD task-graph overlay (the CLI
`agent tasks` folds and renders the graph; the TUI does not yet draw it).

## 2.1 The full gate

`cargo test --all-targets` is green (45 suites). Two failures surfaced the first
time it was run and both were stale snapshots of a deliberate change, not
regressions:

- `core_public_api_matches_snapshot` — the snapshot predated the physis-core
  fast-forward to the austere-core split, listing 503 items from retired modules
  and missing 22 additions. Re-blessed (`BLESS=1`), 22+/503-.
- `http_surface::route_table_crud_match_and_validation` — asserted a bare domain
  was an invalid cell, which the routing-key rework made valid. The invalid case
  is now an empty cell, and the assertion still demands all three problems.

---

## 2. Steps shipped

| Step | Commit | What it delivered |
|---|---|---|
| S0 | `a70eb3b` | `config/actions.json` + loader; registry wired to nav objects; `action` CLI; HUD `action` line; approval gate for irreversible capabilities |
| S1 | `e2de4a0` | `src/agents/{mod,view}.rs`, `config/agents.json`; lifecycle legality; deny-by-default permissions; `@agent[@host]` addressing; AGENTS strip; HUD `--snapshot`; `ai/orchestration` stub deleted |
| S2 | `d754b1a`→`0ce16ae` | `src/agents/adapter.rs`: `AgentAdapter`, `JevResult` schema, `LocalAdapter`, `CliAdapter`, fixture-process tests |
| S3 | `d754b1a`→`0ce16ae` | `src/agents/routing.rs`: cost ladder, policy enforcement, explicit override, no silent substitution |
| S4 | `022ac8a` | `src/agents/tasks.rs`: task graph folded from observations; legality table; acceptance gate; expire creates a child with parent + reason |
| rework | `a1be100` | route key is the region, domain-first with optional mode (see §4) |

Submodule `physis-core` at `c3ddb8e` (product modules retired, boundary docs,
`cargo fmt`). Superproject `Cargo.lock` and pointer updated in `4d93314`.

---

## 3. Reproducible verification

```sh
# Formatting and lint (both must be silent/exit 0)
cargo fmt --check
cargo clippy --all-targets -- -D warnings

# Focused suites
cargo test --lib agents::          # 27 passed
cargo test --lib routes::          # 7 passed
cargo test --test action_surface   # 5 passed
cargo test --test registry_conformance  # 1 passed
cargo test --bin physis-hud        # 25 passed

# The TUI renders headlessly through the real draw() path
PHYSIS_ALLOW_DEV_LICENCE=1 ./target/debug/physis-hud --snapshot 2>/dev/null | grep AGENTS:
# AGENTS: JEV ●  Hermes ●  OpenCode ●  Verifier ●  Local ● · ● ready ○ unavailable

# The roster and the ladder
PHYSIS_ALLOW_DEV_LICENCE=1 ./target/debug/physis-pro agent list
PHYSIS_ALLOW_DEV_LICENCE=1 ./target/debug/physis-pro agent route read file
# route: @local via local (local) — ladder: cheapest permitted agent (cost local)

# The live OpenCode integration (needs network; ~43 s for a bare turn)
PHYSIS_ALLOW_DEV_LICENCE=1 ./target/debug/physis-pro agent call jev "Reply with exactly PONG."
```

The `--snapshot` flag was added in S1; before that the HUD had no headless mode
(the plan's §11 open unknown is now closed: Studio had one, the HUD did not).

---

## 4. The routing-key rework (why the cell is not the key)

`docs/WHAT_PHYSIS_IS.md` §2, E66 (same 731 entries, same embeddings,
size-preserving permutation null), normalised self-recovery:

| axis | classes | normalised |
|---|---|---|
| DOMAIN/MODE | 70 | 0.416 |
| MODE only | 14 | 0.472 |
| DOMAIN only | 5 | 0.585 |

The first S3 draft keyed routes on `DOMAIN/MODE` — the weakest axis — and a mode
that was unknown or misclassified could suppress a route entirely. Now a route
binds a region `DOMAIN` or `DOMAIN/MODE`; `for_domain` is the broad lookup and
`for_cell` the narrow one; the router tries cell first, then falls back to
domain. The domain axis is open (`PHYSIS_DOMAINS`), so no domain name is
hardcoded. Existing `DOMAIN/MODE` `routes.json` still loads and matches.

---

## 5. The OpenCode integration, measured

Measured 2026-09-19 on pansa, `opencode` 1.18.30.

- `opencode run` **defaults to `deepseek-v4-flash`**, which fails immediately
  with `Insufficient account funds`. The adapter therefore always passes `-m`.
- Live free-tier list (`curl -s https://opencode.ai/zen/v1/models`) contains
  `deepseek-v4-flash-free`, `jev-1.13-free`, `ling-3.0-flash-fin-free`,
  `mimo-v2.5-free`, `muse-spark-*-contributor-free`, `nemotron-3.5-lightning-free`,
  `nemotron-3-ultra-free`.
- Tested: `oczen/nemotron-3.5-lightning-free` **answers** (`PONG`);
  `mimo-v2.5-free` and `deepseek-v4-flash-free` returned nothing;
  `jev-1.13-free` errors server-side (`UnknownError`).
- Pinned default: `oczen/nemotron-3.5-lightning-free`, overridable with
  `PHYSIS_JEV_ZEN_MODEL`.
- Latency: a bare turn is **~43 s**. A question that makes the read-only `plan`
  agent explore the repo ran **3 m 10 s** and produced a correct answer about
  `src/agents/`. The adapter deadline is 240 s for `agent call`, 600 s for
  `agent ask`.
- Attaching to a warm `opencode serve` (`opencode run --attach`) was measured
  **slower** — 56 s against 43 s for the same prompt — so the adapter uses a
  plain `run`. The plan's suggested mitigation did not pay off here.

### `agent ask`, measured

`agent ask` packs workspace context and then asks JEV. Two measurements:

- **The pack step was the cliff.** Over the whole tree on a cold embedding cache
  it ran **10+ minutes at ~680 % CPU without finishing** and had to be killed; it
  saturates the box. `pack_cached_limited` now takes a file bound, and `agent
  ask --max-files` defaults to 150, which packs in seconds (5 chunks, ~1.2k
  tokens in the measured run). `--max-files 0` restores the unbounded walk.
- **The model call still dominates.** With the pack bounded the call exceeded
  the then-240 s deadline on a real question, so `ask` now allows 600 s. The
  free Zen tier is a batch/background transport, not an interactive one — which
  is exactly why the HUD asks on a worker thread.

The adapter appends a reply contract to every JEV prompt and requires a parseable
`JevResult`. A reply whose JSON object carries none of `findings`/`relations`/
`notes`/`decisions` is reported schema-invalid — before this, such an object
deserialized to an empty result and read as success.

---

## 6. Known limits and defects

- **`agent ask` is measured, and it is slow by nature.** The pack step is now
  bounded (`--max-files`, default 150), but the model call has a ~43 s floor and
  a real question ran 3 m 10 s, so the deadline is 600 s. Treat it as a
  background tool. `agent call jev` is the smaller, verified path.
- **The HUD asks on a worker thread.** `@agent <question>` spawns a thread and
  the reply arrives through an mpsc channel drained before each draw, so a
  3-minute turn never freezes the loop. Snapshot mode and tests have no channel
  and the card says so.
- **Commit `d754b1a` message is mangled**: shell backticks ate the CLI list, so
  it reads `Callers:  (the plan's rule…)`. The commit is pushed; fixing the
  message needs a force-push and was not done unasked.
- **JEV's direct transport does not exist in this build.** A `JEV_API_KEY` alone
  is not reported as available; the reachable path is OpenCode Zen, and the card
  says `zen:<model>` rather than claiming JEV-the-service.
- **Test-flake fix worth keeping:** fixture tests that wrote a script and exec'd
  it flaked with `ETXTBSY` under parallel `cargo test` (a sibling's forked child
  inherits the write fd). They now run `sh -c <script>` instead; 15/15 parallel
  runs pass.

---

## 7. Next

S4 (task graph over `ProcessTask`/`TaskState`, graph as a fold over
observations) is the next step in the plan. Before it, the two shipping gaps
above are the cheap wins: bound `agent ask`'s pack step, and move HUD invocation
onto a background thread.
