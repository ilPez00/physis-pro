# Physis Pro Dev Plan

## In Progress
- DNS TXT verification for `praxisweb.xyz` — add TXT record `_github-pages-challenge-ilPez00.praxisweb.xyz` = `b2452977bcac84567aa7f09bf743e2` at DNS provider
- Enable GitHub Pages on `ilPez00/physis-pro`: Settings → Pages → Source: gh-pages / root

## Completed (Roadmap Reference)
- ROADMAP.md created with hosting setup, core, docs, and collaboration milestones
- Workspace configuration (`Cargo.toml` with `[workspace]`, `physis-core` member)
- CI pipeline: `.github/workflows/physis-core.yml` — checks, clippy, tests all pass
- `physis-docs/index.html` created with engine overview, hierarchy, publishing info
- `CNAME` set to `praxisweb.xyz`
- All commits pushed to `gh-pages` branch
- **Semiotic node editing (exchange parts)** — `PhysisCore::edit_node`/`delete_node` swap a node's content (label + embedding) while preserving structure (id, verdict, holarchy edges, provenance); `CoherenceNode.cell_pin` hybrid pin. Web: `POST /api/v1/coherence/edit` + `/delete`. Resolves the DEVELOPMENT_PLAN TODO; design in `docs/SEMIOTIC_EDITING.md`
- **Fixed-token node search** — `POST /api/v1/search/nodes`, budget-bounded similarity over coherence + goal nodes (`total_tokens ≤ token_budget`) via `rag::TokenFixedRetriever`
- **CLI node editing** — `physis-pro node-edit` / `node-delete` / `node-search`, sharing the web server's sled store with matching embedder provenance
- **Watch-mode auto-ingest (Phase 7B)** — background thread (`PHYSIS_WATCH_DIR`), `POST /api/v1/watch` on-demand sweep, and `physis-pro watch <dirs> [--once]` register new/changed files as labeled core nodes and drop nodes for removed files; shared `FileInfo::labels()` + `PhysisCore::delete_nodes_by_label_prefix`
- **Vault importers (Phase 7B)** — `src/vault.rs` (Obsidian/markdown, plain notes, `git log`) → labeled core nodes; `physis-pro vault <dir> [--git N]` + `POST /api/v1/vault/import` (scan-root confined, dedupe by label)
- **Personal-history importers (Phase 7B)** — `src/history.rs` (browser bookmarks/history, OPML feeds, chat JSONL) → labeled core nodes; `physis-pro history <file>` + `POST /api/v1/history/import` (scan-root confined, dedupe by label)
- **Praxis backfill (Phase 7B)** — `src/praxis.rs` (notebook entries + trackers → Success/Inert/Failure verdicts) → labeled behavioural nodes with asserted verdicts; `physis-pro praxis <file>` + `POST /api/v1/praxis/backfill`

## Pending Hosting Actions (user required)
1. **DNS TXT** — Contact DNS provider to add TXT record. Once propagated, enable GitHub Pages
2. **GitHub Pages enable** — After DNS verified, go to repo Settings → Pages → Build and deployment → Source: Deploy from a branch → gh-pages / root → Save
3. **Verify live site** — Visit `praxisweb.xyz` and `praxisweb.xyz/physis`

## Core Dev Track
- Facet schema in `physis-core/src/models.rs` — optimize LifecyclePhase, HumanMode, Scale, Abstraction enums
- Studio GUI facet drilldown integration
- CLI facet query commands

## Documentation Track
- Umbrellapedia integration guide (linked from physis-docs)
- Complete engine pipeline diagrams

## Collaboration Track
- Buddy system setup for ontology maintenance (23 machine + 21 agent domains)
- Schedule planning session