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
| Object/action registry: `physis-pro action list\|show\|call` | shipped | S0 tests |
| Agent roster, lifecycle, permissions | shipped | S1 tests |
| Routing ladder: `physis-pro agent route` | shipped | S3 tests |

Not in tonight's ship: S4 task graph, S7–S12, agent pane invocation from the
HUD (the card shows transport and says no call is made).

---

## 2. Steps shipped

| Step | Commit | What it delivered |
|---|---|---|
| S0 | `a70eb3b` | `config/actions.json` + loader; registry wired to nav objects; `action` CLI; HUD `action` line; approval gate for irreversible capabilities |
| S1 | `e2de4a0` | `src/agents/{mod,view}.rs`, `config/agents.json`; lifecycle legality; deny-by-default permissions; `@agent[@host]` addressing; AGENTS strip; HUD `--snapshot`; `ai/orchestration` stub deleted |
| S2 | `d754b1a` | `src/agents/adapter.rs`: `AgentAdapter`, `JevResult` schema, `LocalAdapter`, `CliAdapter`, fixture-process tests |
| S3 | `d754b1a` | `src/agents/routing.rs`: cost ladder, policy enforcement, explicit override, no silent substitution |
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
  `src/agents/`. The adapter deadline is 240 s.

The adapter appends a reply contract to every JEV prompt and requires a parseable
`JevResult`. A reply whose JSON object carries none of `findings`/`relations`/
`notes`/`decisions` is reported schema-invalid — before this, such an object
deserialized to an empty result and read as success.

---

## 6. Known limits and defects

- **`agent ask` is unmeasured end-to-end.** It packs workspace context and then
  calls the model; the pack step embeds the corpus on first run and is
  unbounded, so the command can exceed a 400 s shell timeout. `agent call jev` is
  the verified path.
- **The HUD agent card does not invoke.** `@jev` shows the card and states that
  no call is made; invocation is `agent call`/`agent ask` on the CLI. A 43 s–3 min
  call would freeze the synchronous HUD loop, so wiring it needs a background
  thread (S6 work).
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
