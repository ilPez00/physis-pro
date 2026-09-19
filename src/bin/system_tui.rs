//! Thin binary shim for `physis-pro-system-tui`.
//!
//! The navigator itself lives in `physis_pro::ported::tui` (moved from Core's
//! `src/bin/system_tui.rs` by the 2026-09-14 split, which is why the whole
//! `main()` was already there, private and unreachable). This shim is the
//! shipped entry point, matching `src/bin/physis.rs`'s one-line pattern rather
//! than compiling `tui.rs` itself as a second crate root.
//!
//! Requires the `tui` feature (ratatui + crossterm). No licence gate, for the
//! same reason as `physis-pro-world`: Core's `physis-system-tui` had none.

fn main() -> anyhow::Result<()> {
    physis_pro::ported::tui::main()
}
