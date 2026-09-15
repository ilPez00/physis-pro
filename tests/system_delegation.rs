//! Real CLI/log boundary: scope, reservation contention, stale plans and folders.
use serde_json::{json, Value};
use std::{
    fs,
    process::{Command, Output},
};
use tempfile::TempDir;

fn fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join(".gitignore"),
        ".physis/\ninputs/\nexport/\n",
    )
    .unwrap();
    fs::create_dir(dir.path().join("src")).unwrap();
    fs::create_dir(dir.path().join("inputs")).unwrap();
    fs::write(
        dir.path().join("src/pump.rs"),
        "// pump calibration pressure\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("private.txt"),
        "pump calibration private unrelated source\n",
    )
    .unwrap();
    for args in [
        vec!["init", "-q"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ],
    ] {
        assert!(Command::new("git")
            .current_dir(dir.path())
            .args(args)
            .output()
            .unwrap()
            .status
            .success());
    }
    fs::write(
        dir.path().join("inputs/task.json"),
        serde_json::to_vec(&json!({
            "schema":"physis.task.v1","intent":"pump calibration","paths":["src"],"mode":"write",
            "acceptance":["regression passes"],"checks":[["cargo","test"]],"budget":200
        }))
        .unwrap(),
    )
    .unwrap();
    // The planner must never execute even this harmless program.
    fs::write(
        dir.path().join("inputs/workers.json"),
        serde_json::to_vec(&json!({
            "schema":"physis.workers.v1","workers":[{
                "id":"free","kind":"opencode","program":"/usr/bin/false","provider":"local",
                "model":"local/test","cost":"local","remote_model":false,"modes":["write"],
                "max_parallel":1,"server":"http://127.0.0.1:4096"
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    dir
}
fn command(dir: &TempDir, args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_physis-pro"));
    cmd.current_dir(dir.path())
        .env_remove("PHYSIS_CORE_DIR")
        .args(["system", "--json", "--root"])
        .arg(dir.path())
        .arg("delegation")
        .args(args);
    cmd
}
fn run(dir: &TempDir, args: &[&str]) -> Output {
    command(dir, args).output().unwrap()
}
fn data(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}
fn create(dir: &TempDir) -> Value {
    data(&run(
        dir,
        &[
            "create",
            "--task",
            "inputs/task.json",
            "--registry",
            "inputs/workers.json",
        ],
    ))
}
fn alter(dir: &TempDir, path: &str, change: impl FnOnce(&mut Value)) {
    let path = dir.path().join(path);
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    change(&mut value);
    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
}

#[test]
fn preview_is_read_only_scoped_and_repeatable() {
    let dir = fixture();
    let args = [
        "plan",
        "--task",
        "inputs/task.json",
        "--registry",
        "inputs/workers.json",
    ];
    let a = data(&run(&dir, &args));
    let b = data(&run(&dir, &args));
    assert_eq!(a["task"]["id"], b["task"]["id"]);
    assert!(a["context"]["used_tokens"].as_u64().unwrap() <= 200);
    assert_eq!(a["context"]["chunks"][0]["path"], "src/pump.rs");
    assert!(!a["context"].to_string().contains("private unrelated"));
    assert_eq!(a["source"]["scoped_files"], 1);
    assert!(a["invocation"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x == "--attach"));
    assert!(!dir.path().join(".physis").exists());
}

#[test]
fn create_list_show_export_and_history_use_same_ids() {
    let dir = fixture();
    let created = create(&dir);
    let id = created["plan"]["task"]["id"].as_str().unwrap();
    let second = create(&dir);
    assert_ne!(created["plan"]["task"]["id"], second["plan"]["task"]["id"]);
    assert_eq!(second["plan"]["duplicates"][0], id);
    assert_eq!(data(&run(&dir, &["list"]))["count"], 2);
    assert_eq!(data(&run(&dir, &["show", id])), created);
    data(&run(&dir, &["export", id, "export"]));
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(dir.path().join("export/task.json")).unwrap())
            .unwrap(),
        created
    );
    assert!(!run(&dir, &["export", id, "export"]).status.success());
    let history = Command::new(env!("CARGO_BIN_EXE_physis-pro"))
        .current_dir(dir.path())
        .env_remove("PHYSIS_CORE_DIR")
        .args(["system", "--json", "history", id])
        .output()
        .unwrap();
    assert!(data(&history)["matched_in_window"].as_u64().unwrap() > 0);
}

#[test]
fn concurrent_reservations_have_one_owner_and_release_checks_token() {
    let dir = fixture();
    let t = create(&dir);
    let id = t["plan"]["task"]["id"].as_str().unwrap();
    let mut children = Vec::new();
    for _ in 0..6 {
        children.push(
            command(&dir, &["reserve", id])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        );
    }
    let outputs: Vec<_> = children
        .into_iter()
        .map(|c| c.wait_with_output().unwrap())
        .collect();
    let wins: Vec<_> = outputs.iter().filter(|o| o.status.success()).collect();
    assert_eq!(wins.len(), 1);
    assert!(!run(&dir, &["release", id, "--token", "wrong"])
        .status
        .success());
    let reservation = data(wins[0]);
    data(&run(
        &dir,
        &[
            "release",
            id,
            "--token",
            reservation["token"].as_str().unwrap(),
        ],
    ));
    data(&run(&dir, &["reserve", id]));
    assert!(!std::path::Path::new(t["plan"]["worktree"].as_str().unwrap()).exists());
}

#[test]
fn capacity_and_dirty_source_are_checked_at_reservation() {
    let dir = fixture();
    let a = create(&dir);
    let b = create(&dir);
    let id = a["plan"]["task"]["id"].as_str().unwrap();
    data(&run(&dir, &["reserve", id]));
    assert!(!run(
        &dir,
        &["reserve", b["plan"]["task"]["id"].as_str().unwrap()]
    )
    .status
    .success());
    let dir = fixture();
    let t = create(&dir);
    fs::write(dir.path().join("src/pump.rs"), "changed without a commit\n").unwrap();
    let output = run(
        &dir,
        &["reserve", t["plan"]["task"]["id"].as_str().unwrap()],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("source changed"));
}

#[test]
fn reservations_survive_history_windows_and_corruption_is_not_silently_skipped() {
    let dir = fixture();
    let a = create(&dir);
    let b = create(&dir);
    data(&run(
        &dir,
        &["reserve", a["plan"]["task"]["id"].as_str().unwrap()],
    ));
    let log = dir.path().join(".physis/system/observations.jsonl");
    use std::io::Write;
    let mut writer = fs::OpenOptions::new().append(true).open(&log).unwrap();
    for seq in 4..10_010 {
        let mut obs = physis_core::observe::Observation::new("test", "unrelated later event");
        obs.seq = seq;
        writeln!(writer, "{}", serde_json::to_string(&obs).unwrap()).unwrap();
    }
    drop(writer);
    let refused = run(
        &dir,
        &["reserve", b["plan"]["task"]["id"].as_str().unwrap()],
    );
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("capacity"));
    let mut writer = fs::OpenOptions::new().append(true).open(&log).unwrap();
    writeln!(writer, "torn-event").unwrap();
    drop(writer);
    let refused = run(&dir, &["list"]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("corrupt observation log"));
}

#[test]
fn recording_does_not_stale_a_plan_when_state_is_not_gitignored() {
    let dir = fixture();
    fs::write(dir.path().join(".gitignore"), "inputs/\nexport/\n").unwrap();
    let task = create(&dir);
    data(&run(
        &dir,
        &["reserve", task["plan"]["task"]["id"].as_str().unwrap()],
    ));
}

#[test]
fn unknown_fields_paths_and_unapproved_remote_models_fail_closed() {
    for path in ["../outside", "/tmp", "src/../private.txt", ".git/config"] {
        let dir = fixture();
        alter(&dir, "inputs/task.json", |v| v["paths"] = json!([path]));
        assert!(!run(
            &dir,
            &[
                "plan",
                "--task",
                "inputs/task.json",
                "--registry",
                "inputs/workers.json"
            ]
        )
        .status
        .success());
        assert!(!dir.path().join(".physis").exists());
    }
    let dir = fixture();
    alter(&dir, "inputs/task.json", |v| {
        v["task_id"] = json!("spoofed")
    });
    assert!(!run(
        &dir,
        &[
            "create",
            "--task",
            "inputs/task.json",
            "--registry",
            "inputs/workers.json"
        ]
    )
    .status
    .success());
    let dir = fixture();
    alter(&dir, "inputs/workers.json", |v| {
        v["workers"][0]["remote_model"] = json!(true)
    });
    assert!(!run(
        &dir,
        &[
            "plan",
            "--task",
            "inputs/task.json",
            "--registry",
            "inputs/workers.json"
        ]
    )
    .status
    .success());
}

#[cfg(unix)]
#[test]
fn scope_rejects_symlink_ancestors_even_for_missing_targets() {
    let dir = fixture();
    std::os::unix::fs::symlink("/tmp", dir.path().join("escape")).unwrap();
    alter(&dir, "inputs/task.json", |v| {
        v["paths"] = json!(["escape/not-yet-created.rs"])
    });
    assert!(!run(
        &dir,
        &[
            "plan",
            "--task",
            "inputs/task.json",
            "--registry",
            "inputs/workers.json"
        ]
    )
    .status
    .success());
}
