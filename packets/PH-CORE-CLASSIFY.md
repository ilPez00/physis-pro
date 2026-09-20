---
id: PH-CORE-CLASSIFY
title: ClassifiedSentence.text carries the classified input
status: review
phase: 1
depends: [PH-CORE-CLEANUP]
claimed_by: opencode
claimed_at: 2026-09-20
---

# PH-CORE-CLASSIFY

## Gate

```sh
cd physis-core
cargo test --lib classify            # includes metadata_classification_keeps_the_input_text
cargo test --lib
cargo clippy --lib -- -D warnings
cd .. && cargo test --test api_stability   # snapshot blessed for the added parameter
```

## Execution

Branch `agent/PH-CORE-CLASSIFY` @ `30cde1a`. Report:
`packets/reports/PH-CORE-CLASSIFY.md`.

## Public API

`classify_with_metadata` gains a `text: &str` parameter. Snapshot blessed with
`BLESS=1`; the parent-side `tests/snapshots/physis_core_api.txt` edit is
uncommitted (parent checkout is on `agent/PH-004`).
