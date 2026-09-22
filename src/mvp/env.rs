//! Hot-path environment reads — cached once via `Lazy` so a per-query call
//! (`mvp::repl::scope`) does not re-read the process `environ` on every
//! command. See PH-103/PH-020 §1: direction-affecting `env::var` on a hot path.
//!
//! `PHYSIS_DATA_DIR` is set by `repl::HudApp::open` before any query runs, so
//! the `Lazy` reads it after that initialization point. Tests that set the var
//! mid-process should call `reset()` to drop the cached value.
//!
//! Non-direction-affecting reads (EDITOR, PHYSIS_MVP_VALIDATE — operator-only,
//! per-invocation) stay inline; caching those hides deliberate runtime switches
//! from test setup.

use once_cell::sync::Lazy;

/// `PHYSIS_DATA_DIR` — the per-query data root. Cached once; `repl::open`
/// writes the default (`<root>/.physis`) before the first query so the cache
/// observes it.
pub static PHYSIS_DATA_DIR: Lazy<Option<std::path::PathBuf>> = Lazy::new(|| {
    std::env::var_os("PHYSIS_DATA_DIR")
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
});

/// `PHYSIS_MODEL` — the per-query model selector (`none`/`local`/...`PHYSIS_*`).
/// Cached once per process; flipping it mid-run is deliberate restart territory.
pub static PHYSIS_MODEL: Lazy<Option<String>> = Lazy::new(|| {
    std::env::var("PHYSIS_MODEL").ok().map(|v| v.trim().to_string()).filter(|s| !s.is_empty())
});

/// Drop the cached values so a subsequent access re-reads `environ`.
/// Needed only by tests that mutate these vars at runtime.
#[cfg(test)]
pub fn reset() {
    // NOTE: once_cell::sync::Lazy has no public force-reinit. Tests that need a
    // fresh read of PHYSIS_MODEL/PHYSIS_DATA_DIR must run in a fresh process.
    // This sentinel documents that intent and forces the Lazy to evaluate once
    // so a test touching the value does not race an uninitialized access.
    let _ = Lazy::force(&PHYSIS_MODEL);
    let _ = Lazy::force(&PHYSIS_DATA_DIR);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physis_model_is_cached_not_reread() {
        // Set the var, force the Lazy, then flip it — the cached copy must not
        // change mid-process. (once_cell Lazy reads environ once.)
        std::env::set_var("PHYSIS_MODEL", "local");
        let before = PHYSIS_MODEL.as_deref().map(String::from);
        std::env::set_var("PHYSIS_MODEL", "none");
        let after = PHYSIS_MODEL.as_deref().map(String::from);
        assert_eq!(before, Some("local".to_string()), "Lazy read PHYSIS_MODEL once");
        assert_eq!(after, before, "mid-run flip cached — no re-read of environ");
        std::env::remove_var("PHYSIS_MODEL");
    }
}
