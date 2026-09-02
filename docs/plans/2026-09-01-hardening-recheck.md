# Hardening Re-check Plan — H1-H3 + Future Audits

Source: `PLAN.md:474` Hardening Backlog (external evaluation 2026-08-31) — this closes H1-H3, plans the re-check loop, then T3.

## Closed: H1-H3

| ID | Fix | File | Gate |
|----|-----|------|------|
| H1 | Dev licence gate fail-closed | `src/license.rs:53` `IS_DEV_BUILD` + `allow_dev_licence()`; `vendor_key()` returns `DevelopmentBuildRequiresAllowFlag` unless `PHYSIS_ALLOW_DEV_LICENCE=1` (exempt `cfg(test)`); `enforce()` prints dev banner and refuses start | `cargo test` with dev key fails unless flag set; release builds stamped with `PHYSIS_LICENSE_PUBKEY` bypass |
| H2 | Semantic self-test discriminates | `src/embed/mod.rs:77` 4 disjoint synonym pairs + 0.12 margin; bag-of-words hash fails, trained model passes | `semantic_self_test(&RandomProjectionEmbedder)` == false; health reports `degraded:true` |
| H3 | No silent downgrade | `src/embed/mod.rs:426` `model_dir()` checks `PHYSIS_MODEL_DIR` + exe dir + parents before cwd ; `is_silent_downgrade()` + `src/cli.rs:172` bail unless `PHYSIS_EMBEDDER=random-projection` | `PHYSIS_EMBEDDER=random-projection` works; missing model without flag errors |

H3 web path already aborts via `src/web/mod.rs:2356` `MODEL_BASE_URL` gate — H2 now trips it correctly.

## Future Audit Cadence

- **Quarterly external evaluation** against built binaries (licence, embedder, classification, pack, HTTP, CI). Owner: external reviewer, not author.
- **Post-major-feature gate**: any change touching `src/license.rs`, `src/embed/*`, `src/config.rs::OnnxConfig`, `src/pack.rs` or `src/web/mod.rs` degraded logic requires a re-run of the 3 H1-H3 gates before merge.
- **CI**: add jobs (or extend `ci.yml`):
  - `dev-licence-gate` — `cargo build` without `PHYSIS_LICENSE_PUBKEY`, run `physis-pro --version` and assert exit 2 without `PHYSIS_ALLOW_DEV_LICENCE=1`, exit 0 with it.
  - `self-test-discriminates` — `cargo test embed::semantic_self_test` with RP vs bge-base fixtures.
  - `model-dir` — run `physis-pro classify` from `/tmp` with model only in `data_dir/models` and `PHYSIS_MODEL_DIR` unset vs set.

## Re-hardening Backlog (H4-H11) — sequenced after H1-H3

Per `PLAN.md:674` sequencing: H1-H3 block → accuracy block H6+H4 gates pilot → H7 gates agent sale → hygiene H5/H8-H10. Do not start H4 until H2 is green in CI.

Next: `T3` typed-object round-trip (persist/edit/delete) verifies I5 back-compat — pure unit + e2e, no embedder dep.
