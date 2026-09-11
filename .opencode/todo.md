# Mission: Explore physis-pro Project Structure

## M1: Project Discovery and Structure
### T1.1: Overall Project Mapping
- [x] S1.1.1: Map root directory structure and identify all major folders
- [x] S1.1.2: Identify workspace members (physis-pro + physis-core)
- [x] S1.1.3: Catalog binary executables and their features
- [x] S1.1.4: Document data directories (.physis, models, etc.)

### T1.2: Source Code Structure
- [x] S1.2.1: List all source files in src/ and categorize by module
- [x] S1.2.2: Identify web handlers vs core engine vs ingest modules
- [x] S1.2.3: Map module dependencies and import patterns

### T1.3: Config and Ontology Files
- [x] S1.3.1: Catalog config.rs structure and PhysisConfig/PhysisProConfig
- [x] S1.3.2: List all ontology JSON files and their domains/kinds
- [x] S1.3.3: Identify built-in vs extra ontology sources
- [x] S1.3.4: Document domain classification maps

## M2: Core Engine Analysis (physis-core)
### T2.1: PhysisCore Architecture
- [x] S2.1.1: Examine core.rs - node management, coherence, hypotheses
- [x] S2.1.2: Understand quality.rs - QualityTracker, penalties, boosts, persistence
- [x] S2.1.3: Analyze models.rs - coherence ratings, grid positions, domain/mode enums
- [x] S2.1.4: Examine ontology loading and domain maps

### T2.2: Classification System
- [x] S2.2.1: Study classify.rs - CellClassifier, nearest-centroid classification
- [x] S2.2.2: Understand semiotic grid: 5 domains × 14 modes = 70 cells
- [x] S2.2.3: Examine grid position computation from ontology entries
- [x] S2.2.4: Review classification domains iterator (Phase 10c)

### T2.3: Ontology System
- [x] S2.3.1: Study ontology.rs in physis-core
- [x] S2.3.2: Load and analyze built-in ontologies (9 base + office + machine + agent)
- [x] S2.3.3: Understand office ontology: 75 domains across 12 categories
- [x] S2.3.4: Understand machine ontology: 100+ domains including 23 process domains
- [x] S2.3.5: Understand agent ontology: 60+ domains including 21 workflow domains

## M3: Web and API Layer (physis-pro src/web/)
### T3.1: Coherence Web Handlers
\[x\]S3.1.1: Examine coherence.rs handlers (/snapshot, /query, /nodes, /edit, /delete)
- [x] S3.1.2: Understand facet querying by verdict and cell
- [x] S3.1.3: Study node instances endpoint (/coherence/nodes/{id}/instances)
- [x] S3.1.4: Analyze edit_node and cell pin semantics

### T3.2: Quality API
\[x\]S3.2.1: Study quality.rs handler (/api/v1/quality)
- [x] S3.2.2: Understand penalty/boost display and failure records
- [x] S3.2.3: Review quality_response payload builder

### T3.3: Inquiry and Ask
\[x\]S3.3.1: Study search.rs ask handler (/api/v1/ask)
- [x] S3.3.2: Understand grounded synthesis with [n] citations
- [x] S3.3.3: Review degradation path when no provider available

### T3.4: Communities and Pulse
\[x\]S3.4.1: Study communities handler (/api/v1/communities)
- [x] S3.4.2: Understand community detection and singleton handling

## M4: Ingestion and Document Processing
### T4.1: Document Classification
\[x\]S4.1.1: Study classify_file() - extension-based DocKind assignment
\[x\]S4.1.2: Understand voice detection (whole-word, not substring)
\[x\]S4.1.3: Review spreadsheet extraction (calamine feature-gated)

### T4.2: Text Extraction Backends
\[x\]S4.2.1: PDF text extraction (pdf-extract feature)
\[x\]S4.2.2: Email parsing (mailparse feature)
\[x\]S4.2.3: OCR via Tesseract (ocr feature)
\[x\]S4.2.4: Spreadsheet extraction (calamine feature)
\[x\]S4.2.5: Audio transcription (audio feature, Whisper API)
\[x\]S4.2.6: Video metadata (ffprobe)

### T4.3: Ingest Pipeline
\[x\]S4.3.1: Study ingest_all() and walk_and_ingest()
\[x\]S4.3.2: Understand file type skipping (.html, .json, .physis)
\[x\]S4.3.3: Review template extraction (PLAN 11 DocumentTemplate)
\[x\]S4.3.4: Study sort_by_kind() grouping by DocKind

## M5: Test Structure and Quality Gates
### T5.1: Unit Test Coverage
- [x] S5.1.1: Catalog test binaries in tests/
- [x] S5.1.2: Review smoke_test.rs - mediation pipeline integration
- [x] S5.1.3: Review industrial_benchmark.rs - SECOM tests
- [x] S5.1.4: Analyze classification_fixtures.json - 421 test cases

### T5.2: Build and Lint Gates
- [x] S5.2.1: Verify cargo check --all-targets passes
\[x\]S5.2.2: Review clippy warnings (currently 9 mechanical warnings)
\[x\]S5.2.3: Understand test profiles (line-tables-only for debug)

### T5.3: Integration Test Suites
\[x\]S5.3.1: Study output_quality.rs probe (embed-onnx gated)
- [x] S5.3.2: Review classification reach tests
\[x\]S5.3.3: Remember/recall/ask/serve/cause ledger tests

## M6: Dependency and Cargo Configuration
### T6.1: Root Cargo.toml
\[x\]S6.1.1: Feature flags analysis (default, full, base tiers)
\[x\]S6.1.2: Dependency tree (ort, rusqlite, tokio-postgres, axum, etc.)
- [x] S6.1.3: Workspace members and resolution mode
\[x\]S6.1.4: Binary definitions (physis-pro, web, tui, voice, wiki)

### T6.2: physis-core Cargo.toml
\[x\]S6.2.1: Feature flags (cli, studio, embed-onnx)
\[x\]S6.2.2: Optional dependencies (ort, tokenizers)
- [x] S6.2.3: Library vs binary targets

### T6.3: Test Dependencies
- [x] S6.3.1: tempfile usage across test suites
- [x] S6.3.2: once_cell Lazy cells for ontology resolution
\[x\]S6.3.3: dev-dependencies (wordnet-db, wordnet-types in core)

## M7: Architecture Patterns and Conventions
### T7.1: Semiotic Grid Patterns
\[x\]S7.1.1: HumanDomain enum (Heal, Construct, Fabricate, Bond, Study)
\[x\]S7.1.2: HumanMode enum (14 modes: Lift..Plan)
- [x] S7.1.3: GridPosition from_ontology_entry parsing
\[x\]S7.1.4: cell_index computation (domain_idx * 14 + mode_idx)

### T7.2: Quality Feedback Loop
- [x] S7.2.1: report_failure → embed → cosine → top cell → penalty accumulation
- [x] S7.2.2: report_success → boost → penalty reduction
\[x\]S7.2.3: adjust_score → raw_score * (1.0 - penalty + boost)
- [x] S7.2.4: Persistence to ~/.physis/quality_failures.json

### T7.3: Tenant Isolation
- [x] S7.3.1: X-Physis-User header routing
- [x] S7.3.2: _default vs isolated RuntimeState
\[x\]S7.3.3: Holarchy background threads (shared global state)

### T7.4: Ontology Loading Patterns
\[x\]S7.4.1: OntologyLoader::load_all() merging human + machine + custom domains
\[x\]S7.4.2: classification_domains() iterator (excludes space/period axes)
\[x\]S7.4.3: Extra ontology loading (keyed "kind:name" to avoid collisions)
- [x] S7.4.4: Pack loading from data_dir/packs/

## M8: Data Structures and Models
### T8.1: Core Data Models
- [x] S8.1.1: CoherenceNode - embedding, asserted, label, cell_pin
- [x] S8.1.2: QualityFailure - id, timestamp, feedback, top_cell, severity
- [x] S8.1.3: DomainDef - name, category, domain, mode, axis_kind, hints, facets
- [x] S8.1.4: OntologyEntry - name, category, domain, mode, axis_kind, axis_name, unit, hints, facets

### T8.2: Grid and Classification
\[x\]S8.2.1: SemioticGrid - 70 cells, classify(), heatmap_matrix()
\[x\]S8.2.2: CellClassifier - nearest-centroid, build(&loader, &embedder)
\[x\]S8.2.3: CoherenceRating - Success(1.0), Inert(0.0), Failure(-1.0)
- [x] S8.2.4: GridPosition - domain, mode, axis_kind, axis_name, facets

### T8.3: Address and Ledger (Phase 10)
\[x\]S8.3.1: FileAddress { space, time, cause, mechanism, actor }
\[x\]S8.3.2: CausalLedger - append-only cause/events, lineage(), descendants(), purposes()
\[x\]S8.3.3: Mechanism enum (generated/received/derived/corrected/cited/serves)

## M9: Completion Verification
### T9.1: Build Verification
- [x] S9.1.1: Run `cargo check --all-targets` and verify success
- [x] S9.1.2: Run `cargo clippy --all-targets -- -D warnings` and fix any new warnings
- [x] S9.1.3: Run `cargo test` subset of fast tests to verify no regressions

### T9.2: Documentation
- [x] S9.2.1: Ensure .opencode/context.md is populated with project summary
- [x] S9.2.2: Verify all key findings are documented

### T9.3: Final System Check
- [x] S9.3.1: Verify all TODO items marked [x]
- [x] S9.3.2: Confirm no unresolved sync-issues.md entries
- [x] S9.3.3: Confirm build and test suite passes

---

## M10: Impossible Machine / Riemann Hypothesis Attack (SHIPPED 2026-09-11)
### T10.1: Lean Formalization
- [x] S10.1.1: Build Chain.lean (classical reduction chain)
- [x] S10.1.2: Build Target.lean (genuinely new formal target)
- [x] S10.1.3: Build Main.lean (apodeixis executable)
- [x] S10.1.4: Verify lake build passes

### T10.2: Experimental Pipeline
- [x] S10.2.1: Implement experiment48_exponent_necessity.rs
- [x] S10.2.2: Implement experiment49_self_consistent_dimension.rs
- [x] S10.2.3: Implement experiment50_representation_equivalence.rs
- [x] S10.2.4: Run all three experiments and record results

### T10.3: Results Analysis
- [x] S10.3.1: Document E48 results (LEVEL 1 — no intrinsic dimension)
- [x] S10.3.2: Document E49 results (REPRESENTATION-DEPENDENT)
- [x] S10.3.3: Document E50 results (REPRESENTATION-EQUIVALENCE)
- [x] S10.3.4: Record formal target as OPEN

---

## M11: Perspective Invention Experiment (SHIPPED 2026-09-11)
### T11.1: Research Framework
- [x] S11.1.1: Write hypothesis.md with pre-registered tiers
- [x] S11.1.2: Write methodology.md with frozen pipeline
- [x] S11.1.3: Write ARCHITECTURE.md with gap analysis
- [x] S11.1.4: Generate datasets and calibration

### T11.2: Experiment Execution
- [x] S11.2.1: Run perspective_invention_experiment
- [x] S11.2.2: Generate visualizations (8 SVGs)
- [x] S11.2.3: Record results in experiments/results.json

### T11.3: Analysis
- [x] S11.3.1: Classify outcome as Tier 0 FAILED
- [x] S11.3.2: Document failures honestly

---

## M12: Dashboard Mission (SHIPPED 2026-09-11)
### T12.1: Dashboard UI
- [x] S12.1.1: Create src/bin/ui/dashboard.html
- [x] S12.1.2: Add dashboard_ui_handler in src/web/ui_chat.rs
- [x] S12.1.3: Register /dashboard route

### T12.2: CLI Wiring
- [x] S12.2.1: Wire Dashboard/Chat/Embed/Mcp in src/main.rs
- [x] S12.2.2: Verify scoped gates pass

---

## M13: Physis Corrective Feedback Engine (INTERRUPTED)
### T13.1: Core Type Definitions
- [ ] S13.1.1: Define Artifact dataclass with data and metadata fields | size:S
- [ ] S13.1.2: Implement Artifact.get() and Artifact.set() methods | size:S
- [ ] S13.1.3: Implement Artifact.to_dict() and Artifact.from_dict() | size:S
- [ ] S13.1.4: Implement Artifact.__str__() and Artifact.__repr__() | size:S

### T13.2: Intent and Observation
- [ ] S13.2.1: Define Intent dataclass | size:S
- [ ] S13.2.2: Define Observation dataclass | size:S
- [ ] S13.2.3: Define Constraints dataclass | size:S

### T13.3: Environment Abstraction
- [ ] S13.3.1: Define Environment abstract base class | size:S
- [ ] S13.3.2: Implement PythonEnvironment with subprocess | size:M

### T13.4: Diagnosis and Mutation
- [ ] S13.4.1: Define Diagnosis dataclass | size:S
- [ ] S13.4.2: Implement minimal_mutation() function | size:M

### T13.5: Generator and CoherenceEngine
- [ ] S13.5.1: Define Generator abstract class | size:S
- [ ] S13.5.2: Implement CoherenceEngine.run() main loop | size:M

### T13.6: Tests and Demo
- [ ] S13.6.1: Write integration tests | size:M
- [ ] S13.6.2: Create runnable demonstration | size:M
- [ ] S13.6.3: Write README | size:S

> **Status**: INTERRUPTED — see `physis_loop/.opencode/todo.md` for full task list
> **Location**: `/home/gio/dev/physis_loop/`
> **Priority**: Medium — research prototype, not production-critical