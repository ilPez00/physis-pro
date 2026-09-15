//! Protocol integration tests launch the same binary Codex and OpenCode use.
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};
use tempfile::TempDir;

fn exchange(root: &TempDir, requests: &[Value]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_physis-pro"))
        .env_remove("PHYSIS_CORE_DIR")
        .args(["system", "--root"])
        .arg(root.path())
        .arg("serve")
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
    assert_eq!(tools.len(), 10);
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
