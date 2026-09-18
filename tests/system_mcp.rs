//! Protocol integration tests launch the same binary Codex and OpenCode use.
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};
use tempfile::TempDir;

fn exchange(root: &TempDir, requests: &[Value]) -> Vec<Value> {
    exchange_env(root, requests, &[])
}

fn exchange_env(root: &TempDir, requests: &[Value], env: &[(&str, &str)]) -> Vec<Value> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_physis-pro"));
    command
        .env_remove("PHYSIS_CORE_DIR")
        // The audit trail is opt-in; a developer's exported value must not
        // change what the read-only tests observe.
        .env_remove("PHYSIS_MCP_AUDIT")
        .args(["system", "--root"])
        .arg(root.path())
        .arg("serve");
    for (key, value) in env {
        command.env(key, value);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for request in requests {
        writeln!(input, "{request}").unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn init() -> Value {
    json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","clientInfo":{"name":"test","version":"1"},"capabilities":{}}})
}
fn call(id: u32, name: &str, args: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})
}
fn content(response: &Value) -> Value {
    assert_eq!(response["result"]["isError"], false);
    serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
}

#[test]
fn handshake_queries_and_write_read_roundtrip_share_cli_log() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("pump.rs"), "// pump calibration pressure\n").unwrap();
    let responses = exchange(
        &dir,
        &[
            init(),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
            call(
                3,
                "physis_pack",
                json!({"query":"pump calibration","budget":100}),
            ),
            call(
                4,
                "physis_remember",
                json!({"text":"MCP calibration checked","actor":"codex","outcome":"success"}),
            ),
            call(5, "physis_history", json!({"query":"MCP calibration"})),
            call(6, "physis_read", json!({"target":"obs:1"})),
        ],
    );
    assert_eq!(
        responses.len(),
        6,
        "notifications and EOF must not emit extra JSON"
    );
    assert_eq!(responses[0]["result"]["protocolVersion"], "2025-11-25");
    let tools = responses[1]["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(tools.len(), 13, "tool palette: {names:?}");
    // Read-only rollups added with the usage work; each must stay read-only.
    for added in ["physis_list", "physis_runs", "physis_usage"] {
        let tool = tools.iter().find(|t| t["name"] == added).unwrap();
        assert_eq!(tool["annotations"]["readOnlyHint"], true, "{added}");
        assert_eq!(tool["annotations"]["destructiveHint"], false, "{added}");
    }
    assert!(!tools.iter().any(|t| t["name"] == "physis_run"));
    assert_eq!(content(&responses[2])["chunks"][0]["path"], "pump.rs");
    assert_eq!(content(&responses[3])["id"], "obs:1");
    assert_eq!(content(&responses[4])["matched_in_window"], 1);
    assert_eq!(content(&responses[5])["record"]["source"], "system.note");
    let other_client = exchange(
        &dir,
        &[
            init(),
            call(2, "physis_history", json!({"query":"MCP calibration"})),
        ],
    );
    assert_eq!(content(&other_client[1])["matched_in_window"], 1);
}

#[test]
fn errors_are_protocol_results_and_invalid_notes_do_not_write() {
    let dir = TempDir::new().unwrap();
    let responses = exchange(
        &dir,
        &[
            json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}),
            init(),
            call(2, "physis_pack", json!({"query":"hello","budget":0})),
            call(
                3,
                "physis_remember",
                json!({"text":"hello","actor":"test","outcome":"certified"}),
            ),
            call(4, "physis_read", json!({"target":"/etc/passwd"})),
            call(5, "physis_history", json!({"root":"/etc"})),
            call(6, "physis_predict", json!({"argv":[]})),
            call(7, "physis_run", json!({"argv":["touch","marker"]})),
            json!({"jsonrpc":"2.0","id":8,"method":"unknown"}),
        ],
    );
    assert_eq!(responses[0]["error"]["code"], -32002);
    for response in &responses[2..8] {
        assert_eq!(response["result"]["isError"], true);
    }
    assert_eq!(responses[8]["error"]["code"], -32601);
    assert!(!dir.path().join(".physis").exists());
    assert!(!dir.path().join("marker").exists());
}

/// Usage data is the point of the read-only rollups: `physis_runs` reports the
/// runs `predict` reads, and `physis_usage` counts what the log actually holds.
/// A read-only query must not write, so the counts are a lower bound on reads
/// and exact on notes/runs — the response says which.
#[test]
fn usage_rollups_count_recorded_work_without_writing() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("pump.rs"), "// pump calibration pressure\n").unwrap();
    let responses = exchange(
        &dir,
        &[
            init(),
            call(
                1,
                "physis_remember",
                json!({"text":"calibration note","actor":"cline","outcome":"success"}),
            ),
            call(2, "physis_runs", json!({"limit":10})),
            call(3, "physis_usage", json!({})),
            call(4, "physis_list", json!({"limit":1})),
        ],
    );
    let runs = content(&responses[2]);
    assert_eq!(runs["count"], 0, "no run has been executed: {runs}");
    assert_eq!(runs["runs_total"], 0);
    let usage = content(&responses[3]);
    assert_eq!(usage["by_source"]["system.note"], 1, "{usage}");
    assert_eq!(usage["workspace_records_in_window"], 1, "{usage}");
    assert_eq!(usage["runs_total"], 0);
    assert!(usage["coverage_note"]
        .as_str()
        .unwrap()
        .contains("lower-bound"));
    let list = content(&responses[4]);
    assert_eq!(list["limit"], 1);
    assert_eq!(list["files_enumerated"], 1, "{list}");
    assert_eq!(list["truncated"], false, "one file fits a limit of one");
    assert!(
        !dir.path().join("system-write.lock").exists(),
        "the observation log was appended exactly once, by remember"
    );
}

/// Opt-in audit is the only way the log can answer "which tool did the agent
/// actually call". It must be off by default (the test above proves the log
/// stays absent) and, when on, must record the tool name and outcome without
/// copying the caller's argument values into the store.
#[test]
fn mcp_audit_records_tool_names_only_when_opted_in() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("secret.rs"), "// paste-the-secret-query\n").unwrap();
    let responses = exchange_env(
        &dir,
        &[
            init(),
            call(1, "physis_find", json!({"query":"paste-the-secret-query"})),
            call(2, "physis_read", json!({"target":"../outside"})),
            call(3, "physis_usage", json!({})),
        ],
        &[("PHYSIS_MCP_AUDIT", "1")],
    );
    // The failing call still ran through the audit path.
    assert_eq!(responses[2]["result"]["isError"], true);
    let usage = content(&responses[3]);
    let calls = usage["mcp_calls"].as_array().unwrap();
    let by_tool = |name: &str| calls.iter().find(|r| r["tool"] == name).unwrap().clone();
    assert_eq!(by_tool("physis_find")["calls"], 1, "{usage}");
    assert_eq!(by_tool("physis_find")["errors"], 0);
    assert_eq!(
        by_tool("physis_read")["errors"],
        1,
        "the escape failed: {usage}"
    );
    assert_eq!(
        usage["mcp_calls_total"], 2,
        "audit counts calls, not the rollup itself"
    );
    assert_eq!(usage["by_source"]["system.mcp.call"], 2, "{usage}");

    let log = fs::read_to_string(dir.path().join(".physis/system/observations.jsonl")).unwrap();
    let audit: Vec<Value> = log
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|record| record["source"] == "system.mcp.call")
        .map(|record| serde_json::from_str(record["body"].as_str().unwrap()).unwrap())
        .collect();
    // The rollup does not count its own call, but the log does record it: the
    // audit is appended after a call answers, so a call never counts itself.
    assert_eq!(audit.len(), 3, "{audit:?}");
    let find_row = audit.iter().find(|r| r["tool"] == "physis_find").unwrap();
    assert_eq!(find_row["argument_keys"], json!(["query"]), "{find_row}");
    assert_eq!(find_row["client"], "test", "{find_row}");
    assert_eq!(find_row["actor"], "mcp:test");
    assert_eq!(find_row["ok"], true);
    assert_eq!(
        audit.iter().find(|r| r["tool"] == "physis_read").unwrap()["ok"],
        false,
        "a failed call is audited as an error"
    );
    assert!(
        !log.contains("paste-the-secret-query"),
        "argument values must not be copied into the audit trail: {log}"
    );
}
