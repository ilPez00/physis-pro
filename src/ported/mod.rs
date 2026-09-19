//! Product surfaces moved out of `physis-core` (2026-09-14).
//!
//! These modules were relocated here by the Core/Product split so that the
//! public Core stays an austere research engine. They are the same
//! implementations — moved, not forked — and they build on the public Core
//! API (`physis_core::…`).
//!
//! Provenance: each file carries a header naming its source path and the Core
//! HEAD it was moved from (`d65858b`). See `physis-core/MOVED_TO_PRODUCT.md`.

pub mod act;
pub mod act_recall;
pub mod config_run;
pub mod edition;
pub mod front_door;
pub mod ground;
pub mod history;
pub mod nlq;
pub mod notebook;
pub mod observe_adapters;
pub mod oracle;
pub mod praxis;
pub mod quality;
// The studio surfaces are an axum web application (`feature = "web"`), exactly
// as they were behind Core's `studio` feature. Gating them here keeps
// `--no-default-features` builds honest instead of failing on a missing `axum`.
#[cfg(feature = "web")]
pub mod studio;
#[cfg(feature = "web")]
pub mod studio_communities;
#[cfg(feature = "web")]
pub mod studio_lab;
pub mod system;
// `system_cli` is the clap surface for the workspace service, so it needs the
// `cli` feature exactly as the binary does. Ungated it broke every build that
// turned `cli` off.
#[cfg(feature = "cli")]
pub mod system_cli;
pub mod system_delegation;
pub mod system_mcp;
// The TUI navigator needs ratatui + crossterm, which are optional (`tui`
// feature). Ungated, this module made every no-default-features build fail.
#[cfg(feature = "tui")]
pub mod tui;
pub mod vault;
pub mod world;
