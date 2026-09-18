// provenance: moved from physis-core src/system_mcp.rs (core HEAD d65858b)
// as part of the Core/Product split, 2026-09-14. One implementation; no fork.
//! MCP stdio adapter for the shared workspace service, with no licence or model.
//! Queries and explicit outcome notes share exactly the CLI's objects and log.
//! No shell execution or delegation launch is exposed through this tool palette.
//!
//! Usage accounting: read-only calls deliberately write nothing, so the log can
//! only answer "what did agents *record*", not "what did they call". Setting
//! `PHYSIS_MCP_AUDIT=1` appends one `system.mcp.call` observation per call with
//! the tool name, ok flag, duration and the argument *key names* — never the
//! values, so neither source text nor queries leave the caller's process. Off by
//! default: an audit trail nobody asked for is log growth, not evidence.

use crate::ported::system::Workspace;
use crate::ported::system_delegation;
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::io::{Read, Write};

/// Opt-in MCP call audit. Value must be exactly `1`.
pub const AUDIT_ENV: &str = "PHYSIS_MCP_AUDIT";

fn audit_enabled() -> bool {
    std::env::var(AUDIT_ENV).ok().as_deref() == Some("1")
}

/// One audit row per `tools/call`. Argument *names* only: an audit trail that
/// stored queries would be a second copy of the caller's context.
fn audit_call(ws: &Workspace, tool: &str, args: &Value, ok: bool, ms: u64, client: &str) {
    let mut keys: Vec<&str> = args
        .as_object()
        .map(|o| o.keys().map(String::as_str).collect())
        .unwrap_or_default();
    keys.sort_unstable();
    let _ = ws.record(
        "system.mcp.call",
        tool,
        json!({"tool":tool, "ok":ok, "duration_ms":ms, "argument_keys":keys,
            "client":client, "actor":format!("mcp:{client}"),
            "note":"names only; argument values are not recorded"}),
    );
}

fn tool(
    name: &str,
    description: &str,
    properties: Value,
    required: &[&str],
    writes: bool,
) -> Value {
    json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},
        "annotations":{"readOnlyHint":!writes,"destructiveHint":false,"idempotentHint":!writes,"openWorldHint":false}})
}

pub fn workspace_tools() -> Vec<Value> {
    let text = json!({"type":"string","minLength":1,"maxLength":16384});
    let limit = json!({"type":"integer","minimum":1,"maximum":100,"default":10});
    vec![
        tool("physis_capabilities","Discover the workspace, shared state, and limits.",json!({}),&[],false),
        tool("physis_inspect","Inventory workspace files and recent recorded work.",json!({}),&[],false),
        tool("physis_find","Find files and previous work with lexical BM25; results are pointers, not judgments.",json!({"query":text,"limit":limit}),&["query"],false),
        tool("physis_pack","Read relevant source line windows within a Physis token budget (lexical retrieval).",json!({"query":text,"budget":{"type":"integer","minimum":50,"maximum":20000,"default":1200}}),&["query"],false),
        tool("physis_read","Read a relative path, path:start-end, file ID, or obs:sequence within this workspace.",json!({"target":text,"max_bytes":{"type":"integer","minimum":1,"maximum":65536,"default":32768}}),&["target"],false),
        tool("physis_history","Recall previous attempts and outcomes from this workspace before working.",json!({"query":text,"limit":limit}),&[],false),
        tool("physis_remember","Record a concise outcome with actor and evidence references. Success is operator-reported, not independently certified.",json!({"text":text,"actor":text,"outcome":{"type":"string","enum":["unverified","success","failure","inconclusive"],"default":"unverified"}}),&["text","actor"],true),
        tool("physis_predict","Consult the recorded process-failure prior before running argv; this does not execute it.",json!({"argv":{"type":"array","items":text,"minItems":1,"maxItems":64}}),&["argv"],false),
        tool("physis_tasks","List recorded delegation tasks and unstarted reservation states.",json!({}),&[],false),
        tool("physis_task","Read a delegation task with context, acceptance criteria and proposed invocation.",json!({"id":text}),&["id"],false),
        tool("physis_list","List addressable files (hidden/build/vendor directories excluded). Paged; report which slice this is.",json!({"limit":limit}),&[],false),
        tool("physis_runs","Recent command runs with intent, exit status and duration — the usage record agents actually consult via predict. Newest first.",json!({"limit":limit}),&[],false),
        tool("physis_usage","Tool/call usage counters over the workspace observation log: per-source counts plus run kinds with failure rates. Read-only rollup for adoption questions.",json!({}),&[],false),
    ]
}

fn validate(value: &Value, schema: &Value) -> Result<()> {
    if let Some(options) = schema["enum"].as_array() {
        ensure!(options.contains(value), "value not in allowed choices");
    }
    match schema["type"].as_str() {
        Some("string") => {
            let s = value.as_str().context("expected string")?;
            ensure!(
                !s.trim().is_empty() && s.len() <= 16384 && !s.contains('\0'),
                "string must be nonempty, without NUL, at most 16 KiB"
            );
        }
        Some("integer") => {
            let n = value.as_u64().context("expected positive integer")?;
            ensure!(
                n >= schema["minimum"].as_u64().unwrap_or(0)
                    && n <= schema["maximum"].as_u64().unwrap_or(u64::MAX),
                "integer outside bounds"
            );
        }
        Some("array") => {
            let items = value.as_array().context("expected array")?;
            ensure!(
                !items.is_empty() && items.len() <= 64,
                "array must contain 1..64 items"
            );
            for item in items {
                validate(item, &schema["items"])?;
            }
        }
        _ => anyhow::bail!("unsupported argument schema"),
    }
    Ok(())
}

fn call(ws: &Workspace, name: &str, args: &Value) -> Result<Value> {
    let definition = workspace_tools()
        .into_iter()
        .find(|t| t["name"] == name)
        .context("unknown tool")?;
    let object = args.as_object().context("arguments must be an object")?;
    let properties = definition["inputSchema"]["properties"].as_object().unwrap();
    for key in definition["inputSchema"]["required"].as_array().unwrap() {
        ensure!(
            object.contains_key(key.as_str().unwrap()),
            "missing argument: {key}"
        );
    }
    for (key, value) in object {
        validate(
            value,
            properties
                .get(key)
                .with_context(|| format!("unexpected argument: {key}"))?,
        )?;
    }
    let text = |key: &str| args[key].as_str().unwrap_or("");
    let number =
        |key: &str, default: usize| args[key].as_u64().map(|n| n as usize).unwrap_or(default);
    match name {
        "physis_capabilities" => Ok(ws.capabilities()),
        "physis_inspect" => ws.inspect(),
        "physis_find" => ws.find(text("query"), number("limit", 10)),
        "physis_pack" => ws.pack(text("query"), number("budget", 1200), 3),
        "physis_read" => ws.read(text("target"), number("max_bytes", 32768)),
        "physis_history" => ws.history(args["query"].as_str(), number("limit", 10)),
        "physis_remember" => ws.remember(
            text("text"),
            args["outcome"].as_str().unwrap_or("unverified"),
            text("actor"),
        ),
        "physis_predict" => ws.predict(
            &args["argv"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect::<Vec<_>>(),
        ),
        "physis_tasks" => {
            let tasks = system_delegation::delegation_tasks(ws)?;
            let rows:Vec<_>=tasks.iter().map(|t|json!({"task_id":t.plan.task.id,"intent":t.plan.request.intent,"worker":t.plan.worker.id,"observation":t.observation,
                "status":match &t.reservation {Some(r) if r.expires_at>chrono::Utc::now()=>"reserved",Some(_)=>"expired",None=>"planned"}})).collect();
            Ok(json!({"tasks":rows,"count":rows.len(),"workers_started":0}))
        }
        "physis_task" => Ok(serde_json::to_value(system_delegation::delegation_show(
            ws,
            text("id"),
        )?)?),
        "physis_list" => {
            let mut inventory = ws.inventory()?;
            let total = inventory.objects.len();
            let limit = number("limit", 100);
            inventory.truncated |= total > limit;
            inventory.objects.truncate(limit);
            Ok(json!({"inventory":inventory, "files_enumerated":total,
                "limit":limit, "truncated":inventory.truncated}))
        }
        "physis_runs" => {
            let limit = number("limit", 20).min(200);
            Ok(ws.runs(limit)?)
        }
        "physis_usage" => Ok(ws.usage()?),
        _ => anyhow::bail!("unknown tool"),
    }
}

/// Minimal newline-delimited MCP JSON-RPC. Stdout contains protocol messages only.
pub fn serve_workspace(ws: &Workspace) -> Result<()> {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    let mut initialized = false;
    // Claimed by the client at handshake; used to attribute audit rows.
    let mut client = String::from("unknown");
    let audit = audit_enabled();
    loop {
        let mut line = String::new();
        // Bound allocation even for a peer that never sends a newline.
        let bytes = std::io::BufRead::read_line(&mut (&mut input).take(1_048_577), &mut line)?;
        if bytes == 0 {
            break;
        }
        ensure!(bytes <= 1_048_576, "MCP message exceeds 1 MiB");
        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                writeln!(
                    output,
                    "{}",
                    json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}})
                )?;
                output.flush()?;
                continue;
            }
        };
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        if request.get("id").is_none() {
            continue;
        }
        let response = if request["jsonrpc"] != "2.0" || !request["method"].is_string() {
            json!({"error":{"code":-32600,"message":"Invalid Request"}})
        } else {
            match request["method"].as_str().unwrap() {
                "initialize" => {
                    initialized = true;
                    if let Some(name) = request["params"]["clientInfo"]["name"].as_str() {
                        client = name.chars().take(64).collect();
                    }
                    let version = request["params"]["protocolVersion"]
                        .as_str()
                        .unwrap_or("2024-11-05");
                    let version = if ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"]
                        .contains(&version)
                    {
                        version
                    } else {
                        "2024-11-05"
                    };
                    json!({"result":{"protocolVersion":version,"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"physis-system","version":env!("CARGO_PKG_VERSION")},"instructions":"Recall prior work with physis_history; use physis_pack and physis_read for evidence; record checked outcomes with physis_remember and an explicit actor. Results use lexical retrieval, not inferred truth."}})
                }
                "ping" => json!({"result":{}}),
                _ if !initialized => json!({"error":{"code":-32002,"message":"Initialize first"}}),
                "tools/list" => json!({"result":{"tools":workspace_tools()}}),
                "tools/call" => {
                    let args = request["params"]
                        .get("arguments")
                        .cloned()
                        .unwrap_or(json!({}));
                    let tool = request["params"]["name"].as_str().unwrap_or("").to_string();
                    let started = std::time::Instant::now();
                    let result = call(ws, &tool, &args);
                    let (text, failed) = match result {
                        Ok(data) => (serde_json::to_string(&data)?, false),
                        Err(e) => (format!("{e:#}"), true),
                    };
                    if audit {
                        audit_call(
                            ws,
                            &tool,
                            &args,
                            !failed,
                            started.elapsed().as_millis() as u64,
                            &client,
                        );
                    }
                    json!({"result":{"content":[{"type":"text","text":text}],"isError":failed}})
                }
                _ => json!({"error":{"code":-32601,"message":"Method not found"}}),
            }
        };
        let mut response = response;
        response["jsonrpc"] = json!("2.0");
        response["id"] = id;
        writeln!(output, "{}", response)?;
        output.flush()?;
    }
    Ok(())
}
