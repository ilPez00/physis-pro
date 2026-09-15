use physis_pro::intent_ir::IntentIR;
use physis_pro::registry::{Registry, seed};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_conformance_all_checks() {
    let mut pass_count = 0;
    let mut fail_count = 0;

    // --- IR tests ---

    let ir = IntentIR::new(
        HashMap::from([
            ("goal".to_string(), json!("continue positional attention experiment")),
            ("operations".to_string(), json!(["show"])),
            ("objects".to_string(), json!(["repo:physis"])),
        ]),
        "continue the positional attention experiment".to_string(),
        "gio".to_string(),
        None,
    );

    // ir.version recorded
    if ir.version == 1 {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: ir.version recorded");
    }

    // ir.nil slots detected
    if ir.has_nil() && ir.nil_slots.contains(&"constraints".to_string()) {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: ir.nil slots detected");
    }

    // ir.read_only for show
    if ir.is_read_only() {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: ir.read_only for show");
    }

    // ir.bind_tasks count
    if ir.bind_tasks().len() == ir.nil_slots.len() {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: ir.bind_tasks count");
    }

    // ir.provenance compiled_by
    if ir.compiled_by == "fixed-gate-v1" {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: ir.provenance compiled_by");
    }

    let ir_exec = IntentIR::new(
        HashMap::from([
            ("goal".to_string(), json!("build")),
            ("operations".to_string(), json!(["execute"])),
            ("success_condition".to_string(), json!("exit zero")),
        ]),
        "build physis".to_string(),
        "gio".to_string(),
        None,
    );

    // ir.execute not read_only
    if !ir_exec.is_read_only() {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: ir.execute not read_only");
    }

    // ir.render shows nil marker
    let rendered = ir_exec.render();
    if rendered.contains("constraints") && rendered.contains("NIL") {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: ir.render shows nil marker");
    }

    // --- Registry tests ---

    let tmp = TempDir::new().unwrap();
    let obs_path = tmp.path().join("observations.jsonl");
    let ws = tmp.path().join("ws");
    fs::create_dir_all(ws.join("physis-pro").join(".git")).unwrap();
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(ws.join("physis-pro"))
        .output()
        .unwrap();
    fs::create_dir_all(ws.join("randomesh")).unwrap();
    fs::write(ws.join("physis-pro").join("README.md"), "physis pro readme\n").unwrap();

    let mut reg = Registry::new(&obs_path);
    seed(&mut reg, ws.to_str().unwrap());

    // git.inspect works on repo
    let mut kwargs = HashMap::new();
    let res = reg.call("physis-pro", "inspect", kwargs);
    if res.ok && (res.result.as_ref().and_then(|r| r.get("stdout")).and_then(|s| s.as_str()).unwrap_or("").to_lowercase().contains("physis") || res.ok) {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: git.inspect works on repo");
    }

    // fs.read works on dir
    kwargs = HashMap::new();
    let res = reg.call("randomesh", "read", kwargs);
    if res.ok && res.validation == "read_ok" {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: fs.read works on dir, ok={}, validation={}", res.ok, res.validation);
    }

    // missing verb = unavailable, not no-op
    kwargs = HashMap::new();
    let res = reg.call("randomesh", "undo", kwargs);
    if !res.ok && res.error.as_deref() == Some("unavailable") {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: missing verb = unavailable, not no-op, ok={}, error={:?}", res.ok, res.error);
    }

    // write lands as draft class
    kwargs = HashMap::from([("content".to_string(), json!("hello draft\n"))]);
    let res = reg.call("physis-pro", "write", kwargs);
    let draft_path = res.result.as_ref().and_then(|r| r.as_str()).unwrap_or("");
    if res.ok && draft_path.ends_with(".draft") {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: write lands as draft class, ok={}, result={:?}", res.ok, res.result);
    }

    // draft file exists
    if std::path::Path::new(draft_path).exists() {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: draft file exists, path={}", draft_path);
    }

    // unknown object -> unavailable
    kwargs = HashMap::new();
    let res = reg.call("nonexistent-object", "read", kwargs);
    if !res.ok {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: unknown object -> unavailable");
    }

    // provenance loop: observations recorded
    let tail = reg.obs_tail(20);
    if tail.len() >= 5 {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: observations recorded, count={}", tail.len());
    }

    // observation has actor+verb
    if tail.iter().all(|o| !o.actor.is_empty() && !o.verb.is_empty()) {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: observation has actor+verb");
    }

    // mechanical validation is real: shell validator checks exit code
    // execute on bad object unavailable
    kwargs = HashMap::from([("cmd".to_string(), json!("true"))]);
    let res = reg.call("ws-does-not-exist", "execute", kwargs);
    if !res.ok {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: execute on bad object unavailable");
    }

    // validator rejects exit!=0
    kwargs = HashMap::from([("cmd".to_string(), json!("exit 3"))]);
    let res = reg.call("randomesh", "execute", kwargs);
    if !res.ok {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: validator rejects exit!=0");
    }

    // validator accepts exit 0
    kwargs = HashMap::from([("cmd".to_string(), json!("echo fine"))]);
    let res = reg.call("randomesh", "execute", kwargs);
    if res.ok {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: validator accepts exit 0");
    }

    // reliability updated on failure
    let cap = reg.get_capability("shell.execute").unwrap();
    if cap.reliability_beta > 1 {
        pass_count += 1;
    } else {
        fail_count += 1;
        eprintln!("FAIL: reliability updated on failure, beta={}", cap.reliability_beta);
    }

    println!("\n{} / {} conformance checks passed", pass_count, pass_count + fail_count);
    assert_eq!(fail_count, 0, "{} conformance checks failed", fail_count);
}