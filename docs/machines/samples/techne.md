# Sample output — techne (codebase machine)

Demonstration of `src/machines/techne.rs`. Reproduce with:

```bash
cargo run --example techne_demo
```

## Input

11 of the engine's own modules + their `use crate::…` imports (a representative
slice of physis_pro). techne reduces them to capabilities + a dependency graph,
then traces impact.

## Output

```
techne — codebase machine
perceived 11 files

CAPABILITIES (files clustered by purpose):
  • authentication   src/auth_guard.rs
  • classification   src/classify.rs
  • embedding        src/embed/clip.rs, src/embed/mod.rs
  • graph            src/graph.rs
  • http-api         src/bin/web_server.rs
  • machines         src/machines/chronos.rs, src/machines/techne.rs
  • misc             src/core.rs, src/trie.rs
  • persistence      src/spanner.rs

DEPENDENCY EDGES (module → module):
  clip → embed
  classify → embed
  core → trie
  core → graph
  core → embed
  chronos → core
  chronos → embed
  techne → core
  web_server → core
  web_server → classify
  spanner → core

IMPACT — what breaks if `embed` changes:
  ⚠ chronos ⚠ classify ⚠ clip ⚠ core ⚠ spanner ⚠ techne ⚠ web_server

IMPACT — what breaks if `core` changes:
  ⚠ chronos ⚠ spanner ⚠ techne ⚠ web_server
```

## What it demonstrates

- **Reduction to capabilities** — 11 files cluster into 8 purposes ("embedding"
  pulls `clip` + `mod` together; the capability, not the file, is the unit).
- **Composition** — `use crate::…` lines become a module dependency graph; only
  edges to known modules are kept (external crates dropped).
- **Module-root resolution** — `src/embed/mod.rs` resolves to module `embed` (its
  parent dir), so `use crate::embed::…` correctly links (a bug the demo caught
  and the fix closed — see `module_of`).
- **Elaboration = impact** — transitive reverse-reachability answers "what breaks
  if this changes": editing `embed` ripples to 7 modules; `core` to 4.
- **Reuse** — pairs with `/api/v1/scan` (already registers file + structural-line
  nodes); `register_capabilities` adds capability nodes + dependency edges into
  the shared graph via `add_edge_between`.

## Next

Wired through `register_capabilities`, capabilities + dependency edges become
content-recallable and graph-walkable; a "what depends on X" query is then a
graph traversal in the wiki, not a grep.
