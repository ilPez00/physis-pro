# Dashboard Mission — todo-dash.md (separate from transform mission todo.md)

## M1 Audit — done
- [x] Read pulse.html reference, ui_chat handler pattern, route registration

## M2 Dashboard — done
- [x] Create src/bin/ui/dashboard.html (read-only, 7 endpoints, 15 MCP tools, license note, offline-safe)
- [x] Add dashboard_ui_handler() in src/web/ui_chat.rs
- [x] Register .route("/dashboard") next to /pulse in src/web/mod.rs
- [x] Scoped gates pass (lib + both bins); --all-targets failure is foreign tests/epistemic_evolution.rs E0063 only (pre-existing, transform mission)

## M3 CLI — done
- [x] CLI wiring: Dashboard/Chat/Embed/Mcp in src/main.rs (early dispatch, license-required)

## M4 Verify — done
- [x] Final check + report files changed, remains

## Mission complete — all scoped gates pass, no source touched in verification
