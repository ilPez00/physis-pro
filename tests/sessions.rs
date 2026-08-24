//! Phase 9.1 — session-transcript adapter registry (clean-room adaptation,
//! see docs/reference/repowise-adaptation.md P2).
//!
//! Contract under test:
//! - adapters turn raw transcript LINES into normalized Events;
//! - unparseable input yields None / no events, never a panic;
//! - discovery returns transcript files under a root, sorted, absent dir = empty;
//! - the registry hands out adapters by name and rejects unknown names;
//! - a cheap substring prefilter gates expensive JSON parsing.

use physis_pro::sessions::{get_adapter, registered, Event};

const CLAUDE_USER_LINE: &str = r#"{"type":"user","sessionId":"s1","cwd":"/repo","message":{"role":"user","content":"Review this change for security vulnerabilities."}}"#;

const CLAUDE_ASSISTANT_LINE: &str = r#"{"type":"assistant","sessionId":"s1","message":{"model":"claude-opus-5","role":"assistant","content":[{"type":"thinking","thinking":""},{"type":"text","text":"Scanning the diff."},{"type":"tool_use","id":"toolu_01J","name":"Bash","input":{"command":"cargo audit","description":"run audit"}}]}}"#;

const CLAUDE_TOOL_RESULT_LINE: &str = r#"{"type":"user","sessionId":"s1","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_01J","is_error":false,"content":"vulnerabilities found: 0"}]}}"#;

const CODEX_META_LINE: &str = r#"{"timestamp":"2026-08-20T15:40:18.088Z","ordinal":0,"type":"session_meta","payload":{"session_id":"01a01fd4","id":"01a01fd4","cwd":"/home/gio","originator":"codex-tui"}}"#;

const CODEX_USER_LINE: &str = r#"{"timestamp":"2026-08-20T15:40:18.5Z","ordinal":1,"type":"response_item","payload":{"type":"message","id":"msg_1","role":"user","content":[{"type":"input_text","text":"Fix the flaky test."}]}}"#;

const CODEX_ASSISTANT_LINE: &str = r#"{"timestamp":"2026-08-20T15:41:00.0Z","ordinal":2,"type":"response_item","payload":{"type":"message","id":"msg_2","role":"assistant","content":[{"type":"output_text","text":"Retrying the suite twice."}]}}"#;

#[test]
fn registry_lists_builtins_and_rejects_unknown() {
    let names = registered();
    assert!(names.contains(&"claude_code"));
    assert!(names.contains(&"codex"));

    assert_eq!(get_adapter("claude_code").unwrap().name(), "claude_code");
    assert_eq!(get_adapter("codex").unwrap().name(), "codex");

    // get_adapter returns Err (no panic) for unknown names.
    assert!(
        get_adapter("nope").is_err(),
        "unknown adapter name must error, not default silently"
    );
}

#[test]
fn claude_user_prose_line_yields_one_user_turn() {
    let a = get_adapter("claude_code").unwrap();
    let evs = a.normalize(CLAUDE_USER_LINE).expect("line parses");
    assert_eq!(evs.len(), 1);
    match &evs[0] {
        Event::UserTurn { text } => {
            assert!(text.contains("security vulnerabilities"))
        }
        other => panic!("expected UserTurn, got {other:?}"),
    }
}

#[test]
fn claude_assistant_line_yields_turn_plus_tool_call() {
    let a = get_adapter("claude_code").unwrap();
    let evs = a.normalize(CLAUDE_ASSISTANT_LINE).expect("line parses");
    assert_eq!(evs.len(), 2, "thinking block ignored, text + tool_use kept");
    assert!(matches!(&evs[0], Event::AssistantTurn { .. }));
    match &evs[1] {
        Event::ToolCall { name, .. } => assert_eq!(name, "Bash"),
        other => panic!("expected ToolCall, got {other:?}"),
    }
}

#[test]
fn claude_tool_result_maps_success_flag() {
    let a = get_adapter("claude_code").unwrap();
    let evs = a.normalize(CLAUDE_TOOL_RESULT_LINE).expect("line parses");
    match &evs[0] {
        Event::ToolResult { ok, .. } => assert!(ok),
        other => panic!("expected ToolResult, got {other:?}"),
    }
}

#[test]
fn claude_tool_result_error_flag_flips_ok() {
    let a = get_adapter("claude_code").unwrap();
    let line = CLAUDE_TOOL_RESULT_LINE.replace("\"is_error\":false", "\"is_error\":true");
    let evs = a.normalize(&line).expect("line parses");
    match &evs[0] {
        Event::ToolResult { ok, .. } => assert!(!ok),
        other => panic!("expected ToolResult, got {other:?}"),
    }
}

#[test]
fn codex_lines_map_roles_to_turns() {
    let a = get_adapter("codex").unwrap();
    assert!(
        a.normalize(CODEX_META_LINE).unwrap_or_default().is_empty(),
        "meta carries no conversation event"
    );
    let user = a.normalize(CODEX_USER_LINE).expect("parses");
    match &user[0] {
        Event::UserTurn { text } => assert!(text.contains("flaky test")),
        other => panic!("expected UserTurn, got {other:?}"),
    }
    let asst = a.normalize(CODEX_ASSISTANT_LINE).expect("parses");
    match &asst[0] {
        Event::AssistantTurn { text } => assert!(text.contains("suite")),
        other => panic!("expected AssistantTurn, got {other:?}"),
    }
}

#[test]
fn garbage_and_empty_input_yield_none_never_panic() {
    let a = get_adapter("claude_code").unwrap();
    assert!(a.normalize("").is_none());
    assert!(a.normalize("not json at all").is_none());
    assert!(a
        .normalize("{\"type\":\"unknown-shape\"}")
        .unwrap_or_default()
        .is_empty());
    assert!(a.normalize("[1,2,3]").unwrap_or_default().is_empty());
}

#[test]
fn discover_returns_sorted_jsonl_absent_dir_is_empty() {
    let a = get_adapter("claude_code").unwrap();
    let absent = a.discover(std::path::Path::new("/nonexistent/proj"));
    assert!(absent.is_empty());

    let tmp = std::env::temp_dir().join(format!("physis_sess_test_{}", std::process::id()));
    let sub = tmp.join("nested");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(tmp.join("b.jsonl"), "").unwrap();
    std::fs::write(sub.join("a.jsonl"), "").unwrap();
    std::fs::write(tmp.join("ignore.txt"), "").unwrap();

    let found = a.discover(&tmp);
    assert_eq!(found.len(), 2, "only .jsonl, recursive");
    let mut sorted = found.clone();
    sorted.sort();
    assert_eq!(found, sorted, "discovery output must be sorted: {found:?}");
    assert!(found.iter().any(|p| p.ends_with("b.jsonl")));
    assert!(found.iter().any(|p| p.ends_with("nested/a.jsonl")));

    std::fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn prefilter_gates_expensive_parse() {
    // cheap substring gate: relevant lines contain the needle, others skip.
    assert!(physis_pro::sessions::line_may_contain(
        "{\"type\":\"assistant\",...}",
        &["\"type\":\"assistant\"", "\"type\":\"user\""]
    ));
    assert!(!physis_pro::sessions::line_may_contain(
        "{\"type\":\"file-history-snapshot\",\"x\":1}",
        &["\"type\":\"assistant\"", "\"type\":\"user\""]
    ));
}

#[test]
fn full_file_iteration_builds_event_stream() {
    let tmp = std::env::temp_dir().join(format!("physis_sess_stream_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let file = tmp.join("s1.jsonl");
    std::fs::write(
        &file,
        format!(
            "{CLAUDE_USER_LINE}\nnot json\n{CLAUDE_ASSISTANT_LINE}\n{CLAUDE_TOOL_RESULT_LINE}\n"
        ),
    )
    .unwrap();

    let a = get_adapter("claude_code").unwrap();
    let mut n_user = 0;
    let mut n_call = 0;
    let mut n_result = 0;
    {
        use std::io::BufRead;
    for line in std::io::BufReader::new(std::fs::File::open(&file).unwrap()).lines() {
        if let Some(evs) = a.normalize(line.unwrap().trim()) {
            for e in evs {
                match e {
                    Event::UserTurn { .. } => n_user += 1,
                    Event::ToolCall { .. } => n_call += 1,
                    Event::ToolResult { .. } => n_result += 1,
                    _ => {}
                }
            }
        }
    }
    assert_eq!((n_user, n_call, n_result), (1, 1, 1));
    }
    std::fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn real_local_claude_transcript_parses() {
    // Ground truth: actual transcripts on this machine must not wedge the parser.
    let proj = std::path::Path::new("/home/gio/.claude/projects/-home-gio-dev-physis-pro");
    if !proj.is_dir() {
        return; // machine without local transcripts still passes
    }
    let a = get_adapter("claude_code").unwrap();
    use std::io::BufRead;
    let mut parsed_any = false;
    for f in a.discover(proj).into_iter().take(3) {
        for line in std::io::BufReader::new(std::fs::File::open(f).unwrap())
            .lines()
            .map_while(Result::ok)
        {
            if line.len() > 200_000 {
                continue; // same cap the production reader will use
            }
            if let Some(evs) = a.normalize(line.trim()) {
                if !evs.is_empty() {
                    parsed_any = true;
                }
            }
        }
    }
    assert!(
        parsed_any,
        "real transcripts should yield at least one event"
    );
}
