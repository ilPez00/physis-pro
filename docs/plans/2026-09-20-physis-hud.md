# PHYSIS HUD — one world, many projections (master plan)

Date: 2026-09-20. Status: proposed programme; **sequencing authority for the
HUD surface**. It composes — and extends — the agent-native programme in
[`2026-09-19-impossible-computer.md`](2026-09-19-impossible-computer.md), which
keeps its S0–S12 agent work; where the two overlap the packet board
(`packets/`) is the executable record. Planned acceptance gates are not shipped
capabilities.

Goal: `physis-hud` as the one front door — graph-native navigation, semantic
retrieval, notebook, editor, shell, agents, memory, provenance and actions as
views over one continuously updated world model. Offline-first: zero network
and zero model access must leave the HUD fully useful. Models are accelerators,
never foundations. No LLM in the command path (the repo rule; the spec agrees).

## Reuse map (measured on `3b29780` + working tree)

| Spec piece | Existing | Verdict |
|---|---|---|
| WorldObject | `nav::PhysisObject` (`src/nav/mod.rs:103`), content-hash ids, `ObjectKind`, `SourceRef` spans | reuse — no second object type |
| Typed relations | `nav::Relation` + `RelationOrigin{User,Deterministic,Inferred}`, pins, sled | reuse |
| Project lens | `NavIndex` containment + HUD directory rollup | adapt |
| Semantic lens | nav hybrid query (`NavWeights`, per-component `why`), `pack.rs` | reuse |
| Schema lens | ontology facets + `OntologyLoader` | expose |
| Content lens | `src/doctree.rs` sections, `src/content.rs` BM25+cosine, staleness walk | reuse |
| Object orbit | HUD `graph_view()`, dependents, contradictions | adapt |
| Dependency/causal | `mvp::deps`, `mvp::SemanticDelta`, `ledger.rs`; `src/dependency/`+staleness behind off-by-default `repair-pipeline` (import cycle — keep gated) | reuse |
| Temporal | `mvp::log::TransitionLog`, observation log, **`ported/world.rs` (dead — revive with caller+test)** | revive |
| Activity | `agents/tasks.rs::TaskGraph::fold`, roster, `attention.rs`, `similar.rs` | reuse |
| Fleet | `machines/`, `industrial.rs` adapters, `moonraker*.rs` | expose |
| Zoom | HUD zoom stack + crumbs + sled trail | reuse |
| Magic textbox | `parse_hud_input` (`$`/`:`/`@`/`open/edit/cd`/`/`), `intent_ir.rs` (unwired), `intent.rs` | reuse + wire IR |
| Notebook | `ported/notebook.rs::answer`, `src/notebook.rs` store | adapt (offline + object links) |
| Editor | HUD `EditBuffer`, `diff.rs`, `adaptive_document/` | adapt |
| Shell | `run_shell_command` → `ShellOutcome` | reuse + observation record |
| Agents | `agents/{adapter,routing,view,parse,decision}.rs`; `hermes`+`opencode` on PATH; `.opencode/opencode.json` mounts `physis_system` MCP; JEV via `providers.jev.endpoint` | reuse + hermes adapter |
| Config | `PhysisConfig::load_effective` origins pattern; `settings.rs` two layers; `config::CONFIG_KNOBS` (PH-024) | extend with `hud.toml` |
| Keymap / palette / panes | keys hardcoded; `registry.rs`+`action.rs` shared CLI+HUD | build |

Gaps to build: `hud.toml` layering, keymap table, command palette, lens trait +
9 lenses, multi-buffer editor, offline notebook store with `[[object:…]]` links,
hermes adapter, agent-session objects, shell observations, background rescan →
`HudEvent::SourceDiff`, fleet lens, `--view` snapshot matrix.

## Phases (packets; one at a time; gates in each packet)

| Phase | Packet | Content |
|---|---|---|
| 0 | `PH-024` (claimed) | views (world/graph/notebook/tasks/agents/settings/setup/help), editable settings (`Settings`+`CONFIG_KNOBS`), `--setup` screen, shared knob table (`tui.rs`/`studio.rs` delegate) |
| P0 | `PH-025` | `src/hud/{mod,event,keys,config,palette}.rs` — `HudEvent` reducer (headless-tested), `hud.toml` layering (builtin < `~/.config/physis/hud.toml` < `.physis/hud.toml` < flags), palette over Registry, `intent_ir` wired into the textbox chain |
| P1 | `PH-026` | lens trait + project/semantic/schema/content/object/dependency; hotkeys 1–6 from keymap; snapshot per lens |
| P2 | `PH-027` | revive `ported/world.rs` (caller+test), temporal+activity lenses, background rescan → delta → Δ badge, draw-before-scan startup |
| P3 | `PH-028` | multi-buffer editor (undo/search/goto/save→rescan→affected highlight), offline notebook + object links + provenance stamps |
| P4 | `PH-029` | shell observations, adaptive-document proposal preview→apply, palette mutations gated by `risk` |
| P5 | `PH-030` | hermes adapter, JEV MCP in OpenCode, agent-session WorldObjects, bounded Physis-selected context |
| P6 | `PH-031` | fleet lens over machines/industrial/moonraker, stale-tolerant remote state |
| P7 | `PH-032` | themes, mouse, session persistence, crash recovery, packaging (PH-010 ties in; Windows measured separately) |

## Rules carried into every packet

- Deterministic-first routing; textbox parsing stays a total, tested table.
- Degradation labelled, never silent (`semantic: off`, `zen:<model>` vs
  `jev:<model>`); agents asked on a worker thread (43 s–3 min measured).
- No new epistemic store; reduce the two-`PhysisCore` confusion, never extend it.
- Reviving dead code adds a caller and a test in the same change.
- Secrets stored as env-var names only; never printed.
- Windows/macOS packaging is a measured release-engineering item, not assumed.
- Dogfooding per `AGENTS.md`: `physis_history` → `physis_pack`/`physis_read`,
  `physis_remember` after gates, `physis_usage` before adoption claims.
