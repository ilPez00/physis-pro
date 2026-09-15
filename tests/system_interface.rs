//! Exercise the public CLI/service boundary, including real child processes.
use std::fs;
use std::process::{Command, Output};

use physis_pro::ported::observe_adapters::watch_fs;
use physis_pro::ported::system::Workspace;
use serde_json::Value;
use tempfile::TempDir;

fn workspace() -> (TempDir, Workspace) {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("project");
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("pump.rs"),
        "// Pump calibration records expected pressure and observed pressure.\n",
    )
    .unwrap();
    fs::write(
        root.join("notes.md"),
        "A notebook of unrelated calendar appointments.\n",
    )
    .unwrap();
    let state = root.join(".physis/system");
    let ws = Workspace::open(&root, &state).unwrap();
    (temp, ws)
}

fn cli(ws: &Workspace, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_physis-pro"))
        .env_remove("PHYSIS_CORE_DIR")
        .args(["system", "--json", "--root"])
        .arg(&ws.root)
        .args(args)
        .output()
        .unwrap()
}

/// `cli`, for arguments that must keep their own `--` separated tail.
fn cli_with(ws: &Workspace, args: &[&str]) -> Output {
    cli(ws, args)
}

/// Cargo only defines `CARGO_BIN_EXE_<name>` at compile time for binaries of
/// *this* package. `physis-core` is a sibling workspace member, so its
/// binary is instead located next to this package's own (both land in the
/// same shared `target/debug/`), exactly as `Edition::detect`'s own sibling
/// lookup does at runtime.
fn sibling_bin(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_BIN_EXE_physis-pro"))
        .parent()
        .expect("CARGO_BIN_EXE_physis-pro has a parent directory")
        .join(name)
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "invalid JSON: {e}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn read_only_queries_share_ids_without_creating_state() {
    let (_temp, ws) = workspace();
    let found = cli(&ws, &["find", "pump pressure calibration"]);
    assert!(found.status.success());
    let found = json(&found);
    assert_eq!(found["api_version"], "physis.system.v1");
    assert_eq!(found["data"]["documents_ranked"], 2);
    assert_eq!(found["data"]["hits"][0]["label"], "pump.rs");
    let id = found["data"]["hits"][0]["id"].as_str().unwrap();
    let read = json(&cli(&ws, &["read", id]));
    assert_eq!(read["data"]["object"]["id"], id);
    assert!(read["data"]["text"]
        .as_str()
        .unwrap()
        .contains("observed pressure"));
    assert!(!ws.state.exists(), "read-only queries created state");
}

#[test]
fn remembered_outcomes_are_recalled_and_visible_to_existing_core_observed() {
    let (_temp, ws) = workspace();
    let note = json(&cli(
        &ws,
        &[
            "remember",
            "Pressure calibration failed because the probe was disconnected",
            "--outcome",
            "failure",
            "--actor",
            "test-agent",
        ],
    ));
    assert_eq!(note["data"]["id"], "obs:1");
    let history = json(&cli(&ws, &["history", "calibration"]));
    assert_eq!(history["data"]["matched_in_window"], 1);
    let payload: Value =
        serde_json::from_str(history["data"]["records"][0]["body"].as_str().unwrap()).unwrap();
    assert_eq!(payload["outcome"], "failure");
    assert_eq!(payload["actor"], "test-agent");
    // BLOCKED on Core: as of this split, `physis-core` has no `observed`
    // subcommand (see REQUESTS.md item 4) even though `observe.rs` retains the
    // read/append primitives it would wrap. This assertion documents the
    // expected cross-binary contract and will start failing loudly (not
    // silently passing) the moment Core restores or renames the reader; do not
    // "fix" it by inventing a different Core command without checking there.
    let observed = Command::new(sibling_bin("physis-core"))
        .env("PHYSIS_CORE_DIR", &ws.state)
        .args(["observed", "--source", "system.note", "--json"])
        .output()
        .unwrap();
    assert!(
        observed.status.success(),
        "{}",
        String::from_utf8_lossy(&observed.stderr)
    );
    let records = String::from_utf8(observed.stdout).unwrap();
    assert!(
        records.contains("disconnected"),
        "the existing observer cannot see interface memory"
    );
    let found = ws.find("probe disconnected", 5).unwrap();
    assert_eq!(found["hits"][0]["id"], "obs:1");
}

#[test]
fn a_shared_core_log_keeps_workspace_history_separate() {
    let (temp, a) = workspace();
    let other = temp.path().join("other");
    fs::create_dir(&other).unwrap();
    let b = Workspace::open(&other, &a.state).unwrap();
    a.remember("alpha-only", "unverified", "a").unwrap();
    b.remember("beta-only", "unverified", "b").unwrap();
    assert_eq!(
        a.history(None, 20).unwrap()["workspace_records_in_window"],
        1
    );
    assert_eq!(b.history(None, 20).unwrap()["records"][0]["seq"], 2);
    assert!(a.read("obs:2", 1000).is_err());
}

#[test]
fn existing_filesystem_observations_join_the_interface_history() {
    let (_temp, ws) = workspace();
    let mut observations = watch_fs(&ws.root, &[], 10);
    assert_eq!(observations.len(), 2);
    physis_core::observe::append(&ws.state.join("observations.jsonl"), &mut observations).unwrap();
    let history = ws.history(Some("pump.rs"), 20).unwrap();
    assert_eq!(history["matched_in_window"], 1);
    assert_eq!(history["records"][0]["source"], "fs");
    assert_eq!(
        ws.read(&format!("obs:{}", history["records"][0]["seq"]), 1000)
            .unwrap()["record"]["source"],
        "fs"
    );
}

#[test]
fn front_door_routes_to_the_same_interface() {
    let (_temp, ws) = workspace();
    let output = Command::new(env!("CARGO_BIN_EXE_physis"))
        .env_remove("PHYSIS_CORE_DIR")
        .args(["system", "--root"])
        .arg(&ws.root)
        .args(["capabilities", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(json(&output), json(&cli(&ws, &["capabilities"])));
}

#[test]
fn folder_export_preserves_references_and_refuses_overwrite() {
    let (temp, ws) = workspace();
    ws.remember(
        "Export a reproducible reference snapshot",
        "unverified",
        "test",
    )
    .unwrap();
    let object = ws.inventory().unwrap().objects[0].clone();
    let destination = temp.path().join("view");
    let exported = ws.export(&destination).unwrap();
    assert_eq!(exported["manifest"]["files"], 2);
    assert_eq!(exported["manifest"]["live"], false);
    let file = destination
        .join("objects")
        .join(format!("{}.json", object.id.replace(':', "-")));
    let projected: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    assert_eq!(projected["id"], object.id);
    assert_eq!(projected["path"], object.path);
    assert!(destination.join("history/obs-1.json").exists());
    let index = fs::read_to_string(destination.join("INDEX.md")).unwrap();
    assert!(index.contains(&format!(
        "[{}](objects/{}.json)",
        object.path,
        object.id.replace(':', "-")
    )));
    let before = fs::read(&file).unwrap();
    assert!(ws.export(&destination).is_err());
    assert_eq!(fs::read(&file).unwrap(), before);
}

#[test]
fn invalid_reads_have_json_errors_and_nonzero_exit() {
    let (_temp, ws) = workspace();
    let out = cli(&ws, &["read", "missing.rs"]);
    assert!(!out.status.success());
    let response = json(&out);
    assert_eq!(response["ok"], false);
    assert!(response["error"]["message"].is_string());
    let out = cli(&ws, &["read", "obs:not-a-number"]);
    assert!(!out.status.success());
    assert_eq!(json(&out)["ok"], false);
}

#[test]
fn scan_exclusions_and_partial_reads_are_visible() {
    let (_temp, ws) = workspace();
    fs::create_dir(ws.root.join("target")).unwrap();
    fs::write(ws.root.join("target/generated.rs"), "should not be indexed").unwrap();
    fs::write(ws.root.join(".env"), "should not be indexed").unwrap();
    assert_eq!(ws.inventory().unwrap().objects.len(), 2);
    let read = ws.read("pump.rs", 4).unwrap();
    assert_eq!(read["text"], "// P");
    assert_eq!(read["truncated"], true);
}

#[cfg(unix)]
#[test]
fn symlink_escape_is_not_read_or_indexed() {
    let (temp, ws) = workspace();
    let outside = temp.path().join("outside.txt");
    fs::write(&outside, "outside content").unwrap();
    std::os::unix::fs::symlink(&outside, ws.root.join("escape.txt")).unwrap();
    assert_eq!(ws.inventory().unwrap().skipped_links, 1);
    assert!(ws.read("escape.txt", 1000).is_err());
    assert!(ws.read(outside.to_str().unwrap(), 1000).is_err());
}

#[cfg(unix)]
#[test]
fn run_keeps_arguments_literal_and_records_both_ends() {
    let (_temp, ws) = workspace();
    let output = cli(
        &ws,
        &[
            "run",
            "--intent",
            "Check literal arguments",
            "--actor",
            "test-agent",
            "--",
            "/usr/bin/printf",
            "%s",
            "$(touch unwanted); $HOME",
        ],
    );
    assert!(output.status.success());
    let data = &json(&output)["data"];
    assert_eq!(data["stdout_tail"], "$(touch unwanted); $HOME");
    assert_eq!(data["start_id"], "obs:1");
    assert_eq!(data["finish_id"], "obs:2");
    assert_eq!(data["result"]["process_success"], true);
    assert!(!ws.root.join("unwanted").exists());
    assert_eq!(ws.history(None, 10).unwrap()["matched_in_window"], 2);
}

#[cfg(unix)]
#[test]
fn failed_child_and_spawn_failure_remain_failed_and_recorded() {
    let (_temp, ws) = workspace();
    let output = cli(
        &ws,
        &[
            "run",
            "--intent",
            "Observe a failed command",
            "--",
            "/bin/sh",
            "-c",
            "printf 'expected failure' >&2; exit 7",
        ],
    );
    assert_eq!(output.status.code(), Some(7));
    let response = json(&output);
    assert_eq!(response["ok"], false);
    assert_eq!(response["data"]["result"]["exit_code"], 7);
    assert_eq!(response["data"]["stderr_tail"], "expected failure");
    let output = cli(
        &ws,
        &[
            "run",
            "--intent",
            "Observe spawn failure",
            "--",
            "/definitely-missing-physis-program",
        ],
    );
    assert!(!output.status.success());
    assert!(json(&output)["data"]["result"]["spawn_error"].is_string());
    assert_eq!(ws.history(None, 10).unwrap()["matched_in_window"], 4);
    assert_eq!(
        ws.inspect().unwrap()["runs_without_recorded_finish_in_window"],
        serde_json::json!([])
    );
}

#[test]
fn pack_stays_inside_its_budget_and_carries_line_provenance() {
    let (_temp, ws) = workspace();
    // A file long enough that the budget, not the corpus, decides the size.
    let long: String = (1..=400)
        .map(|i| format!("line {i} pump calibration pressure reading\n"))
        .collect();
    fs::write(ws.root.join("long.txt"), long).unwrap();

    let out = cli(
        &ws,
        &["pack", "pump calibration pressure", "--budget", "300"],
    );
    assert!(out.status.success());
    let data = &json(&out)["data"];
    let used = data["used_tokens"].as_u64().unwrap();
    assert!(used > 0, "packed nothing: {data}");
    assert!(used <= 300, "budget exceeded: {used}");
    assert!(data["windows_ranked"].as_u64().unwrap() > 0);

    let chunks = data["chunks"].as_array().unwrap();
    assert!(!chunks.is_empty());
    for chunk in chunks {
        assert!(chunk["start_line"].as_u64().unwrap() >= 1);
        assert!(chunk["end_line"].as_u64().unwrap() >= chunk["start_line"].as_u64().unwrap());
        assert!(!chunk["text"].as_str().unwrap().is_empty());
    }
    // Reading order, so the caller can re-read a region as printed.
    let mut ordered = chunks.clone();
    ordered.sort_by_key(|c| {
        (
            c["path"].as_str().unwrap().to_string(),
            c["start_line"].as_u64().unwrap(),
        )
    });
    assert_eq!(&ordered, chunks);
}

#[test]
fn pack_bridges_a_one_window_gap_but_not_an_open_end() {
    let (_temp, ws) = workspace();
    // Query terms at the start and end of a file, absent from the middle: the
    // middle window is reachable only by bridging.
    let mut text = String::new();
    for i in 1..=40 {
        text.push_str(&format!("valve seal inspection line {i}\n"));
    }
    for i in 41..=80 {
        text.push_str(&format!("MIDDLE MARKER {i}\n"));
    }
    for i in 81..=120 {
        text.push_str(&format!("valve seal inspection line {i}\n"));
    }
    // The filename rides in every window's indexed document, so a name
    // sharing a query term would score the middle window on its own.
    fs::write(ws.root.join("record.txt"), text).unwrap();

    let out = cli(&ws, &["pack", "valve seal inspection", "--budget", "4000"]);
    let data = &json(&out)["data"];
    let text: String = data["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["text"].as_str().unwrap())
        .collect();
    assert!(
        text.contains("MIDDLE MARKER"),
        "unranked middle window was not bridged: {}",
        data["chunks"]
    );
    assert!(data["windows_bridged"].as_u64().unwrap() >= 1);

    let off = cli(
        &ws,
        &[
            "pack",
            "valve seal inspection",
            "--budget",
            "4000",
            "--no-bridge",
        ],
    );
    let off = &json(&off)["data"];
    assert_eq!(off["windows_bridged"].as_u64().unwrap(), 0);
    let off_text: String = off["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["text"].as_str().unwrap())
        .collect();
    assert!(
        !off_text.contains("MIDDLE MARKER"),
        "--no-bridge still bridged, so the comparison measures nothing"
    );
}

#[test]
fn reads_accept_a_line_range_and_an_unambiguous_id_prefix() {
    let (_temp, ws) = workspace();
    fs::write(
        ws.root.join("ranged.txt"),
        "alpha\nbeta\ngamma\ndelta\nepsilon\n",
    )
    .unwrap();

    let out = cli(&ws, &["read", "ranged.txt:2-4"]);
    assert!(out.status.success());
    let data = &json(&out)["data"];
    assert_eq!(data["text"].as_str().unwrap(), "beta\ngamma\ndelta");
    assert_eq!(data["lines"]["start_line"].as_u64().unwrap(), 2);
    assert_eq!(data["lines"]["file_lines"].as_u64().unwrap(), 5);

    // The display prints an abbreviated ID; `read` must accept what it printed.
    let full = data["object"]["id"].as_str().unwrap().to_string();
    let short = format!("file:{}", &full.strip_prefix("file:").unwrap()[..12]);
    let by_prefix = cli(&ws, &["read", &short]);
    assert!(
        by_prefix.status.success(),
        "prefix ID was rejected: {short}"
    );
    assert_eq!(
        json(&by_prefix)["data"]["object"]["id"].as_str().unwrap(),
        full
    );

    // An ambiguous prefix is an error, not a silent first match.
    let ambiguous = cli(&ws, &["read", "file:"]);
    assert!(!ambiguous.status.success());
    assert!(!json(&ambiguous)["ok"].as_bool().unwrap());

    // A path containing a colon that is not a range is still a path.
    fs::write(ws.root.join("odd:name.txt"), "kept\n").unwrap();
    let odd = cli(&ws, &["read", "odd:name.txt"]);
    assert!(odd.status.success(), "colon path misread as a line range");
    assert_eq!(json(&odd)["data"]["text"].as_str().unwrap(), "kept\n");
}

#[test]
fn history_display_does_not_reprint_the_note_it_just_printed() {
    let (_temp, ws) = workspace();
    let note = "pump calibration halted: probe disconnected";
    let recorded = Command::new(env!("CARGO_BIN_EXE_physis-pro"))
        .env_remove("PHYSIS_CORE_DIR")
        .args(["system", "--root"])
        .arg(&ws.root)
        .args([
            "remember",
            note,
            "--outcome",
            "failure",
            "--actor",
            "worker3",
        ])
        .output()
        .unwrap();
    assert!(recorded.status.success());

    // Human display, not --json: the payload repeats the subject verbatim, and
    // printing both doubled the cost of every recall (measured: 2620 → 1051
    // tokens over 20 records).
    let shown = Command::new(env!("CARGO_BIN_EXE_physis-pro"))
        .env_remove("PHYSIS_CORE_DIR")
        .args(["system", "--root"])
        .arg(&ws.root)
        .args(["history", "pump"])
        .output()
        .unwrap();
    let text = String::from_utf8(shown.stdout).unwrap();
    assert!(text.contains(note), "the note itself must still be shown");
    assert_eq!(text.matches(note).count(), 1, "note printed twice:\n{text}");
    // The fields the subject does not carry are what the second line is for.
    assert!(text.contains("actor=worker3"), "{text}");
    assert!(text.contains("outcome=failure"), "{text}");
    assert!(
        !text.contains("\"workspace\""),
        "raw payload leaked:\n{text}"
    );
}

#[test]
fn build_directories_are_hidden_by_default_and_returnable_by_name() {
    let (_temp, ws) = workspace();
    fs::create_dir(ws.root.join("dist")).unwrap();
    fs::write(
        ws.root.join("dist/bundle.js"),
        "// pump calibration, compiled\n",
    )
    .unwrap();

    // Default: a published artifact's own contents are invisible, and the file
    // count is confidently wrong about what is there.
    let hidden = cli(&ws, &["list", "--limit", "50"]);
    let hidden = json(&hidden);
    let paths = hidden["data"]["inventory"]["objects"].to_string();
    assert!(!paths.contains("bundle.js"), "{paths}");

    let shown = cli(&ws, &["list", "--limit", "50", "--include-dir", "dist"]);
    let shown = json(&shown);
    assert!(
        shown["data"]["inventory"]["objects"]
            .to_string()
            .contains("bundle.js"),
        "--include-dir did not return the directory: {}",
        shown["data"]["inventory"]["objects"]
    );

    // The contract says which names are excluded *now*, not only the defaults,
    // so a caller can tell which run it is reading.
    let caps = json(&cli(&ws, &["capabilities", "--include-dir", "dist"]));
    let excluded = caps["data"]["limits"]["excluded_directories"].to_string();
    assert!(!excluded.contains("dist"), "{excluded}");
    assert!(excluded.contains("vendor"), "{excluded}");
    assert!(caps["data"]["limits"]["default_excluded_directories"]
        .to_string()
        .contains("dist"));
}

#[test]
#[cfg(unix)]
fn a_closed_pipe_ends_output_instead_of_panicking() {
    let (_temp, ws) = workspace();
    for i in 0..200 {
        fs::write(ws.root.join(format!("file{i}.txt")), "calibration\n").unwrap();
    }
    // Rust ignores SIGPIPE before main, so every `physis … | head` used to print
    // `failed printing to stdout: Broken pipe` and a panic trace after the
    // output the user asked for — which reads like a crash in their command.
    let piped = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "{} system --root {} list --limit 500 | head -2",
            env!("CARGO_BIN_EXE_physis-pro"),
            ws.root.display()
        ))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&piped.stderr);
    assert!(
        !stderr.contains("panicked"),
        "panic on closed pipe:\n{stderr}"
    );
    assert!(!stderr.contains("Broken pipe"), "{stderr}");
    assert_eq!(
        String::from_utf8_lossy(&piped.stdout).lines().count(),
        2,
        "head should still receive its two lines"
    );
}

#[test]
fn predict_reads_the_workspace_record_and_says_when_there_is_none() {
    let (_temp, ws) = workspace();

    // An unread store and an empty one look identical, and both would round to
    // "nothing ever fails".
    let empty = json(&cli(&ws, &["predict", "--", "cargo", "test"]));
    assert_eq!(empty["data"]["status"].as_str().unwrap(), "NOT MEASURED");
    assert_eq!(empty["data"]["runs_total"].as_u64().unwrap(), 0);

    for _ in 0..3 {
        let failed = cli(&ws, &["run", "--intent", "probe", "--", "false"]);
        assert!(!failed.status.success(), "`false` should fail");
    }
    let ok = cli(&ws, &["run", "--intent", "probe", "--", "true"]);
    assert!(ok.status.success());

    let seen = json(&cli(&ws, &["predict", "--", "false"]));
    let data = &seen["data"];
    assert_eq!(data["runs_of_this_kind"].as_u64().unwrap(), 3);
    assert_eq!(data["failures_of_this_kind"].as_u64().unwrap(), 3);
    assert_eq!(data["runs_total"].as_u64().unwrap(), 4);
    let p = data["failure_probability"].as_f64().unwrap();
    assert!(p > 0.7 && p < 1.0, "smoothed away from a bare 3/3: {p}");
    assert!(!data["recent"].as_array().unwrap().is_empty());

    // A kind with no record of its own falls back to the workspace's own rate,
    // not to zero and not to the failing kind's rate.
    let unseen = json(&cli(&ws, &["predict", "--", "some-command-never-run"]));
    let unseen = &unseen["data"];
    assert_eq!(unseen["runs_of_this_kind"].as_u64().unwrap(), 0);
    let backoff = unseen["failure_probability"].as_f64().unwrap();
    let workspace_rate = unseen["workspace_failure_rate"].as_f64().unwrap();
    assert!(
        (backoff - workspace_rate).abs() < 1e-9,
        "{backoff} vs {workspace_rate}"
    );

    // `cargo test` and `cargo build` are different kinds; `ls -la` is not two.
    assert_eq!(
        json(&cli(&ws, &["predict", "--", "cargo", "test"]))["data"]["kind"]
            .as_str()
            .unwrap(),
        "cargo test"
    );
    assert_eq!(
        json(&cli(&ws, &["predict", "--", "ls", "-la"]))["data"]["kind"]
            .as_str()
            .unwrap(),
        "ls"
    );
}

#[test]
fn a_run_carries_the_prior_it_was_about_to_defy() {
    let (_temp, ws) = workspace();
    for _ in 0..2 {
        cli(&ws, &["run", "--intent", "probe", "--", "false"]);
    }
    // The first run of a kind has no prior; the third has the record of the
    // first two, in the same result as its own outcome.
    let third = json(&cli(&ws, &["run", "--intent", "probe", "--", "false"]));
    let prior = &third["data"]["prior"];
    assert_eq!(prior["kind"].as_str().unwrap(), "false");
    assert_eq!(prior["runs_of_this_kind"].as_u64().unwrap(), 2);
    assert_eq!(prior["failures_of_this_kind"].as_u64().unwrap(), 2);
    assert!(!third["data"]["result"]["process_success"]
        .as_bool()
        .unwrap());

    // The prior is read before the run, so it cannot include it.
    let fourth = json(&cli(&ws, &["run", "--intent", "probe", "--", "false"]));
    assert_eq!(
        fourth["data"]["prior"]["runs_of_this_kind"]
            .as_u64()
            .unwrap(),
        3
    );
}

#[test]
fn a_borrowed_prior_is_used_only_when_its_coverage_earns_it() {
    let (_shared_temp, shared) = workspace();
    // The other store has run `false` (always fails) and `true` (never does).
    for _ in 0..4 {
        cli(&shared, &["run", "--intent", "probe", "--", "false"]);
    }
    for _ in 0..4 {
        cli(&shared, &["run", "--intent", "probe", "--", "true"]);
    }
    let shared_log = shared.state.join("observations.jsonl");

    // Cold start: this workspace has no record at all, so the borrowed prior is
    // used — E73 measured borrowed beating own on exactly this case.
    let (_cold_temp, cold) = workspace();
    let cold_out = json(&cli_with(
        &cold,
        &[
            "predict",
            "--prior-from",
            shared_log.to_str().unwrap(),
            "--",
            "false",
        ],
    ));
    let cold_data = &cold_out["data"];
    assert_eq!(cold_data["source"].as_str().unwrap(), "shared");
    assert!(
        cold_data["failure_probability"].as_f64().unwrap() > 0.6,
        "{cold_data}"
    );
    assert!(cold_data["shared"]["used"].as_bool().unwrap());

    // A workspace whose own work is entirely in kinds the shared store has
    // never seen: coverage is below the gate, so the borrowed prior is reported
    // and NOT used.
    let (_thin_temp, thin) = workspace();
    for _ in 0..6 {
        cli(&thin, &["run", "--intent", "probe", "--", "printf", "x"]);
    }
    let thin_out = json(&cli_with(
        &thin,
        &[
            "predict",
            "--prior-from",
            shared_log.to_str().unwrap(),
            "--",
            "false",
        ],
    ));
    let thin_data = &thin_out["data"];
    let coverage = thin_data["shared"]["coverage_of_this_workspace"]
        .as_f64()
        .unwrap();
    assert!(coverage < 0.9, "coverage should be low, was {coverage}");
    assert!(!thin_data["shared"]["used"].as_bool().unwrap());
    assert_eq!(thin_data["source"].as_str().unwrap(), "workspace base rate");

    // Its own record always wins over a borrowed one.
    let own = json(&cli_with(
        &thin,
        &[
            "predict",
            "--prior-from",
            shared_log.to_str().unwrap(),
            "--",
            "printf",
            "x",
        ],
    ));
    assert_eq!(own["data"]["source"].as_str().unwrap(), "workspace");

    // A missing shared log is an error, not a silent fall-back to local.
    let missing = cli_with(
        &thin,
        &[
            "predict",
            "--prior-from",
            "/nonexistent/observations.jsonl",
            "--",
            "false",
        ],
    );
    assert!(!missing.status.success());
}
