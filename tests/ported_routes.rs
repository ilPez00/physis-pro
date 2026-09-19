//! Routes the 2026-09-14 Core→Pro split moved, exercised through the binaries
//! this package actually builds.
//!
//! Why a child process and not a library call: after the split, a moved module
//! that compiles but is reachable from nowhere is indistinguishable from one
//! that works. Every test below starts the shipped executable against a
//! sandboxed `PHYSIS_CORE_DIR`, so the claim under test is "a user can run
//! this", and none of them touch the operator's real store or observation log.
//!
//! Scope note: a route counts as delivered only when the *product* binary
//! serves it — the "an old target binary is not evidence" rule the plan states,
//! applied to the moved verbs (`ground`, `act-recall`, `nlq`, `run`) and the
//! two moved binaries (`physis-pro-world`, `physis-pro-system-tui`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

/// The `physis-pro` binary, with `PHYSIS_CORE_DIR` pointed at a private dir so a
/// test writes nothing the operator would notice.
fn pro(sandbox: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_physis-pro"))
        .env("PHYSIS_CORE_DIR", sandbox)
        .env_remove("PHYSIS_MODEL_DIR")
        .args(args)
        .output()
        .expect("physis-pro runs")
}

/// The `physis-pro-world` binary — Core's `physis-world`, moved.
fn world(sandbox: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_physis-pro-world"))
        .env("PHYSIS_CORE_DIR", sandbox)
        .args(args)
        .output()
        .expect("physis-pro-world runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "invalid JSON: {e}\nstdout: {}\nstderr: {}",
            stdout(output),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

/// `ground` reads the store and the observation log and renders the shared
/// state. Empty is a valid answer, and it must be an answer rather than an
/// error: "nothing has been asserted" is information.
#[test]
fn ground_renders_the_shared_state_from_an_empty_sandbox() {
    let sandbox = TempDir::new().unwrap();
    let out = pro(sandbox.path(), &["ground"]);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let text = stdout(&out);
    assert!(text.contains("COMMON GROUND"), "not the ground view: {text}");
    assert!(
        text.contains("No claims yet"),
        "an empty ledger must say so: {text}"
    );
}

/// The same view, machine-readable — the shape `ground --json` promises.
#[test]
fn ground_json_has_the_documented_fields() {
    let sandbox = TempDir::new().unwrap();
    let out = pro(sandbox.path(), &["ground", "--json"]);
    assert!(out.status.success());
    let v = json(&out);
    for key in ["claims", "disagreements", "unscored", "events", "nodes"] {
        assert!(v.get(key).is_some(), "ground JSON is missing `{key}`: {v}");
    }
}

/// `nlq` proposes a command, and the command it prints has to name a binary
/// that exists in this repository. It said `physis-world` after the move until
/// this assertion was fixed, which is exactly the kind of drift a moved string
/// carries.
#[test]
fn nlq_proposes_a_command_that_names_the_shipped_binary() {
    let sandbox = TempDir::new().unwrap();
    let out = pro(sandbox.path(), &["nlq", "where is the pump"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(
        text.contains("physis-pro-world locate pump"),
        "proposal must be pasteable: {text}"
    );

    let v = json(&pro(sandbox.path(), &["nlq", "--json", "where is the pump"]));
    assert_eq!(v["intent"], "Locate");
    assert_eq!(v["term"], "pump");
    assert_eq!(v["fallback"], false);
}

/// A question with no cue must declare itself a fallback instead of pretending
/// the default was inferred.
#[test]
fn nlq_declares_an_unrecognised_question_as_a_fallback() {
    let sandbox = TempDir::new().unwrap();
    let out = pro(sandbox.path(), &["nlq", "zzz qqq"]);
    assert!(out.status.success());
    let v = json(&pro(sandbox.path(), &["nlq", "--json", "zzz qqq"]));
    assert_eq!(v["fallback"], true, "no cue fired, so this is a default: {v}");
    assert!(stdout(&out).contains("not an inference"), "{}", stdout(&out));
}

/// `act-recall` is a measurement, so it must report the embedder that produced
/// it and write the artifact Core's verb wrote — at the same path, relative to
/// the working directory, because a measurement that lands somewhere new is not
/// the same measurement.
#[test]
fn act_recall_reports_its_embedder_and_writes_its_artifact() {
    let sandbox = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_physis-pro"))
        .env("PHYSIS_CORE_DIR", sandbox.path())
        .current_dir(cwd.path())
        .args(["act-recall", "--json"])
        .output()
        .expect("act-recall runs");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let v = json(&out);
    assert!(
        v["embedder"].is_string(),
        "a recall number without its embedder is not a recall claim: {v}"
    );
    assert!(v["arms"].is_array(), "both arms are measured in one pass: {v}");
    assert!(
        v["arms"][0]["null_uniform_recall"].is_number(),
        "each arm carries its own null: {v}"
    );
    let artifact = cwd.path().join("benchmarks/results/act-recall.json");
    assert!(artifact.is_file(), "artifact missing at {}", artifact.display());
}

/// `physis-pro-world` — Core's `physis-world`, moved — must run from this
/// package and must not write to the tree it is pointed at. `observe` only
/// appends to its own log; that is the invariant the module's docs promise.
#[test]
fn world_observes_without_touching_the_tree() {
    let sandbox = TempDir::new().unwrap();
    let root = TempDir::new().unwrap();
    let file = root.path().join("pump.md");
    fs::write(&file, "pump calibration record\n").unwrap();
    let before = fs::read_to_string(&file).unwrap();

    let observed = world(sandbox.path(), &["observe", "--root", root.path().to_str().unwrap()]);
    assert!(observed.status.success(), "stderr: {}", String::from_utf8_lossy(&observed.stderr));
    assert!(
        stdout(&observed).contains("append-only"),
        "observe must say what it did not do: {}",
        stdout(&observed)
    );
    assert_eq!(before, fs::read_to_string(&file).unwrap(), "the tree was written");

    let inspected = world(sandbox.path(), &["inspect"]);
    assert!(inspected.status.success());
    assert!(
        stdout(&inspected).contains("observations"),
        "inspect must report the log: {}",
        stdout(&inspected)
    );
    assert!(
        sandbox.path().join("observations.jsonl").is_file(),
        "the observation log belongs in PHYSIS_CORE_DIR, not the tree"
    );
}

/// The help text is the moved binary's own, not a stub's: a user has to be able
/// to see the navigational verbs before running one.
#[test]
fn world_help_lists_the_navigational_verbs() {
    let sandbox = TempDir::new().unwrap();
    let out = world(sandbox.path(), &["--help"]);
    assert!(out.status.success());
    let text = stdout(&out);
    for verb in ["locate", "neighbors", "history", "trace", "retrieve"] {
        assert!(text.contains(verb), "help omits `{verb}`: {text}");
    }
}
/// `physis-pro-system-tui` — Core's `physis-system-tui`, moved. `--help` is the
/// only assertion that can run without a terminal; it exits 0 and prints the
/// key map rather than failing on a missing TTY.
#[test]
fn system_tui_answers_help_without_a_terminal() {
    let out = Command::new(env!("CARGO_BIN_EXE_physis-pro-system-tui"))
        .arg("--help")
        .output()
        .expect("physis-pro-system-tui runs");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let text = stdout(&out);
    assert!(
        text.contains("workspace interface"),
        "not the moved TUI's help: {text}"
    );
}

/// `run` reads a JSON config and drives Core's pipeline. This is the route that
/// makes `ported::config_run` the code path rather than a compiled file, so the
/// assertion is the run banner plus the config it echoed back — the corpus and
/// query have to be the ones in the file, not defaults.
///
/// Slow by nature: the pipeline loads the cascade-selected embedder. Kept here
/// anyway, because skipping it would leave the moved config loader unproven
/// from the product side.
#[test]
fn run_drives_the_pipeline_from_a_json_config() {
    let sandbox = TempDir::new().unwrap();
    let work = TempDir::new().unwrap();
    let corpus = work.path().join("corpus");
    fs::create_dir(&corpus).unwrap();
    fs::write(
        corpus.join("a.md"),
        "the pump needs calibration\nthe pump runs at 40hz\n",
    )
    .unwrap();
    let config = work.path().join("run.json");
    fs::write(
        &config,
        serde_json::to_vec_pretty(&serde_json::json!({
            "corpus": corpus,
            "query": "the pump",
            "budget": 400,
            "model": null,
            "ngram": { "registry_id": null, "order": 3 }
        }))
        .unwrap(),
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_physis-pro"))
        .env("PHYSIS_CORE_DIR", sandbox.path())
        .args(["run", "--config"])
        .arg(&config)
        .output()
        .expect("run executes");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let text = stdout(&out);
    assert!(
        text.contains("PHYSIS RUN (config-driven"),
        "not the run banner: {text}"
    );
    assert!(
        text.contains("query: the pump"),
        "the config's query must be the one run: {text}"
    );
}

/// A missing config is a hard error that names the file — the contract
/// `ported::config_run::load_config` documents, checked through the CLI.
#[test]
fn run_reports_a_missing_config_by_path() {
    let sandbox = TempDir::new().unwrap();
    let missing: PathBuf = sandbox.path().join("nope.json");
    let out = pro(sandbox.path(), &["run", "--config", missing.to_str().unwrap()]);
    assert!(!out.status.success(), "a missing config must not exit 0");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("nope.json"), "error must name the file: {err}");
}
