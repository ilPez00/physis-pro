# PH-CORE-CLASSIFY — `ClassifiedSentence.text` carries the input

**Status:** implementation done, awaiting independent review.
**Branch/commit:** `physis-core` `agent/PH-CORE-CLASSIFY` @ `30cde1a`.

## Task

`src/classify.rs` `CellClassifier::classify_with_metadata` returned
`ClassifiedSentence { text: "classified text".to_string(), // TODO }`. The field
is documented as "The original text" and is `pub`; any caller that stored the
sentence recorded a constant instead of what it classified.

## What changed

- `classify_with_metadata` gains a `text: &str` parameter and stores it in
  `ClassifiedSentence::text`.
- New unit test `metadata_classification_keeps_the_input_text` asserts the
  returned `text` equals the input and that the cell is still the top match.

The function had **zero callers** in `src/`, `examples/` and `tests/`, so the
signature change breaks nothing in-tree.

## Verification (native)

```
cargo test --lib classify
  test result: ok. 10 passed; 0 failed
  (includes metadata_classification_keeps_the_input_text)
cargo test --lib
  test result: ok. 233 passed; 0 failed
cargo clippy --lib -- -D warnings
  Finished — clean
cargo test --test api_stability   (from parent)
  ok   (after BLESS)
```

### Public API drift (blessed)

`tests/snapshots/physis_core_api.txt` (parent repo) changed exactly one line:

```
- classify_with_metadata( &self, embedding, metadata, filter ) -> Option<ClassifiedSentence>
+ classify_with_metadata( &self, text: &str, embedding, metadata, filter ) -> Option<ClassifiedSentence>
```

Blessed with `BLESS=1 cargo test --test api_stability`. This is the documented
mechanism for a deliberate API change; the parent working tree now carries the
snapshot edit (uncommitted — parent is on another agent's branch `agent/PH-004`).

## What I could not verify

- **Independent review.** Single agent; `status: review`, not `done`.
- **The `text` parameter is not validated against the embedding.** A caller can
  now pass text that does not correspond to `embedding`. This is the caller's
  contract (as with every `embedding: &[f32]` entry point here); no check was
  added because there is no cheap way to bind text to a vector.
- **No downstream caller exists yet**, so the value of the field is only proven
  by the unit test, not by an integration path.
