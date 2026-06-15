//! Philosophical machines — thin reducers over the physis core.
//!
//! Each machine turns a modality into *primitives* (the smallest elaborable
//! unit), registers them as labeled coherence nodes, and lets the core handle
//! recall / coherence / dream / durability. See `docs/machines/`.
//!
//! Shipped: `chronos` (temporal), `techne` (codebase), `soma` (body),
//! `elenchus` (dialogue), `lex` (contracts), `harmony` (music), `polis` (social),
//! `empedocles` (physical telemetry → elements), `euclid` (geometry).
//!
//! The standalone Python machines (github.com/ilPez00/physis-*) own the
//! perception layer (camera / mic / psutil / parser); these modules are the thin
//! reducers over the core. `physis-logos`→[`elenchus`], `physis-nous`→[`techne`],
//! `physis-pythagoras`→[`harmony`] map onto existing reducers (see their notes).

pub mod chronos;
pub mod topos;
pub mod techne;
pub mod soma;
pub mod elenchus;
pub mod lex;
pub mod harmony;
pub mod polis;
pub mod empedocles;
pub mod euclid;
pub mod erosp;
pub mod erosa;
