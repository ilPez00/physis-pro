# Project Context

## Environment
- Language: Rust 2021 edition
- Runtime: cargo build/test
- Build: `cargo check --all-targets` (33s), `cargo test` (**790s**)
- Package Manager: cargo
- Project Type: Monoroot workspace (physis-pro + physis-core)
- Repository: /home/gio/dev/physis-pro

## Project Type
- [x] Library/Package (physis-core published crate)
- [x] Application (physis-pro: web server + CLI + ingestion)
- [x] Microservice (physis-pro-web HTTP surface)
- [x] Multi-tenancy supported (X-Physis-User header)
- [ ] Monorepo (2 workspace members)

## Infrastructure
- Container: None (native Rust binary)
- Orchestration: None
- CI/CD: GitHub Actions (`.github/workflows/`)
- Cloud: None
- License: Proprietary EULA (LICENSE.txt), Ed25519 gate in src/license.rs

## Structure
- Source: `/home/gio/dev/physis-pro/src/` (150+ .rs files)
- Tests: `/home/gio/dev/physis-pro/tests/` (5 test binaries)
- Docs: `/home/gio/dev/physis-pro/docs/` (architectural docs)
- Config: `/home/gio/dev/physis-pro/config/` (30+ ontology JSON files)
- Data: `/home/gio/dev/physis-pro/.physis/` (state.sled, wiki, quality_failures.json, embedder cache)
- Core: `/home/gio/dev/physis-pro/physis-core/` (sub-crate, 0.1.23)

## Conventions (OBSERVE from existing code)
- Naming: snake_case for functions/variables, PascalCase for structs/enums
- Imports: `use crate::...` within crates, external crates as `crate_name::...`
- Error handling: `anyhow::Result`, `?` operator, explicit error handling
- Testing: `#[cfg(test)] mod tests` embedded in source files, `cargo test` runs all
- Feature gates: `#[cfg(feature = "pdf")]`, `#[cfg(feature = "embed-onnx")]`, feature flags in Cargo.toml
- Enums: HumanDomain (5 values), HumanMode (14 values), CoherenceRating (3 values), DocKind (13 values)
- Grid: 5 domains × 14 modes = 70 semiotic cells
- NUL character: `\0` used as separator in cell keys (e.g., "WORK\x00DO")
- JSON persistence: serde with `#[serde(default)]` for backward compatibility
- Tenant isolation: `X-Physis-User` header, `_default` tenant = shared global state

## Key Modules
| Module | File | Status |
|--------|------|--------|
| Quality | `src/quality.rs`, `physis-core/src/quality.rs` | ✅ Explored (QualityTracker, penalties/boosts) |
| Coherence | `src/web/coherence.rs`, `physis-core/src/core.rs` | ✅ Explored (PhysisCore, node graph) |
| Classification | `src/classify.rs`, `physis-core/src/classify.rs` | ✅ Explored (CellClassifier, 70 cells) |
| Ontology | `src/config.rs`, `physis-core/src/ontology.rs` | ✅ Explored (OntologyLoader, 30+ ontologies) |
| Ingest | `src/ingest.rs` | ✅ Explored (DocKind, extraction backends) |
| Graph | `src/graph.rs` | ✅ Explored (FileThemeGraph, theme/file edges) |
| Address | `src/address.rs` | ⚠️ Not fully explored (Phase 10 FileAddress) |
| Ledger | `src/ledger.rs` | ⚠️ Not fully explored (CausalLedger) |

## Ontology Summary
- 9 built-in ontologies: human, machine, semiotic, category, agent, natural, social, abstract, engineering
- Office ontology: 75 domains across 12 categories (loaded as kind: "office", axis_kind: "office")
- Machine ontology: 100+ domains including 23 new process domains (Thermal Zone through Yield & Scrap)
- Agent ontology: 60+ domains including 21 new workflow domains (System Prompt through Telemetry & Log)
- Classification domains: all ontologies merged, but space/period axes excluded (filing axes, not themes)
- Office domains validated: domain∈{STUDY,HEAL,FABRICATE,CONSTRUCT,BOND}, mode∈{WORK,WALK,LEARN,CREATE,REST,LIFT,DESTROY,SENSE,GUIDE,PLAY,BRAINSTORM,MAINTAIN,MOVE,PLAN}
- Machine domains validated: >= 100 entries, includes Thermal Zone Monitoring, Anomaly Detection, OEE Metrics
- Agent domains validated: >= 60 entries, includes System Prompt, Working Memory, PDCA Cycle

## Cargo Features
- default: ["cli", "voice", "web", "tui", "sql", "postgres", "email", "embed-onnx"]
- full: adds industrial, ingest, pdf, email, audio, ocr, sql, postgres, mcp
- base: ["cli", "voice", "web", "tui"]
- embed-onnx: requires ort + tokenizers
- Industrial: no-op kept for compat, MQTT/Modbus unconditional
- Ingest: calamine (spreadsheets), tesseract (OCR), pdf-extract, mailparse
- Wiki-embedded: llmwiki_physis in-repo vendor
- Spanner: jsonwebtoken for SA auth

## Cross-agent coordination (2026-09-11)
- Qwen n-gram agent owns: physis-core/src/embed_ngram.rs (untracked), physis-core/src/epistemic.rs (modified), physis-core/tests/test_ngram_embed.rs. DO NOT touch.
- Structural-reasoning mission owns: physis-core/src/transform.rs (new), physis-core/tests/transform_bench.rs (new), docs/TRANSFORM_REASONING.md (new), physis-core/src/lib.rs (append-only export lines).
- physis-core is a submodule: NO commits inside it; leave working tree uncommitted.
- Loom MCP plan tool broken (jieba missing) — Commander runs missions via direct delegation.

## Research Artifacts (committed 2026-09-11, branch research/perspective-invention)
- **Impossible Machine / Apodeixis**: Lean formalization of Riemann Hypothesis attack
  - `research/impossible_machine/apodeixis/` — Chain.lean, Target.lean, Main.lean
  - Experiments E48 (geometry→spectral), E49 (self-consistent dimension), E50 (representation equivalence)
  - Status: All three experiments completed; formal target remains OPEN
- **Perspective Invention**: Testing whether Physis can discover coordinate systems
  - `research/perspective_invention/` — hypothesis, methodology, architecture, datasets, experiments, visualizations
  - Status: Tier 0 FAILED (no cross-perspective convergence)
- **Wiring documents**: `wiring/funding_draft_targets.md`, `wiring/neurosemantic_wiring.md`, `wiring/physis_rnd_assessment.md`

## Known Issues
- 9 clippy mechanical warnings (RED gate) - need to fix before CI green
- output_quality.rs gated behind embed-onnx feature (needs model on disk)
- Some test binaries carry full DWARF (CI disk limits issue, line-tables-only profile)
- physis-core submodule has untracked research/ directory with experiment results