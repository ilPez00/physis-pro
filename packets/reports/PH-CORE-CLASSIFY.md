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

---

## Verification note (2026-09-20, independent reviewer) — ACCEPTED

Reviewer: opencode. Verdict: **accept** (`status: done`). Every claim in this
report was re-measured at the current submodule revision.

### Revisions

| | report time | review time |
|---|---|---|
| `physis-core` | `30cde1a` | `5728a32` (descends from `30cde1a`) |
| parent | `7840593` | `d46ffb4` |

### Reproduction (all rerun)

```
cargo test --lib classify
  test result: ok. 10 passed; 0 failed
  (metadata_classification_keeps_the_input_text among them)

cargo test --lib
  test result: ok. 233 passed; 0 failed

cargo clippy --lib -- -D warnings
  Finished — clean

cargo test --test api_stability          (parent)
  test core_public_api_matches_snapshot ... ok
```

### Claims confirmed

- **The constant is gone.** `classify.rs:488` now stores
  `text: text.to_string()`; the `// TODO` the report cites is not present.
- **The signature is the blessed one.** `classify.rs:468-474` is
  `classify_with_metadata(&self, text: &str, embedding, metadata, filter)`, and
  `tests/snapshots/physis_core_api.txt:155` matches it exactly. The snapshot
  edit the report left uncommitted is committed (`7840593`), so the parent tree
  is consistent.
- **The test asserts both halves.** `metadata_classification_keeps_the_input_text`
  checks `sentence.text == "spindle bearing seizure"` *and* that the cell is
  still the top match `("A", "X")` — so a future change that fixes `text` by
  breaking classification fails here.
- **No production caller was broken.** `grep -rn classify_with_metadata` over
  `src/`, `tests/`, `examples/` and the parent tree finds the definition, the
  snapshot line, and one call — the new inline test at `classify.rs:730`.
  The report's "zero callers" reads as "zero callers before this change"; that
  is true, and the one caller that exists now is the test this packet added.

### On the report's own caveat

The report flags that `text` is not validated against `embedding`. Agreed and
correctly left alone: binding text to a vector is not checkable cheaply, every
other `embedding: &[f32]` entry point in this crate has the same contract, and
adding a check here would invent an invariant the rest of the crate does not
hold. The field is now honest for callers that respect the contract, which is
strictly better than a constant that lied to all of them.
