//! Tool-using agent loop that orchestrates LLM calls with tool execution.
//!
//! The agent sends messages to the provider cascade, handles tool-call responses,
//! and loops until the model produces a final text response or the maximum
//! number of tool rounds is reached.

use std::collections::HashMap;
use std::sync::Arc;

use crate::ai::provider::{Content, ContentPart, ImageUrl, Message};
use crate::ai::tools::{ToolRegistry, call_tool};
use crate::ai::{AiError, AiResult, provider::ProviderCascade};
use serde::{Deserialize, Serialize};

const MAX_TOOL_ROUNDS: u32 = 8;

/// A tool result counts as a failure (for the PDCA rail) when it contains one of
/// these markers. Rough heuristic — only used to gate recalibration guidance.
fn looks_like_failure(result: &str) -> bool {
    let r = result.to_lowercase();
    ["error", "failed", "failure", "exception", "panic", "traceback", "not found", "❌"]
        .iter()
        .any(|m| r.contains(m))
}

/// A short, stable signature for a tool call so repeated identical failures can
/// be detected: tool name + the first 80 chars of its arguments.
fn failure_signature(tool_name: &str, args_raw: &str) -> String {
    let args_trim: String = args_raw.chars().take(80).collect();
    format!("{tool_name}::{args_trim}")
}

/// Configuration for an agent run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// System prompt prepended to every request.
    pub system_prompt: String,
    /// Maximum tokens for each LLM response.
    pub max_tokens: u32,
    /// Image detail level ("low", "high").
    pub detail: String,
    /// PDCA rails: when a tool call fails, inject self-assessment guidance and,
    /// on a repeated identical failure, force a strategy change instead of a
    /// blind retry. Reduces wasted rounds re-exploring a known-bad branch.
    pub pdca_rails: bool,
}

/// The result of an agent run.
#[derive(Debug, Clone)]
pub struct AgentOutput {
    /// The final text response from the LLM.
    pub text: String,
    /// Which provider served the final response.
    pub provider: String,
    /// Number of tool-call rounds executed.
    pub tool_rounds: u32,
    /// Approximate tokens used across all rounds.
    pub tokens_used: u32,
    /// How many times the PDCA rail forced a recalibration on a repeated failure.
    pub pdca_recalibrations: u32,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            system_prompt: String::new(),
            max_tokens: 1024,
            detail: "low".into(),
            pdca_rails: true,
        }
    }
}

/// A single observable step emitted while the agent runs. Streamed to clients
/// (e.g. the `/agent` dashboard over SSE) so the loop is visible, not a black
/// box. `kind` is one of: `plan` (assistant text), `tool_call`, `tool_result`,
/// `note` (PDCA rail guidance), `opencode` (a line of OpenCode subprocess
/// output), `final`, `error`, `done`.
#[derive(Debug, Clone)]
pub struct AgentEvent {
    pub round: u32,
    pub kind: String,
    pub text: String,
}

/// Sink for agent events. `Err(_)` from the sink (e.g. a dropped SSE channel)
/// is ignored by the loop so a disconnected client never kills the run.
pub type EventSink = Arc<dyn Fn(AgentEvent) + Send + Sync>;

/// Run the agent loop, streaming each step through `sink`.
///
/// This is the streaming twin of [`run_agent`]: same logic, same PDCA rails,
/// but every plan/tool-call/result is reported via `sink` as it happens. The
/// agent sends `query` (optionally with a base64 image `frame_b64`) to the
/// provider cascade; tool calls are executed via `ToolRegistry` and fed back;
/// the loop terminates when the model returns a plain text response or
/// `MAX_TOOL_ROUNDS` is exceeded.
#[allow(clippy::too_many_arguments)]
pub async fn run_agent_stream(
    provider_cascade: &ProviderCascade,
    tools: &ToolRegistry,
    config: &AgentConfig,
    history: &[Message],
    query: &str,
    frame_b64: Option<&str>,
    mode: &str,
    model_override: Option<&str>,
    sink: &EventSink,
) -> AiResult<AgentOutput> {
    let emit = |round: u32, kind: &str, text: &str| {
        sink(AgentEvent {
            round,
            kind: kind.to_string(),
            text: text.to_string(),
        });
    };
    let mut messages: Vec<Message> = Vec::new();

    messages.push(Message {
        role: "system".into(),
        content: Content::Text(config.system_prompt.clone()),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    });

    let mode_prefix = match mode {
        "SCOUT" => "[SCOUT MODE - brief observation]\n",
        "ANALYZE" => "[ANALYZE MODE - detailed analysis]\n",
        "DATA" => "[DATA MODE - structured output]\n",
        _ => "",
    };

    messages.extend_from_slice(history);

    let user_content = if let Some(b64) = frame_b64 {
        Content::Parts(vec![
            ContentPart {
                type_: "text".into(),
                text: Some(format!("{mode_prefix}{query}")),
                image_url: None,
            },
            ContentPart {
                type_: "image_url".into(),
                text: None,
                image_url: Some(ImageUrl {
                    url: format!("data:image/jpeg;base64,{b64}"),
                    detail: Some(config.detail.clone()),
                }),
            },
        ])
    } else {
        Content::Text(format!("{mode_prefix}{query}"))
    };

    messages.push(Message {
        role: "user".into(),
        content: user_content,
        tool_calls: None,
        tool_call_id: None,
        name: None,
    });

    let route = provider_cascade.classify(query, frame_b64.is_some());
    let tool_defs = tools.definitions();
    let defs_for_call = if tool_defs.is_empty() {
        None
    } else {
        Some(tool_defs)
    };

    let mut tool_rounds = 0u32;
    let mut total_tokens = 0u32;
    // PDCA rail state: count failures per call signature across the run.
    let mut failure_counts: HashMap<String, u32> = HashMap::new();
    let mut pdca_recalibrations = 0u32;

    loop {
        let (resp, provider_name) = if let Some(m) = model_override {
            let p = provider_cascade.providers_for_task(&route).into_iter()
                .find(|p| p.model() == m || p.name() == m)
                .ok_or_else(|| AiError::NoProvider)?;
            (p.complete(&messages, defs_for_call, Some(config.max_tokens)).await?, p.name().to_string())
        } else {
            provider_cascade
                .complete(&messages, defs_for_call, &route, Some(config.max_tokens))
                .await?
        };

        if let Some(u) = &resp.content {
            total_tokens += u.len() as u32;
            emit(tool_rounds, "plan", u);
        }

        messages.push(Message {
            role: "assistant".into(),
            content: Content::Text(resp.content.clone().unwrap_or_default()),
            tool_calls: resp.tool_calls.clone(),
            tool_call_id: None,
            name: None,
        });

        let calls = match resp.tool_calls {
            Some(c) if !c.is_empty() => c,
            _ => {
                let text = resp.content.unwrap_or_default();
                emit(tool_rounds, "final", &text);
                return Ok(AgentOutput {
                    text,
                    provider: provider_name,
                    tool_rounds,
                    tokens_used: total_tokens,
                    pdca_recalibrations,
                });
            }
        };

        tool_rounds += 1;
        if tool_rounds > MAX_TOOL_ROUNDS {
            emit(tool_rounds, "error", &format!("max tool rounds ({MAX_TOOL_ROUNDS}) reached"));
            return Err(AiError::MaxToolRounds);
        }

        // Collected PDCA guidance to append after this round's tool results.
        let mut pdca_notes: Vec<String> = Vec::new();

        for call in &calls {
            let args: serde_json::Value =
                serde_json::from_str(&call.function.arguments).unwrap_or(serde_json::json!({}));
            emit(
                tool_rounds,
                "tool_call",
                &format!(
                    "{} → {}",
                    call.function.name,
                    serde_json::to_string(&args).unwrap_or_default()
                ),
            );
            let result = call_tool(tools, &call.function.name, args).await;
            emit(tool_rounds, "tool_result", &result);

            // PDCA rail: on a failing tool result, escalate guidance — a light
            // CHECK note on the first failure, a hard recalibration directive
            // once the same call has failed repeatedly (stop blind retries).
            if config.pdca_rails && looks_like_failure(&result) {
                let sig = failure_signature(&call.function.name, &call.function.arguments);
                let count = failure_counts.entry(sig).or_insert(0);
                *count += 1;
                if *count >= 2 {
                    pdca_recalibrations += 1;
                    pdca_notes.push(format!(
                        "=== PDCA RECALIBRATION === `{}` has now failed {} times with the same arguments. \
                         Do NOT retry the same approach. Change strategy (different tool, different \
                         arguments, or revised plan), or stop and report the blocker.",
                        call.function.name, count
                    ));
                } else {
                    pdca_notes.push(format!(
                        "=== PDCA CHECK === `{}` failed. Before retrying, assess the cause and try a \
                         different approach rather than repeating the same call.",
                        call.function.name
                    ));
                }
            }

            messages.push(Message {
                role: "tool".into(),
                content: Content::Text(result),
                tool_calls: None,
                tool_call_id: Some(call.id.clone()),
                name: Some(call.function.name.clone()),
            });
        }

        if !pdca_notes.is_empty() {
            for note in &pdca_notes {
                emit(tool_rounds, "note", note);
            }
            messages.push(Message {
                role: "user".into(),
                content: Content::Text(pdca_notes.join("\n\n")),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });
        }
    }
}

/// Non-streaming wrapper around [`run_agent_stream`] — identical behaviour,
/// just discards the per-step events. Kept for callers that don't need a live
/// trace (tests, embedded API consumers).
#[allow(clippy::too_many_arguments)]
pub async fn run_agent(
    provider_cascade: &ProviderCascade,
    tools: &ToolRegistry,
    config: &AgentConfig,
    history: &[Message],
    query: &str,
    frame_b64: Option<&str>,
    mode: &str,
    model_override: Option<&str>,
) -> AiResult<AgentOutput> {
    let sink: EventSink = Arc::new(|_| {});
    run_agent_stream(
        provider_cascade,
        tools,
        config,
        history,
        query,
        frame_b64,
        mode,
        model_override,
        &sink,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_defaults() {
        let config = AgentConfig::default();
        assert_eq!(config.max_tokens, 1024);
        assert_eq!(config.detail, "low");
        assert_eq!(config.system_prompt, "");
    }

    #[test]
    fn test_agent_mode_prefixes() {
        let cases = [
            ("SCOUT", "[SCOUT MODE - brief observation]\nquery"),
            ("ANALYZE", "[ANALYZE MODE - detailed analysis]\nquery"),
            ("DATA", "[DATA MODE - structured output]\nquery"),
            ("OTHER", "query"),
        ];
        for (mode, expected) in &cases {
            let prefix = match *mode {
                "SCOUT" => "[SCOUT MODE - brief observation]\n",
                "ANALYZE" => "[ANALYZE MODE - detailed analysis]\n",
                "DATA" => "[DATA MODE - structured output]\n",
                _ => "",
            };
            assert_eq!(format!("{prefix}query"), *expected, "mode={mode}");
        }
    }

    #[test]
    fn test_agent_max_tool_rounds_constant() {
        assert_eq!(MAX_TOOL_ROUNDS, 8);
    }

    #[test]
    fn test_pdca_rails_default_on() {
        assert!(AgentConfig::default().pdca_rails);
    }

    #[test]
    fn test_looks_like_failure() {
        assert!(looks_like_failure("Error: file not found"));
        assert!(looks_like_failure("compilation FAILED"));
        assert!(looks_like_failure("panic at line 4"));
        assert!(looks_like_failure("❌ blocked"));
        assert!(!looks_like_failure("ok, wrote 3 files"));
        assert!(!looks_like_failure("success"));
    }

    #[test]
    fn test_failure_signature_stable_and_truncated() {
        let a = failure_signature("run", "{\"cmd\":\"build\"}");
        let b = failure_signature("run", "{\"cmd\":\"build\"}");
        assert_eq!(a, b);
        assert!(a.starts_with("run::"));
        let long = "x".repeat(200);
        let sig = failure_signature("t", &long);
        // name + "::" + 80 arg chars
        assert_eq!(sig.len(), "t::".len() + 80);
    }
}
