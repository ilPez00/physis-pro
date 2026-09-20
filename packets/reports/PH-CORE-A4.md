# PH-CORE-A4 — hash-chained provenance

**Status:** implementation done, awaiting independent review.
**Branch/commit:** `physis-core` `agent/PH-CORE-A4` @ `54b8a8d`.

## Task

PLANNING.md §7.1 A4: "content hash + previous-link hash per link; verify reports
the first break with sequence coordinates. Gate:
`ledger_tamper_breaks_chain_at_sequence`."

## What changed

`src/provenance.rs`
- `ProvenanceLink` gains `content_hash: Option<String>` and
  `prev_hash: Option<String>`, both `#[serde(default)]` so stored chains written
  before A4 still deserialize.
- `content_digest()` — deterministic SHA-256 over claim, source, raw_reference,
  method, intake_id, RFC-3339 timestamp and confidence bits. The hash fields are
  excluded, so the digest is stable and a tampered field is detectable.
- `ProvenanceChain::add_link` seals on append (sets `prev_hash` from the last
  link, then the digest), so API-built chains are hash chains by construction.
- `ProvenanceChain::seal()` recomputes a chain built by pushing into `links`
  directly, or re-seals a deliberate edit.
- `ProvenanceChain::verify() -> ChainIntegrity`:
  - `Sealed` — every link hashes and back-links correctly;
  - `Unsealed { first }` — first link with no hash (legacy/never sealed);
  - `Broken { first }` — first mismatch, with its sequence coordinate.
- `ChainIntegrity` exported from `lib.rs`.

`tests/epistemic_p2.rs`
- Gate test `ledger_tamper_breaks_chain_at_sequence`: sealed chain verifies; a
  middle-field edit reports `Broken { first: 1 }`; re-seal restores `Sealed`; a
  first-link edit reports `Broken { first: 0 }`; a legacy hashless chain reports
  `Unsealed { first: 0 }` rather than a false tamper.

## Verification (native)

```
cargo test --test epistemic_p2
  test result: ok. 5 passed; 0 failed   (incl. ledger_tamper_breaks_chain_at_sequence)
cargo test --lib
  test result: ok. 233 passed; 0 failed
cargo clippy --lib --tests -- -D warnings
  Finished — clean
cargo check --all-targets
  ok (pre-existing example warnings only)
cargo test --test api_stability   (from parent)
  ok   (after BLESS; all drift additive)
```

### Public API drift (blessed)

New: `ChainIntegrity` (+ 3 variants), `ProvenanceLink::{content_hash, prev_hash}`
fields, `ProvenanceChain::{seal, verify}`, `add_link` takes `mut link`. No
existing field or method removed. Blessed with `BLESS=1`.

## What I could not verify

- **Independent review.** Single agent; `status: review`, not `done`.
- **`clippy --all-targets -D warnings` is red on pre-existing example dead code**
  (`examples/mapper_corpus_export.rs`, `examples/perspective_invention_experiment.rs`),
  unrelated to this change. The enforced lib+tests scope is clean.
- **Re-seal attack is out of scope by design.** An attacker who edits a link and
  every hash after it produces a chain that verifies. Detecting that needs an
  external anchor (a signed chain head); `verify()` documents this limitation.
- **`explanation.rs` does not yet surface integrity.** PLANNING §7.1 A4 names
  `explanation.rs` (`provenance_chain`, `human_readable_summary`) as part of the
  item; the chain can now be verified, but no report field exposes
  `ChainIntegrity` yet. Follow-up packet.
