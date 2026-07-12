# physis_pro — CI baseline (established 2026-07-12)

Toolchain:
- cargo / rustc 1.96.0 (installed locally to ~/.cargo on root; previously the
  cargo home was a symlink to an unmounted external drive /media/gio/7d36e554-*/cargo_home)
- clippy component installed

Baseline run (main, before any P10 / accuracy-floor work):
```
cargo check            -> Finished dev profile, 0 errors
cargo clippy --all-targets -> Finished, 0 warnings
cargo test             -> 8 passed, 1 ignored, 0 failed
        - tests/classification.rs: 2 passed, 1 ignored (accuracy_floor needs real model on disk; run with --features full -- --ignored)
        - tests/holarchy.rs: 5 passed
        - tests/smoke_test.rs: 1 passed
```

Notes:
- Dependency registry lives in ~/.cargo/registry (~816M, fetched online; was NOT on the old drive).
- `target/` is NOT committed; `cargo clean` after a run to avoid filling the 91%-full root disk (a full check+clippy+test cycle uses ~7.5G of build artifacts).
- CARGO_NET_OFFLINE must be unset (was true in some shells) or deps fail to resolve.
- `accuracy_floor` test is #[ignore] by design — gated behind a real on-disk model.
