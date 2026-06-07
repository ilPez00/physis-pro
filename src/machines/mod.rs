//! Philosophical machines — thin reducers over the physis core.
//!
//! Each machine turns a modality into *primitives* (the smallest elaborable
//! unit), registers them as labeled coherence nodes, and lets the core handle
//! recall / coherence / dream / durability. See `docs/machines/`.
//!
//! Shipped: `chronos` (temporal), `techne` (codebase), `soma` (body),
//! `elenchus` (dialogue), `lex` (contracts), `harmony` (music), `polis` (social).

pub mod chronos;
pub mod techne;
pub mod soma;
pub mod elenchus;
pub mod lex;
pub mod harmony;
pub mod polis;
