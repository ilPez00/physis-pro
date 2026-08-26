//! Phase 9.1 — session-transcript adapters (clean-room pattern adaptation
//! from docs/reference/repowise-adaptation.md P2; no source code reused).
//!
//! One normalized [`Event`] stream per agent harness. An adapter knows:
//! 1. where a harness's transcripts live for a repo root — [`TranscriptAdapter::discover`];
//! 2. how ONE raw transcript line becomes events — [`TranscriptAdapter::normalize`].
//!
//! `normalize` is best-effort: anything unparseable yields `None` (or an
//! empty vec for valid-but-irrelevant lines), never a panic — transcript
//! formats change between CLI versions and must not wedge ingestion.
//!
//! Consumers ask by intent (see [`Event`]); harness vocabulary never leaks
//! past this module.

use std::path::{Path, PathBuf};

/// Normalized conversation event mined from any supported transcript.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// User prose turn.
    UserTurn { text: String },
    /// Assistant prose turn.
    AssistantTurn { text: String },
    /// Tool invocation by the assistant.
    ToolCall { name: String },
    /// Tool result delivery (`ok` mirrors the harness's error flag).
    ToolResult { ok: bool },
}

impl Event {
    /// Short kind tag for logging/rollups.
    pub fn kind(&self) -> &'static str {
        match self {
            Event::UserTurn { .. } => "user_turn",
            Event::AssistantTurn { .. } => "assistant_turn",
            Event::ToolCall { .. } => "tool_call",
            Event::ToolResult { .. } => "tool_result",
        }
    }
}

/// The contract every harness adapter implements.
pub trait TranscriptAdapter: Send + Sync {
    /// Stable identifier, e.g. `"claude_code"`.
    fn name(&self) -> &'static str;

    /// Transcript files for sessions rooted at `repo_root`, sorted.
    /// An absent directory yields an empty list, never an error.
    fn discover(&self, repo_root: &Path) -> Vec<PathBuf>;

    /// One raw transcript line → its events.
    /// `None` = line unparseable/foreign; `Some(vec![])` = parsed, nothing of interest.
    fn normalize(&self, raw_line: &str) -> Option<Vec<Event>>;
}

// ── registry ────────────────────────────────────────────────────────────────

/// Registered adapter instances keyed by [`TranscriptAdapter::name`].
fn adapters() -> &'static [&'static dyn TranscriptAdapter] {
    &[&ClaudeCodeAdapter, &CodexAdapter]
}

/// Names of every registered harness, sorted.
pub fn registered() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = adapters().iter().map(|a| a.name()).collect();
    names.sort_unstable();
    names
}

/// The adapter for `name`, or a panic-free error via `Err`.
///
/// Lookup happens per call; adapters are stateless so sharing them is safe.
pub fn get_adapter(name: &str) -> Result<&'static dyn TranscriptAdapter, String> {
    adapters()
        .iter()
        .copied()
        .find(|a| a.name() == name)
        .ok_or_else(|| format!("no transcript adapter registered for {name:?}"))
}

/// Cheap substring gate applied BEFORE paying for JSON parsing: transcript
/// lines routinely run to hundreds of kilobytes and most are irrelevant to a
/// given consumer. True = the line contains any needle and is worth parsing.
pub fn line_may_contain(raw_line: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| raw_line.contains(n))
}

// ── shared JSON helpers ─────────────────────────────────────────────────────

use serde_json::Value;

fn parse(raw_line: &str) -> Option<Value> {
    serde_json::from_str(raw_line).ok()
}

/// Extract prose from a Claude-Code-style content block array or plain string.
fn claude_text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| {
                let ty = b.get("type")?.as_str()?;
                if ty == "text" || ty == "input_text" || ty == "output_text" {
                    b.get("text").and_then(Value::as_str).map(str::to_owned)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_owned(),
        _ => String::new(),
    }
}

// ── Claude Code ─────────────────────────────────────────────────────────────

/// Claude Code JSONL: `~/.claude/projects/<munged-cwd>/<session-id>.jsonl`.
/// Line types of interest: `user` (prose string OR tool_result block array),
/// `assistant` (content blocks: thinking / text / tool_use).
pub struct ClaudeCodeAdapter;

impl TranscriptAdapter for ClaudeCodeAdapter {
    fn name(&self) -> &'static str {
        "claude_code"
    }

    fn discover(&self, repo_root: &Path) -> Vec<PathBuf> {
        discover_jsonl(repo_root)
    }

    fn normalize(&self, raw_line: &str) -> Option<Vec<Event>> {
        // Cheap gate first: only two line types can ever yield events.
        if !line_may_contain(raw_line, &["\"type\":\"user\"", "\"type\":\"assistant\""]) {
            return None;
        }
        let v = parse(raw_line)?;
        match v.get("type").and_then(Value::as_str)? {
            "user" => {
                let msg = v.get("message")?;
                match msg.get("content")? {
                    Value::String(text) => {
                        let t = text.trim();
                        if t.is_empty() {
                            Some(Vec::new())
                        } else {
                            Some(vec![Event::UserTurn { text: t.to_owned() }])
                        }
                    }
                    Value::Array(blocks) => {
                        let mut evs = Vec::new();
                        for b in blocks {
                            if b.get("type").and_then(Value::as_str) == Some("tool_result") {
                                evs.push(Event::ToolResult {
                                    ok: !b
                                        .get("is_error")
                                        .and_then(Value::as_bool)
                                        .unwrap_or(false),
                                });
                            }
                        }
                        Some(evs)
                    }
                    _ => Some(Vec::new()),
                }
            }
            "assistant" => {
                let msg = v.get("message")?;
                let mut evs = Vec::new();
                if let Value::Array(blocks) = msg.get("content")? {
                    let mut prose = Vec::new();
                    for b in blocks {
                        match b.get("type").and_then(Value::as_str) {
                            Some("text") => {
                                if let Some(t) = b.get("text").and_then(Value::as_str) {
                                    prose.push(t.to_owned());
                                }
                            }
                            Some("tool_use") => {
                                evs.push(Event::ToolCall {
                                    name: b
                                        .get("name")
                                        .and_then(Value::as_str)
                                        .unwrap_or("unknown")
                                        .to_owned(),
                                });
                            }
                            _ => {} // thinking / redacted_thinking etc. ignored
                        }
                    }
                    let joined = prose.join(" ").trim().to_owned();
                    if !joined.is_empty() {
                        evs.insert(0, Event::AssistantTurn { text: joined });
                    }
                }
                Some(evs)
            }
            _ => Some(Vec::new()),
        }
    }
}

// ── Codex CLI ───────────────────────────────────────────────────────────────

/// Codex rollout JSONL: `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`.
/// Conversation lives in `response_item` lines with `payload.type == "message"`
/// and roles user/assistant; `session_meta`, `event_msg`, `turn_context`,
/// `token_count` etc. carry no conversation events.
pub struct CodexAdapter;

impl TranscriptAdapter for CodexAdapter {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn discover(&self, repo_root: &Path) -> Vec<PathBuf> {
        discover_jsonl(repo_root)
    }

    fn normalize(&self, raw_line: &str) -> Option<Vec<Event>> {
        if !line_may_contain(raw_line, &["\"response_item\"", "\"message\""]) {
            return None;
        }
        let v = parse(raw_line)?;
        if v.get("type").and_then(Value::as_str)? != "response_item" {
            return Some(Vec::new());
        }
        let p = v.get("payload")?;
        if p.get("type").and_then(Value::as_str)? != "message" {
            return Some(Vec::new());
        }
        let role = p.get("role").and_then(Value::as_str)?;
        let text = claude_text_of(p.get("content").unwrap_or(&Value::Null));
        let text = text.trim();
        if text.is_empty() {
            return Some(Vec::new());
        }
        match role {
            "user" | "developer" => Some(vec![Event::UserTurn {
                text: text.to_owned(),
            }]),
            "assistant" => Some(vec![Event::AssistantTurn {
                text: text.to_owned(),
            }]),
            _ => Some(Vec::new()),
        }
    }
}

// ── discovery ───────────────────────────────────────────────────────────────

/// Recursive `.jsonl` walk under `repo_root`, sorted, absent dir = empty.
fn discover_jsonl(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(discover_jsonl(&path));
        } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            out.push(path);
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_prose_string_yields_no_event_not_none() {
        let a = get_adapter("claude_code").unwrap();
        let line = r#"{"type":"user","sessionId":"s1","message":{"role":"user","content":"   "}}"#;
        assert_eq!(a.normalize(line), Some(Vec::new()));
    }

    #[test]
    fn codex_developer_role_maps_to_user_turn() {
        let a = get_adapter("codex").unwrap();
        let line = r#"{"type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"sys note"}]}}"#;
        assert_eq!(
            a.normalize(line),
            Some(vec![Event::UserTurn {
                text: "sys note".into()
            }])
        );
    }

    #[test]
    fn prefilter_rejects_before_parse() {
        // A huge irrelevant line short-circuits without touching serde_json.
        let big = format!("{}\"type\":\"other\"", "x".repeat(300_000));
        assert!(!line_may_contain(&big, &[NEEDLE_CC_USER, NEEDLE_CC_ASSISTANT]));
    }

    const NEEDLE_CC_USER: &str = "\"type\":\"user\"";
    const NEEDLE_CC_ASSISTANT: &str = "\"type\":\"assistant\"";
}
