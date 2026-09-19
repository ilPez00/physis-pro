//! Thin binary shim for `physis-pro-world`.
//!
//! The navigator itself lives in `physis_pro::ported::world` (moved from Core's
//! `src/bin/world.rs` by the 2026-09-14 split, which is why the whole `main()`
//! was already there, private and unreachable). This shim is the shipped entry
//! point, matching `src/bin/physis.rs`'s one-line pattern rather than compiling
//! `world.rs` itself as a second crate root.
//!
//! No licence gate: Core's `physis-world` had none, and the migration contract
//! is "moved, not forked" — a gate added here would be a new product decision,
//! not a relocation, and is called out as such in the Day-1 handoff.

fn main() -> anyhow::Result<()> {
    physis_pro::ported::world::main()
}
