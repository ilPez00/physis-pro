# Physis Pro Roadmap

## Hosting Setup
- Keep `praxisweb.xyz` served from pansa-ts (`gio@100.65.59.18`, alias
  `pansa-ts` in `/home/gio/.zshrc`)
- Document and test the pansa-ts reverse proxy, TLS termination, and deploy
  restart path

## Core Development
- Optimize facet schema performance in `src/models.rs`
- Add ontology facet drilldown in Studio GUI
- Expand CLI support for facet queries

## Documentation
- Complete physis-docs engine pipeline diagrams
- Add Umbrellapedia integration guide
- Adapt repowise patterns clean-room (Phase 9 in `PLAN.md`): wiki generation
  over the holarchy, session-transcript adapters, grounding gate, decision
  mining, community detection — see `docs/reference/repowise-adaptation.md`

## Collaboration
- Implement buddy system for ontology maintenance
- Schedule next planning session

## Epistemic revision (Atlas + Graphiti)
- Ripple reassessment pass over dependents — PLAN.md Phase 22 (A1) + integration plan §§1/4
- DependsOn-only walk replacing the cosine wave — PLAN.md Phase 22 (A2) + integration plan §1
- AGM shape gate over status transitions — PLAN.md Phase 22 (A3) + integration plan §§1/5
- Hash-chained provenance links + verify walk — PLAN.md Phase 22 (A4/G4) + integration plan §§1–2
- Adjudication routing (small-Δ auto, large-Δ Open with rationale) — PLAN.md Phase 22 (A5) + integration plan §1
- expired_at leg + invalidate-don't-delete + ingest triple — PLAN.md Phase 22 (G1/G7/G2) + integration plan §2
- Episode trail + point-in-time queries — PLAN.md Phase 22 (G3/G6) + integration plan §2
- Hybrid BM25+RRF vs frozen cosine baseline — PLAN.md Phase 22 (G5) + integration plan §§2/5
- Monster-benchmark mapping + falsifiers per take — PLAN.md Phase 22 (22.5) + integration plan §5
- Formal semantics: Γ/R operator separation + additive-construction invariant + provisional fixed point — PLAN.md Phase 23 (23.1–23.3) + physis_resources.txt
- Replay/dream candidates from failed/inert/contradicted structures — PLAN.md Phase 23 (23.4)
- Critical experiment: RAG vs AGM vs TMS/ATMS vs Atlas/Ripple vs Physis, numbers only with nulls — PLAN.md Phase 23 (23.5) + physis_resources.txt
- TMS-family takes: ATMS environment labels, NATMS negated justifications, JTMS retraction cascade, Nixon-Diamond scenario, Atlas quarantine/promotion, Graphiti graph-distance leg + community rollups — BRAINSTORM_ATLAS_NTMS_TMS_GRAPHITI.md (T1–T7)
- Wave 2: decision lineage + foundation-weakening alerts, alias resolution, per-stream confidence floors, authority registry, federated adjudication, SSE trail stream, bounded ingest cycles, working-memory pressure, group-id namespaces, driver trait layer, two-clock watermark — BRAINSTORM_ATLAS_NTMS_TMS_GRAPHITI.md (T8–T18)
- opencode-orchestrator inspection: it retired its own hybrid-RAG + decay memory (ADR-0019) — external evidence for §2b; ADR supersession chains + structured session handoff adopted as conventions — BRAINSTORM_ATLAS_NTMS_TMS_GRAPHITI.md (O1–O4)
- Temporal dream: retrospective proposals over the time machine (reactivate / restore / retire old branches, proposals only) + the timelessness constraint — PLAN.md Phase 23 (23.5/23.6) + mindalogue.md
