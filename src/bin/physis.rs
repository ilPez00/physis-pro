//! Thin binary shim for the shared `physis` front door.
//!
//! The actual logic lives in `physis_pro::ported::front_door` (moved from
//! Core's multi-edition front door by the 2026-09-14 split, wired back in as
//! the one shipped `physis` executable once Core dropped its own `[[bin]]
//! name = "physis"` in favour of this one). Kept as a one-line `src/bin/`
//! file, matching every other Pro binary shim, rather than compiling
//! `front_door.rs` itself as a second crate root.

fn main() {
    physis_pro::ported::front_door::main();
}
