---
id: PH-027
track: physis-pro
phase: 0
status: claimed
rung: 3
depends: [PH-026]
gate: "cargo clippy --all-targets -- -D warnings && cargo test --lib --features tui hud:: && cargo test --lib world_object:: && cargo test --features tui --bin physis-hud && cargo build --features tui --bin physis-hud"
commit: "branch agent/PH-027; conventional commits; per PROTOCOL §6"
claimed_by: cline-pansa-2026-09-21
claimed_at: 2026-09-21
---

# The world surface becomes operational: intent, two lenses, structured rows

> Claim note. PH-026 is `status: review` and was authored by this agent, so §7's
> independent review is **not** something I may perform — it stays `review` until
> another reviewer re-runs its gate. The dependency this packet needs is
> PH-026's shipped work (on `agent/PH-026`), which exists and is gated.

## Why (measured, not assumed)

PH-026 landed the projection contract and one lens
(`hud::lens_object::ObjectLens`, wired into `graph_view` on 2026-09-21). Two
measured facts bound what comes next:

1. **One lens is not a registry.** The `ProjectionId` set is still a closed
   eight-variant enum, and the only lens is the object one. A trait built now
   would be built for one caller — the "generalizes before the concept is
   shared" case. Build the registry when a *second* lens exists that cannot be
   expressed as data.
2. **`hud::intent` is tested but not called.** The bare-text resolution chain
   (`ResolutionRoute`, `compile_bare`) exists and is covered by three tests,
   and `submit_command_line` does not consult it. That is the repo's named
   defect shape (declared, reachable, never called) sitting in the file we just
   wrote, so it is the first thing this packet fixes.

## Steps

1. **Wire the chain into the textbox** (`src/bin/hud.rs::submit_command_line`):
   after `parse_hud_input` returns a `SemanticQuery`, resolve it through
   `hud::intent::compile_bare` with the objects the query actually matched, and
   show the provenance line (`route=intent-ir objects=N nil=[…]`) in the status
   bar. Deterministic-first means the *first* leg of the chain is the parser
   that already exists; this packet adds the second leg and labels it.
2. **Make the chain's routes real, not aspirational.** `ExactFact`, `Guided`
   and `AgentEscalation` are currently enum variants with no producer. Either
   give each a caller (guided already exists as a command; a fact resolver
   would be new work) or delete the variants this packet does not implement.
   An enum of routes with no producers is the same defect one level down.
3. **Second lens: SEMANTIC**, expressed as data over `ObjectLens`-style rows
   (meaning-neighbours, bridges, semantic parents) drawn from the same ranked
   list, with `Inferred` edges. Only when its row shape differs from the object
   lens should a trait appear — and then the trait is derived from two
   implementations, not invented ahead of them.
4. **Snapshot per lens**: `--view semantic` renders a frame on a fixture, so the
   feather matrix (see below) covers it without a TTY.
5. **Name the score legs honestly too.** The header was fixed (PH-026), but a
   lexical index still shows `semantic 0.79` among the hit components and the
   list pane is titled "semantic neighborhood" — measured on feather in the
   `--view graph` frame. Either name the component for the leg
   (`vector`/`lexical-hash`) or hide it when `nav::is_semantic_kind` is false.
   Leaving it is the same substitution the header fix removed, one layer down.

## Gate

As in the frontmatter, plus the feather matrix in §Delivery.

## Never

- No lens trait with one implementation.
- No route variant without a producer.
- No second ranking: a lens reads the ranked list the shared scorer produced.

## Delivery: the client machine

`feather` is the HUD client (`~/.config/agents/FLEET.md`): Debian 13, glibc
**2.41**, 6 cores, no embedding model. Measured 2026-09-21: `feather` (the
tailscale name) is offline, the live node is `feather-1`, and **Tailscale SSH
demands an interactive approval** — the LAN path (`gio@192.168.1.14`) works with
keys. pansa's glibc is 2.43, so a native pansa build cannot run there; build in
the `rust:1-trixie` container and push with `scripts/push-feather.sh`.

```bash
# on pansa
docker run --rm --user "$(id -u):$(id -g)" -v "$PWD":/app -w /app \
  -e CARGO_TARGET_DIR=/app/target-trixie-mine rust:1-trixie \
  cargo build --release --no-default-features --features cli,tui --bin physis-hud
PHYSIS_FEATHER_HOST=gio@192.168.1.14 PHYSIS_TARGET_DIR=target-trixie-mine/release \
  scripts/push-feather.sh physis-hud
```

Every packet that touches the HUD surface re-runs the matrix there:
`--snapshot` on `{60x18,100x30,160x50}` × `{world,graph}`, with
`PHYSIS_EMBEDDER=random-projection`, and the header must read `LEXICAL ONLY`.
