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