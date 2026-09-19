# Developing physis with physis (dogfood loop)

The engine's own dev loop runs on the engine: outcome memory (`note` /
`node-search`, twins of the MCP `physis_remember` / `physis_recall` tools)
plus the standard gates. No server needed — everything hits the shared sled
store (`PHYSIS_DATA_DIR`, default `.physis/`).

## The loop

```
just recall "<task>"          # 1. what did we already try? (verdicts ride along)
... work ...                  # 2. pack/find/classify as needed
just dev-loop "<task>"        # 3. gate + verdict-stamped note (-1 RED / +1 GREEN)
```

`dev-loop` defaults to `cargo check --all-targets`; pass any gate after `--`:

```bash
./scripts/physis-dev-loop.sh "fix clippy" -- cargo clippy --all-targets -- -D warnings
```

Failure never fails silent: the script prints the gate tail (expected vs
actual) and exits with the gate's code. Only **notes** are written —
never `fail`/`pass`: the quality tracker's penalties/boosts are about
classification output, and dev-gate noise must not pollute that signal.

## Agent attachment (MCP)

`.mcp.json` at the repo root spawns `physis-pro serve --stdio` (feature
`mcp`) with `PHYSIS_SCAN_ROOT=.`, so any MCP client gets all 30 tools
against this repo. Agent session protocol:

1. **Start**: `physis_recall` the task — a top hit with `verdict: -1` means
   "already tried, already failed", read it first.
2. **Orient**: `physis_pack` (bounded context) / `physis_find` (answer inside
   a file) / `physis_workflow` (who did what where, docs coverage).
3. **During**: `physis_classify` ambiguous changes; `physis_route` when the
   change should trigger a harness action.
4. **End**: `physis_remember` the outcome with a verdict; `physis_serve` to
   link artifacts to goals; `physis_themes` for "what do we keep failing at".
5. **Ontology gaps**: `physis_discover` over dev logs drafts new domains
   (`EXTRA_ONTOLOGIES`) — how the grid grows from its own work.

CLI equivalents for scripts/harness: `note` (= remember), `node-search`
(= recall), `pack`, `find`, `workflow`, `where` (= ledger facets),
`fail`/`pass`/`quality` (classification feedback only).

---

## The loop actually used on 2026-09-19 (and the traps it hit)

Recorded because most of the time lost that session went to traps, not to work.

### The two MCP servers

| server | launched by | store | what it gives |
|---|---|---|---|
| `physis` | `.mcp.json` → `./target/debug/physis-pro system serve`, env `PHYSIS_SCAN_ROOT=.`, `PHYSIS_MCP_AUDIT=1` | `.physis/system` | 30 tools: `physis_recall`, `physis_pack`, `physis_find`, `physis_classify`, `physis_route`, `physis_remember`, `physis_serve`, `physis_usage`, `physis_history`, `physis_tasks` |
| `jev-review` | the harness's own config → `/home/gio/dev/jev-review/dist/server.js`, `JEV_MODEL=jev-1.13-free` | — | one tool: a scalar review loop returning per-metric scores + confidence. Not a prose reviewer: it scores, you diagnose. |

**The root trap.** The `physis` server is scoped by the directory it was
launched in. In that session it pointed at `/home/gio/dev/physis-pro` while the
work was in `/home/gio/physis_pro` — a *different clone*. Every MCP result would
have described the wrong tree. The fix used was to stop using the MCP tools and
drive the CLI directly from the working clone:

```bash
PHYSIS_ALLOW_DEV_LICENCE=1 ./target/debug/physis-pro system remember --outcome success \
  --actor opencode --root /home/gio/physis_pro "<what happened>"
```

`--root` is explicit there, which is the whole point. **Before trusting any MCP
result, confirm the root matches the checkout you are editing.**

### The CLI surfaces that did the work

`system remember` (outcome memory), `agent call|ask|tasks|route`, `action
list|show|call`, `intent show|set|history`, `settings show|path|set`, `decide`
(decision-native classification), `attention`, `draw` (every surface as text),
`pack --save`. The HUD: `physis-hud --snapshot=WxH` renders one frame through
the real `draw()` path, so a claim about the TUI can be checked without a TTY.

### Traps that cost real time

1. **`pkill -f '<pattern>'` kills the shell running it.** The pattern matches
   the shell's own command line, so everything after the `pkill` in that command
   silently never runs. Symptom: "the command halts without running anything".
   Use `kill <pid>`, or a pattern that cannot match your own command line.
2. **Backticks and angle brackets in `git commit -m "..."` are shell.** A
   backticked phrase is command-substituted (the message loses it); `<state>`
   is a redirection (the commit does not run). Write messages with no backticks
   and no `<`.
3. **`cargo test --all-targets` twice in one command is ~26 minutes.** Run it
   once and grep the captured output, or run it in the background.
4. **`cargo fmt` is repo-wide and the tree was not fmt-clean.** It rewrote ~95
   unrelated files. Revert everything outside your change set
   (`git diff --name-only | grep '\.rs$' | grep -vE '<yours>' | xargs git checkout --`)
   and re-apply your edits; never commit the sweep as if it were yours.
5. **Fixture tests that write a script and exec it flake with `ETXTBSY`** under
   parallel `cargo test`: a sibling's forked child inherits the write fd, and the
   kernel refuses to exec a file open for writing. Run `sh -c "<script>"`
   instead — no file, no race.
6. **`tokio::task::block_in_place` panics on a current-thread runtime**, which
   is what `#[tokio::test]` builds. Guard on
   `Handle::runtime_flavor() == MultiThread`; the real server is multi-threaded,
   tests are not.
7. **`opencode run` defaults to a model that is out of funds.** Always pass
   `-m`. The pinned working free model is `oczen/nemotron-3.5-lightning-free`;
   a bare turn is ~43 s and a repo-exploring question ran 3 m 10 s.
8. **The adapter appends its own reply contract to every prompt.** When a caller
   wants a different JSON schema (a judge, say), the two collide and the model
   may answer the wrong one — or return an empty result. Make the contract
   opt-in per call.
9. **A cold `pack` over the whole tree is minutes of CPU.** `pack --save` then
   `agent ask --pack` is the fast path; `--max-files` bounds a fresh walk.

### The gates

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets            # ~13 min; the done gate
```
Focused first: `cargo test --lib <module>::`, `cargo test --test <name>`. A
failing suite stops cargo unless you pass `--no-fail-fast`, so use it when you
expect more than one break.

---

## One clone on pansa (2026-09-20)

There used to be two clones of this repo on pansa and they drifted: the MCP
server was scoped to one while the work happened in the other, and the same
mistake is easy to repeat. **There is now one tree:**

```
/home/gio/physis_pro                 <- the real directory, branch agent/MS-005-q6
/home/gio/dev/physis-pro  ->  /home/gio/physis_pro     (symlink)
```

Both paths resolve to the same working tree, so the `.mcp.json` root
(`/home/gio/dev/physis-pro`) and the directory you edit are the same checkout.
`readlink -f /home/gio/dev/physis-pro` is the check.

Before retiring the second clone its local `main` commits (10 of them, unpushed)
were imported into this tree as `local-main-import` **and pushed to the remote**,
so nothing lives only on a disk. The retired directory is kept at
`/home/gio/dev/.physis-pro-retired-20260920-014539` (19 GB, mostly build cache
and git objects) and is safe to delete once you have confirmed you do not want
it.

**Do not clone again.** If you need a second working tree, use
`git worktree add` — it shares the object store instead of duplicating it, and
it cannot drift into a different history the way a second clone did.
