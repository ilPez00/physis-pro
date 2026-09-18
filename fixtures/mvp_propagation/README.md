# Retrieval-budget propagation fixture

This tiny repository makes one honest dependency visible: the numeric budget is
defined once in `src/retrieval.rs`, then used by `src/agent.rs` and
`src/context.rs`. This README also names the retrieval module, so it is a
document-shaped dependent of the same change.

Edit `retrieval_budget` in `src/retrieval.rs`, save, and watch Physis mark the
changed file and its code-shaped dependents.
