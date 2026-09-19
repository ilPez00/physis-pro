//! Day 2 of the Core→Pro plan: the observation log survives the adapter move.
//!
//! `physis-core/src/observe.rs` keeps the record, the JSONL format and the
//! reader API; the collection adapters (`watch_fs`, `watch_git`, …) moved to
//! `physis-pro/src/ported/observe_adapters.rs`. A move that compiles is not a
//! move that works, so every test here runs the shipped `physis-pro-world`
//! binary against a sandboxed `PHYSIS_CORE_DIR` and `HOME`, and asserts the
//! four things Day 2 accepts:
//!
//! 1. a legacy log keeps its IDs, order and provenance across the move;
//! 2. a retry appends nothing and duplicates nothing;
//! 3. an interrupted append does not skip the next record;
//! 4. a copied log answers identically after a restart.
//!
//! Nothing here touches the operator's store, log or shell history.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

/// The `physis-pro-world` binary — Core's `physis-world`, moved — with both the
/// store and `HOME` pointed at private dirs. `HOME` matters: `observe` reads
/// `$HOME/.zsh_history`, and the operator's history is not a test fixture.
fn world(sandbox: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_physis-pro-world"))
        .env("PHYSIS_CORE_DIR", sandbox)
        .env("HOME", home)
        .args(args)
        .output()
        .expect("physis-pro-world runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// The integer following `marker` in `text`, e.g. `count_after("world: ", " observations")`.
fn count_after(text: &str, marker: &str, end: &str) -> u64 {
    let rest = text
        .split(marker)
        .nth(1)
        .unwrap_or_else(|| panic!("`{marker}` not found in:\n{text}"));
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    assert!(!digits.is_empty(), "no number after `{marker}` in:\n{text}");
    let _ = end;
    digits.parse().unwrap()
}

/// A log exactly as a pre-move `physis-core` wrote it: three complete records
/// carrying their own sequence numbers, times and `produced_by` provenance,
/// followed by a truncated final line — the shape a killed process leaves.
fn legacy_log(sandbox: &Path) -> std::path::PathBuf {
    let p = sandbox.join("observations.jsonl");
    fs::write(
        &p,
        concat!(
            "{\"seq\":1,\"at\":\"2026-01-01T08:00:00Z\",\"source\":\"fs\",\"subject\":\"/srv/pump.md\",\"body\":\"the pump needs calibration\",\"produced_by\":\"watch_fs\"}\n",
            "{\"seq\":2,\"at\":\"2026-01-02T09:30:00Z\",\"source\":\"git\",\"subject\":\"deadbeef\",\"body\":\"commit: recalibrate the pump\",\"produced_by\":\"watch_git\"}\n",
            "{\"seq\":3,\"at\":\"2026-01-03T10:00:00Z\",\"source\":\"terminal\",\"subject\":\"cargo test pump\",\"body\":\"\",\"produced_by\":\"watch_shell\"}\n",
            "{\"seq\":4,\"at\":\"2026-01-04T11:00:00Z\",\"source\":\"fs\",\"subject\":\"/srv/valve.md\",\"body\":\"valve replaced\"",
        ),
    )
    .unwrap();
    p
}

/// A tree with one recognisable file, so `observe` has something to collect.
fn fixture_tree() -> TempDir {
    let root = TempDir::new().unwrap();
    fs::write(
        root.path().join("pump.md"),
        "pump calibration record\npressure 4.2 bar\n",
    )
    .unwrap();
    fs::write(root.path().join("valve.md"), "valve replaced on line 3\n").unwrap();
    root
}

/// Day 2 acceptance (legacy state): the records an old Core wrote must read
/// back with the same sequence numbers, the same order and the same provenance.
/// The truncated final line is dropped, by design — one lost record, not a lost
/// log — but the three complete ones are untouched.
#[test]
fn legacy_ids_order_and_provenance_survive_the_move() {
    let sandbox = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    legacy_log(sandbox.path());

    let inspected = world(sandbox.path(), home.path(), &["inspect"]);
    assert!(inspected.status.success(), "stderr: {}", stderr(&inspected));
    let text = stdout(&inspected);
    assert!(
        text.contains("world: 3 observations"),
        "the truncated line must cost one record, not the log: {text}"
    );

    // IDs, order and provenance, read back position by position.
    let first = stdout(&world(sandbox.path(), home.path(), &["trace", "0"]));
    assert!(first.contains("seq 1 · fs"), "wrong first record: {first}");
    assert!(first.contains("by watch_fs"), "provenance lost: {first}");
    assert!(
        first.contains("2026-01-01 08:00"),
        "the recorded time must survive: {first}"
    );
    assert!(first.contains("/srv/pump.md"), "subject lost: {first}");

    let third = stdout(&world(sandbox.path(), home.path(), &["trace", "2"]));
    assert!(third.contains("seq 3 · terminal"), "order changed: {third}");
    assert!(third.contains("by watch_shell"), "provenance lost: {third}");
}

/// Day 2 acceptance (retries): running the same collection twice must not
/// append a second copy of anything.
#[test]
fn a_retry_appends_nothing_and_duplicates_nothing() {
    let sandbox = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let root = fixture_tree();
    let log = sandbox.path().join("observations.jsonl");

    let first = world(sandbox.path(), home.path(), &["observe", "--root", root.path().to_str().unwrap()]);
    assert!(first.status.success(), "stderr: {}", stderr(&first));
    let after_first = fs::read_to_string(&log).unwrap();
    assert!(after_first.lines().count() >= 2, "nothing was observed: {after_first}");

    let second = world(sandbox.path(), home.path(), &["observe", "--root", root.path().to_str().unwrap()]);
    assert!(second.status.success(), "stderr: {}", stderr(&second));
    assert!(
        stdout(&second).contains("nothing new"),
        "a retry must be a no-op: {}",
        stdout(&second)
    );
    assert_eq!(
        after_first,
        fs::read_to_string(&log).unwrap(),
        "the log changed on a retry"
    );
}

/// Day 2 acceptance (interruption): a process killed mid-write leaves a
/// truncated final line. The next `observe` must still land its first record —
/// before the tail repair it was concatenated onto the fragment and both were
/// dropped, which is a silently skipped record.
#[test]
fn an_interrupted_append_does_not_skip_the_next_record() {
    let sandbox = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let root = fixture_tree();
    legacy_log(sandbox.path()); // 3 complete records + 1 truncated tail

    let observed = world(sandbox.path(), home.path(), &["observe", "--root", root.path().to_str().unwrap()]);
    assert!(observed.status.success(), "stderr: {}", stderr(&observed));

    // What the writer says the log holds must equal what a fresh reader sees.
    let claimed = count_after(&stdout(&observed), "log now holds ", " ");
    let inspected = stdout(&world(sandbox.path(), home.path(), &["inspect"]));
    let seen = count_after(&inspected, "world: ", " observations");
    assert_eq!(
        claimed, seen,
        "the writer and the reader disagree — a record was swallowed:\n{inspected}"
    );

    // The new records are really there, and the sequence has no gap after the
    // dropped fragment: the first new record takes the fragment's number.
    let history = stdout(&world(sandbox.path(), home.path(), &["history", "valve"]));
    assert!(history.contains("valve"), "the new record vanished: {history}");
}

/// Day 2 acceptance (migration + restart): a copied log answers identically to
/// the original, and a second process reading the same log agrees with the
/// first. The default representation is the counted word table, so there is no
/// model and no clock in the answer — parity is exact.
#[test]
fn a_copied_log_answers_identically_after_restart() {
    let sandbox = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let root = fixture_tree();
    let observed = world(sandbox.path(), home.path(), &["observe", "--root", root.path().to_str().unwrap()]);
    assert!(observed.status.success(), "stderr: {}", stderr(&observed));

    let locate_before = stdout(&world(sandbox.path(), home.path(), &["locate", "pump"]));
    let inspect_before = stdout(&world(sandbox.path(), home.path(), &["inspect"]));
    assert!(locate_before.contains("pump"), "the fixture was not observed: {locate_before}");

    // Restart: a second process over the same store.
    assert_eq!(
        locate_before,
        stdout(&world(sandbox.path(), home.path(), &["locate", "pump"])),
        "restart changed the answer"
    );

    // Migration: the same log copied to a fresh location answers the same.
    let copy = TempDir::new().unwrap();
    fs::copy(
        sandbox.path().join("observations.jsonl"),
        copy.path().join("observations.jsonl"),
    )
    .unwrap();
    assert_eq!(
        locate_before,
        stdout(&world(copy.path(), home.path(), &["locate", "pump"])),
        "a copied log answered differently"
    );
    assert_eq!(
        inspect_before,
        stdout(&world(copy.path(), home.path(), &["inspect"])),
        "a copied log inspected differently"
    );
}
