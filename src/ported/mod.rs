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
pub mod studio;
pub mod studio_communities;
pub mod studio_lab;
pub mod system;
pub mod system_cli;
pub mod system_delegation;
pub mod system_mcp;
// The relocated TUI is the one ported module that needs an optional
// dependency pair (`ratatui`/`crossterm`, behind the `tui` feature). Without
// this gate the whole crate failed to compile for the deploy feature set
// (`web,cli,embed-onnx`), which is how the pansa auto-updater builds it.
#[cfg(feature = "tui")]
pub mod tui;
pub mod vault;
pub mod world;
