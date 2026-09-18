//! Philosophical machines — thin reducers over the physis core.
//!
//! Each machine turns a modality into *primitives* (the smallest elaborable
//! unit), registers them as labeled coherence nodes, and lets the core handle
//! recall / coherence / dream / durability. See `docs/machines/`.
//!
//! Shipped: `chronos` (temporal), `topos` (place), `techne` (codebase),
//! `soma` (body), `elenchus` (dialogue), `lex` (contracts), `harmony` (music),
//! `polis` (social), `empedocles` (physical telemetry → elements),
//! `euclid` (geometry), and the relational pair `erosp` / `erosa` (desire).
//!
//! `apodeixis` is the demonstrative-proof machine: it reduces a proof obligation
//! (a claim plus a Lean 4 formalization) into a machine-checked verdict —
//! `certified` / `refuted` / `untestable` — by running the snippet through the
//! Lean kernel. It is the deductive complement to the rest of the engine:
//! elenchus scores what a claim *deserves*, apodeixis settles what it *is*.
//!
//! `erosp` and `erosa` are twin reducers over the same perception (a directed
//! reach), inverse in genealogy: `erosp` is the Platonic eros, child of Penia
//! (lack) and Poros (resource) — the ascending longing; `erosa` is the mythic
//! eros, child of Ares (strife) and Aphrodite (love) — possessive, agonistic
//! conquest. `polis` reduces the *standing* tie; the eros pair reduces the
//! *directed pull* and its two fates (ascent vs seizure).
//!
//! The standalone Python machines (github.com/ilPez00/physis-*) own the
//! perception layer (camera / mic / psutil / parser); these modules are the thin
//! reducers over the core. `physis-logos`→[`elenchus`], `physis-nous`→[`techne`],
//! `physis-pythagoras`→[`harmony`] map onto existing reducers (see their notes).

pub mod apodeixis;
pub mod chroma;
pub mod chronos;
pub mod demos;
pub mod elenchus;
pub mod empedocles;
pub mod erosa;
pub mod erosp;
pub mod euclid;
pub mod harmony;
pub mod kinesis;
pub mod krisis;
pub mod lex;
pub mod pathos;
pub mod plutos;
pub mod polis;
pub mod run;
pub mod soma;
pub mod techne;
pub mod topos;
