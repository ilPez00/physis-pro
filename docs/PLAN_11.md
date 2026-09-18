# PLAN 11 — Enhanced Ingestion, Citation, and Conceptual Search

## Goals

1. Expand ingestion pipelines to handle OCR, audio (mp3/wav), images, and video
2. Add structured output templates per file type (invoice, email, spreadsheet, transcript, image, video)
3. Cite exact fragments in ontology node visualization
4. List ALL instances (conceptual + actual) hyperlinked to their origins
5. Add JEPA-like reranker integration for classification disambiguation

## Architecture

### Enhanced Ingestion Pipeline

```
File → classify_file() → extract_doc() → Template Extraction → Embed → Classify → Register Node
                        ↘ (templates)
```

New DocKind variants:
- `DocKind::OcrDocument` — OCR-extracted text from scanned images/PDFs
- `DocKind::Image` — Image files (.png, .jpg, .webp, .bmp, .tiff) handled via CLIP/Jina embedders
- `DocKind::Video` — Video files (.mp4, .avi, .mov, .mkv) handled via ffmpeg frame sampling

### Structured Templates

Each file type produces a `DocumentTemplate` enum with type-specific metadata:

```rust
pub enum DocumentTemplate {
    Invoice(InvoiceTemplate),
    Email(EmailTemplate),
    Spreadsheet(SpreadsheetTemplate),
    Transcript(TranscriptTemplate),
    Image(ImageTemplate),
    Video(VideoTemplate),
    Generic(GenericTemplate),
}
```

Templates feed both:
- Structured metadata display in Studio UI
- Enhanced classification signals (e.g., invoice vendor influences classification)
- Fragment citation in NodeView

### Fragment Citation & Instance Listing

When displaying a coherence node:
1. Show the exact text fragment that contributed most to classification
2. List all nodes sharing conceptual similarity (same cell or high cosine)
3. Each instance hyperlinked to its origin file + line/position

### JEPA Reranker

Lightweight MLP reranker trained on ontology entries:
- Input: top-k candidate cell scores from cosine similarity
- Output: reordered scores with JEPA-derived adjustment
- Feature-gated behind `jepa-rerank` feature
- Falls back to pure cosine when disabled or unavailable

## Implementation Status

### Phase 11A — Structured Templates & Enhanced Ingestion (COMPLETED)

- ✅ `DocKind` enum extended with `OcrDocument`, `Image`, `Video` variants
- ✅ `DocumentTemplate` enum with 7 type-specific template structs
- ✅ `InvoiceTemplate`, `EmailTemplate`, `SpreadsheetTemplate`, `TranscriptTemplate`, `ImageTemplate`, `VideoTemplate`, `GenericTemplate`
- ✅ `extract_doc()` extended with OCR (tesseract), audio (Whisper), spreadsheet (calamine), video (ffprobe metadata) extractors
- ✅ `extract_ocr()` — Tesseract-based OCR for scanned documents and images
- ✅ `extract_audio_transcript()` — Whisper API transcription for mp3/wav/flac
- ✅ `extract_spreadsheet()` — calamine-based XLSX/CSV parsing
- ✅ `extract_video_metadata()` — ffprobe metadata extraction
- ✅ `classify_file()` updated to recognize new file extensions
- ✅ `sort_by_kind()` updated with new DocKind variants
- ✅ Feature gates: `ocr` (tesseract), `audio` (Whisper API), `ingest` (calamine)
- ✅ `IngestedDoc` extended with `template: Option<DocumentTemplate>` field
- ✅ Template extraction integrated into main ingestion loop
- ✅ Cargo features updated: `full` includes `ocr`, `audio`. `ingest` feature now includes `tesseract`

### Phase 11B — Fragment Citation & Instance Listing (COMPLETED)

- ✅ `CoherenceNode` extended with `template: Option<DocumentTemplate>` field
- ✅ `NodeView` extended with `fragment: Option<String>` and `instances: Vec<ConceptualInstance>` fields
- ✅ `ConceptualInstance` struct: `id`, `label`, `score`, `origin`, `position`, `cell`
- ✅ `CoherenceNode::extract_fragment()` — extracts most relevant text fragment
- ✅ `CoherenceNode::with_template()` — builder method for template attachment
- ✅ `CoherenceNode::with_embedder()` — builder method for embedder provenance
- ✅ `PhysisCore::find_conceptual_instances()` — finds conceptually similar nodes by cosine + cell
- ✅ `observable()` method updated to populate fragment from template or label
- ✅ Web handlers (`coherence_nodes_handler`, `coherence_nodes_all_handler`, `query_coherence_nodes`) updated to include fragment
- ✅ New endpoint: `GET /api/v1/coherence/nodes/{id}/instances`
- ✅ `InstanceParams` / `InstanceResponse` request/response types
- ✅ Tests: `test_find_conceptual_instances`, `test_node_fragment_extraction`

### Phase 11C — Conceptual Search (COMPLETED)

- ✅ `GET /api/v1/concepts/search?query=<text>&threshold=<f32>&limit=<n>` endpoint (`web/search.rs::concepts_search_handler`, tenant-scoped)
- ✅ Concept grouping by effective semiotic cell (pin wins, else classifier top hit) — `group` numbers cells by first appearance
- ✅ Salience-aware ranking via the quality tracker's `cell_standing` (`raw * standing`; both `score` and `adjusted` ship so the promotion is visible, never silent). Pure ranker `models::rank_concept_hits()` unit-tested
- ✅ CLI commands: `physis-pro concepts <query>`, `physis-pro node-instances <id>`, `physis-pro jump --node <id>`, `physis-pro context --node <id>`
- ✅ Tests: `concept_rank_*` (3), `test_cli_concepts_node_instances_jump_context`, `concepts_search_ranks_groups_and_isolates_tenants` (http_surface)

### Phase 11D — JEPA Reranker (SUPERSEDED — DO NOT BUILD)

- ❌ Trained JEPA-style predictive latent: **FALSIFIED at scale** — toy win was a supervision confound; chance-level on 730 entries under a shuffled-label control (`research/perspective-discovery/THESIS_EVALUATION_JEPA_QWEN.md` It. 3–4; Stage 12 paired t = −5.58 over 22 configs; It. 17b supervised metric learning ceiling 0.534 held-out with 0.277 generalization gap)
- ✅ What ships instead: the quality-tracker salience re-rank above (`cell_standing` = `1 − penalty + boost`, the same multiplier `adjust_score` already applies to live classification) plus the existing LLM `rerank_cell` for ambiguous classify calls. No new model, no new feature gate, no training infrastructure — the plan's `jepa-rerank` feature was never added to `Cargo.toml` and stays out

## Files Modified

| File | Changes |
|------|---------|
| `src/models.rs` | DocumentTemplate enum + template structs, ConceptualInstance, CoherenceNode.with_template/with_embedder/extract_fragment, NodeView.fragment/instances fields, ConceptHit + rank_concept_hits() (11C) |
| `src/core.rs` | `find_conceptual_instances()` method |
| `src/ingest.rs` | DocKind::OcrDocument/Image/Video, extract_ocr/extract_audio_transcript/extract_spreadsheet/extract_video_metadata, template extraction in push loop |
| `src/operational.rs` | DocKind match updated for new variants |
| `src/web/coherence.rs` | Instance listing endpoint, fragment population in handlers |
| `src/web/search.rs` | `concepts_search_handler` (11C: threshold + salience re-rank + cell grouping) |
| `src/web/mod.rs` | Route registration for instances endpoint + `/api/v1/concepts/search`; `ConceptSearchQuery` type |
| `src/cli.rs` | `run_concepts` / `run_node_instances` / `run_jump` / `run_node_context` (11C) |
| `src/main.rs` | `Concepts` / `NodeInstances` / `Jump` / `Context` CLI subcommands + dispatch + CLI test |
| `tests/http_surface.rs` | `concepts_search_ranks_groups_and_isolates_tenants` (11C) |
| `Cargo.toml` | New features: `ocr`, `audio` (`jepa-rerank` planned but never added — 11D superseded, see above); updated `full` and `ingest` features |

## Feature Flags

| Feature | Dependencies | Purpose |
|---------|-------------|---------|
| `ocr` | `tesseract` | OCR pipeline for scanned documents/images |
| `audio` | (Whisper API) | Audio transcription for mp3/wav/flac |
| `ingest` | `calamine`, `csv`, `quick-xml`, `tesseract` | Spreadsheet parsing + OCR support |

## Tests

All new functionality has tests:
- `test_find_conceptual_instances` — verifies conceptual similarity search
- `test_node_fragment_extraction` — verifies fragment extraction from labels
- `test_classify_new_file_types` — verifies OCR/audio/image/video file type recognition
- `test_invoice_template_fragment` — verifies invoice template + fragment flow
- `test_email_template_fragment` — verifies email template + fragment fallback
- `test_spreadsheet_template` — verifies spreadsheet template
- `test_transcript_template` — verifies transcript template
- `test_image_template` — verifies image template
- `test_video_template` — verifies video template
- `test_generic_template_default` — verifies default template
- `test_coherence_node_with_template` — verifies template integration with CoherenceNode

## Remaining Work

None on the plan as written: 11A + 11B shipped before, 11C ships here, 11D
is superseded (do not build — see above). What remains is verification:
`cargo check --all-targets --features web` is green; still to run once the
build settles: `cargo test --lib models::tests`,
`cargo test --bin physis-pro test_cli_concepts_node_instances_jump_context`,
`cargo test --test http_surface --features web concepts_search`, then the
full gates (`cargo test`, `cargo clippy --all-targets -- -D warnings`,
`cargo fmt -- --check`, `just dev-loop "PLAN 11C conceptual search"`).
