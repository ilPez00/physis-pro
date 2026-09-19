//! Structured output / slot extraction (Phase 19, B2).
//!
//! Routing tells a harness *where* to go; extraction tells it *what to fill
//! in*. This is the half LangChain and Semantic Router both have and the
//! engine did not: a schema-constrained extraction pass over the provider
//! cascade. It degrades honestly — no provider, no tool support, or a model
//! that returns nothing parseable all come back as an explicit note, never a
//! fabricated object.

use crate::ai::provider::{Content, Message, ProviderCascade, RouteHint};
use crate::ai::tools::{ToolDefinition, ToolFunction};

/// Extract fields from `text` conforming to `schema` (a JSON Schema object).
///
/// Two passes: tools first (the provider's native function-calling contract,
/// where supported), then a plain JSON-instruction prompt as fallback. Returns
/// the extracted object and the provider name that produced it.
pub async fn extract_slots(
    cascade: &ProviderCascade,
    text: &str,
    schema: &serde_json::Value,
) -> Result<(serde_json::Value, String), String> {
    let messages = build_messages(text, schema);
    let hint = RouteHint {
        task_type: "smart".into(),
        has_image: false,
    };

    let tool = ToolDefinition {
        type_: "function".into(),
        function: ToolFunction {
            name: "extract".into(),
            description: "Extract the requested fields from the input text as JSON".into(),
            parameters: schema.clone(),
        },
    };

    match cascade
        .complete(&messages, Some(&[tool]), &hint, Some(256))
        .await
    {
        Ok((resp, provider)) => {
            if let Some(calls) = resp.tool_calls {
                if let Some(call) = calls.first() {
                    if let Ok(v) =
                        serde_json::from_str::<serde_json::Value>(&call.function.arguments)
                    {
                        return Ok((v, provider));
                    }
                }
            }
            if let Some(content) = resp.content {
                if let Some(v) = parse_json_block(&content) {
                    return Ok((v, provider));
                }
            }
            Err("model returned no parseable slots".into())
        }
        Err(tools_err) => {
            // No tool-capable provider, or the tool call failed: ask plainly.
            match cascade.complete(&messages, None, &hint, Some(256)).await {
                Ok((resp, provider)) => {
                    let content = resp
                        .content
                        .ok_or_else(|| "provider returned no content".to_string())?;
                    let v = parse_json_block(&content)
                        .ok_or_else(|| "no parseable JSON in provider response".to_string())?;
                    Ok((v, provider))
                }
                Err(_) => Err(format!(
                    "extraction failed: no usable provider (tools pass: {tools_err:?})"
                )),
            }
        }
    }
}

fn build_messages(text: &str, schema: &serde_json::Value) -> Vec<Message> {
    let schema_text = serde_json::to_string_pretty(schema).unwrap_or_else(|_| "{}".into());
    vec![
        Message {
            role: "system".into(),
            content: Content::Text(
                "You extract structured data from input text. Return ONLY valid JSON \
                 matching the given JSON Schema. Do not add commentary or markdown."
                    .into(),
            ),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
        Message {
            role: "user".into(),
            content: Content::Text(format!(
                "JSON Schema:\n{schema_text}\n\nInput text:\n{text}"
            )),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
    ]
}

/// Parse a JSON object out of provider output, tolerating ```json fences and
/// surrounding prose (extract the first {...} span as a last resort).
fn parse_json_block(content: &str) -> Option<serde_json::Value> {
    let trimmed = content.trim();
    let fenced = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|s| s.strip_suffix("```"))
        .map(|s| s.trim())
        .unwrap_or(trimmed);
    serde_json::from_str(fenced).ok().or_else(|| {
        let start = fenced.find('{')?;
        let end = fenced.rfind('}')?;
        if end <= start {
            return None;
        }
        serde_json::from_str(&fenced[start..=end]).ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fenced_json() {
        let v = parse_json_block("```json\n{\"a\": 1}\n```").unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn parses_json_embedded_in_prose() {
        let v = parse_json_block("The answer is {\"machine\": \"press\", \"downtime_min\": 12}.")
            .unwrap();
        assert_eq!(v["downtime_min"], 12);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_json_block("no json here").is_none());
        assert!(parse_json_block("").is_none());
    }

    #[test]
    fn builds_schema_into_user_message() {
        let schema = serde_json::json!({"type": "object", "properties": {"x": {"type": "number"}}});
        let msgs = build_messages("some text", &schema);
        assert_eq!(msgs.len(), 2);
        match &msgs[1].content {
            Content::Text(t) => assert!(t.contains("JSON Schema:")),
            _ => panic!("expected text content"),
        }
    }
}
