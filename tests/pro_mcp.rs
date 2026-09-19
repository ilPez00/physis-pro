//! Protocol test for the **Pro** MCP server: `physis-pro serve --stdio`.
//!
//! `tests/system_mcp.rs` covers a different server — the licence-free Core one
//! (`physis-pro system serve`, 10 workspace tools). Nothing covered the 30-tool
//! Pro surface, which is why `.mcp.json` could point at a subcommand a standard
//! build did not contain: `serve` is behind `--features mcp`, and the feature
//! was not in `Cargo.toml`'s default set (found 2026-09-16). This test is the
//! gate that would have caught it.
//!
//! It signs its own licence with the all-zeros development key rather than
//! reading a checked-in one, so it does not depend on any local file.
//!
//! Three properties of the Pro surface are asserted here that the Core server
//! does not share, each of them learned the hard way while writing this:
//!
//! * the tool set is 30 and is **not** the Core workspace set;
//! * `rmcp` negotiates the protocol version down to `2024-11-05`;
//! * replies come back **out of request order**, and input written before the
//!   handshake completes is dropped — so the client must be sequential, which
//!   is what a real MCP client is.

use base64::Engine as _;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;

/// Mint a licence signed by the development key, exactly as
/// `physis-dev-license/` does. The dev binary's `vendor_key()` resolves to the
/// public half of this seed, so the signature verifies.
fn dev_licence() -> String {
    let key = SigningKey::from_bytes(&[0u8; 32]);
    let payload = serde_json::to_vec(&json!({
        "license_id": "PX-DEV-TEST",
        "org": "pro_mcp integration test",
        "seats": 1,
        "features": ["*"],
        "iat": "2026-01-01",
        "exp": "2099-01-01",
        "maint_exp": "2099-01-01",
        "revoked": false,
    }))
    .unwrap();
    let signature = key.sign(&payload);
    json!({
        "payload": base64::engine::general_purpose::STANDARD.encode(&payload),
        "sig": base64::engine::general_purpose::STANDARD.encode(signature.to_bytes()),
    })
    .to_string()
}

/// One running `serve --stdio`, driven one request at a time.
struct Server {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Value>,
}

impl Server {
    fn start(root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_physis-pro"))
            .env_remove("PHYSIS_CORE_DIR")
            .env("PHYSIS_ALLOW_DEV_LICENCE", "1")
            .env("PHYSIS_DATA_DIR", root)
            .env("PHYSIS_LICENSE_FILE", root.join("license.key"))
            .env("PHYSIS_EMBEDDER", "random-projection")
            .arg("serve")
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if line.trim().is_empty() {
                    continue;
                }
                if tx.send(serde_json::from_str(&line).unwrap()).is_err() {
                    break;
                }
            }
        });
        Server { child, stdin, rx }
    }

    fn send(&mut self, request: &Value) {
        writeln!(self.stdin, "{request}").unwrap();
        self.stdin.flush().unwrap();
    }

    /// Send one request and wait for its reply.
    ///
    /// Waits by id, not by position: `rmcp` dispatches concurrently and answers
    /// out of order (observed: the recall reply for id 4 arriving before the
    /// remember reply for id 3). Waiting for the reply before sending the next
    /// request is also what keeps the exchange deterministic — and it is what a
    /// real client does, since input written before the handshake completes is
    /// dropped.
    fn request(&mut self, request: &Value, id: u64) -> Value {
        self.send(request);
        let deadline = Instant::now() + Duration::from_secs(120);
        while Instant::now() < deadline {
            match self.rx.recv_timeout(Duration::from_millis(500)) {
                Ok(value) if value["id"] == json!(id) => return value,
                Ok(_) => continue,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        panic!("no reply to request id {id}");
    }

    fn finish(self) -> String {
        drop(self.stdin);
        let output = self.child.wait_with_output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        assert!(
            output.status.success(),
            "serve --stdio exited {} — stderr:\n{stderr}",
            output.status
        );
        stderr
    }
}

fn init() -> Value {
    json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","clientInfo":{"name":"pro-mcp-test","version":"1"},"capabilities":{}}})
}

fn call(id: u64, name: &str, args: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})
}

fn content(response: &Value) -> Value {
    assert_eq!(
        response["result"]["isError"], false,
        "tool call failed: {response}"
    );
    serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
}

/// The server must start, advertise all 30 tools, and round-trip a write.
///
/// The tool count is the assertion that matters: it is what a build without
/// `--features mcp` cannot satisfy, because such a build has no `serve` at all
/// and the process fails before the handshake.
#[test]
fn pro_server_advertises_thirty_tools_and_round_trips_a_write() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("license.key"), dev_licence()).unwrap();

    let mut server = Server::start(dir.path());

    let handshake = server.request(&init(), 1);
    // rmcp negotiates down to 2024-11-05 regardless of what the client asks
    // for; the Core server echoes the client's version.
    assert_eq!(handshake["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(handshake["result"]["serverInfo"]["name"], "physis-pro");
    server.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));

    let listed = server.request(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}), 2);
    let tools = listed["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names.len(),
        30,
        "the Pro surface is 30 tools; got {names:?}"
    );
    for expected in [
        "physis_pack",
        "physis_route",
        "physis_why",
        "physis_causes",
        "physis_remember",
        "physis_recall",
    ] {
        assert!(
            names.contains(&expected),
            "`{expected}` missing from the Pro tool list"
        );
    }
    // A Core-only tool must not be advertised here: the two servers are
    // separate surfaces, and conflating them is how the gap went unnoticed.
    assert!(
        !names.contains(&"physis_capabilities"),
        "the Pro server must not advertise the Core workspace tools"
    );

    // Write, then read back. The Pro `physis_remember` takes `input` (a
    // labelled memory in the semiotic graph) and an optional `verdict` — not
    // the Core server's `text`/`actor`/`outcome`.
    let written = content(&server.request(
        &call(
            3,
            "physis_remember",
            json!({"input":"pro mcp round trip","verdict":1.0}),
        ),
        3,
    ));
    assert_eq!(written["label"], "pro mcp round trip");
    assert_eq!(written["verdict"], 1.0);
    assert!(!written["id"].as_str().unwrap().is_empty());

    let recalled = content(&server.request(
        &call(4, "physis_recall", json!({"query":"pro mcp round trip"})),
        4,
    ));
    assert_eq!(recalled["query"], "pro mcp round trip");
    let results = recalled["results"].as_array().unwrap();
    let hit = results
        .iter()
        .find(|h| h["label"] == "pro mcp round trip")
        .unwrap_or_else(|| {
            panic!(
                "a memory written through the Pro server must be recallable through it: {recalled}"
            )
        });
    // The verdict rides along, which is what makes recall outcome memory.
    assert_eq!(
        hit["verdict"], 1.0,
        "the asserted verdict must survive: {recalled}"
    );

    let stderr = server.finish();
    assert!(
        stderr.contains("MCP server ready on stdio"),
        "the server should say it is ready; stderr:\n{stderr}"
    );
}
