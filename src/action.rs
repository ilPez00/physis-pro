//! The action surface: one parser and one renderer shared by the CLI and the
//! HUD, so an action resolves through the same [`Registry`] path in both. All
//! logic lives here; frontends only print the strings.

use std::collections::HashMap;

use serde_json::Value;

use crate::capability::ObservationRecord;
use crate::registry::{CallResult, Registry};

/// One parsed action line: `<verb> <object> [key=value ...]`.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionLine {
    pub verb: String,
    pub object: String,
    pub kwargs: HashMap<String, Value>,
}

/// Parse is total: every input is either a well-formed line or an error naming
/// what was wrong. There is no silent default object or verb.
pub fn parse_action_line(text: &str) -> Result<ActionLine, String> {
    let mut it = text.split_whitespace();
    let verb = it
        .next()
        .ok_or_else(|| "usage: <verb> <object> [key=value ...]".to_string())?;
    let object = it
        .next()
        .ok_or_else(|| "usage: <verb> <object> [key=value ...]".to_string())?;
    let mut kwargs = HashMap::new();
    for tok in it {
        let (k, v) = tok
            .split_once('=')
            .ok_or_else(|| format!("bad argument `{tok}` (expected key=value)"))?;
        // A text box carries no types, but the executors read typed kwargs
        // (`limit` as u64, `confirm` as bool, `args` as array). Parse the value
        // as JSON when it is JSON and fall back to a plain string otherwise, so
        // `limit=100` and `confirm=true` mean what they say.
        let value =
            serde_json::from_str::<Value>(v).unwrap_or_else(|_| Value::String(v.to_string()));
        kwargs.insert(k.to_string(), value);
    }
    Ok(ActionLine {
        verb: verb.to_string(),
        object: object.to_string(),
        kwargs,
    })
}

fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Render one call plus the observation it appended. The observation is shown
/// because the registry's own record is the provenance, not a hidden side
/// effect.
pub fn render_call(
    line: &ActionLine,
    result: &CallResult,
    obs: Option<&ObservationRecord>,
) -> String {
    let status = if result.ok { "ok" } else { "FAILED" };
    let mut out = format!(
        "[{status}] {} {} -> capability={} validation={}",
        line.verb, line.object, result.capability, result.validation
    );
    if let Some(err) = &result.error {
        out.push_str(&format!(" error={err}"));
    }
    if let Some(obs) = obs {
        out.push_str(&format!(
            " | obs ok={} reliability={:?}",
            obs.ok, obs.reliability
        ));
    }
    if let Some(v) = &result.result {
        let text = value_text(v);
        if !text.is_empty() {
            out.push('\n');
            out.push_str(&text);
        }
    }
    out
}

/// Parse and dispatch one line through the registry. Returns the structured
/// result (for tests and callers) and the rendered string (for frontends).
pub fn dispatch(reg: &mut Registry, text: &str) -> Result<(CallResult, String), String> {
    let line = parse_action_line(text)?;
    let result = reg.call(&line.object, &line.verb, line.kwargs.clone());
    let obs = reg.obs_tail(1).into_iter().next();
    let rendered = render_call(&line, &result, obs.as_ref());
    Ok((result, rendered))
}

/// `action list`: capabilities first, then registered objects.
pub fn render_list(reg: &Registry) -> String {
    let mut out = String::from("CAPABILITIES\n");
    for (name, verbs, risk, tier) in reg.list_capabilities() {
        out.push_str(&format!(
            "  {name:<16} verbs={:<24} risk={risk:<30} tier={tier}\n",
            verbs.join(",")
        ));
    }
    let objects = reg.list_objects();
    out.push_str(&format!("\nOBJECTS ({})\n", objects.len()));
    for (oid, kind, path) in objects.iter().take(200) {
        out.push_str(&format!("  {oid:<28} {kind:<10} {path}\n"));
    }
    if objects.len() > 200 {
        out.push_str(&format!("  … {} more\n", objects.len() - 200));
    }
    out
}

/// `action show <capability>`.
pub fn render_capability(reg: &Registry, name: &str) -> String {
    match reg.get_capability(name) {
        Some(c) => format!(
            "{}\n  kinds: {}\n  verbs: {}\n  tier: {}\n  risk: {}\n  cost: {:?}\n  reliability: mean={:.3} alpha={} beta={}\n  undo_link: {:?}",
            c.name,
            c.object_kinds.join(", "),
            c.verbs.join(", "),
            c.tier,
            c.risk,
            c.cost,
            c.reliability_mean(),
            c.reliability_alpha,
            c.reliability_beta,
            c.undo_link,
        ),
        None => format!("no such capability: {name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_is_total() {
        let l = parse_action_line("read fs:abc").unwrap();
        assert_eq!(l.verb, "read");
        assert_eq!(l.object, "fs:abc");
        assert!(l.kwargs.is_empty());

        let l = parse_action_line("read fs:abc limit=100").unwrap();
        assert_eq!(l.kwargs.get("limit"), Some(&Value::Number(100.into())));

        let l = parse_action_line("execute fs:abc confirm=true cmd=echo").unwrap();
        assert_eq!(l.kwargs.get("confirm"), Some(&Value::Bool(true)));
        assert_eq!(l.kwargs.get("cmd"), Some(&Value::String("echo".into())));

        assert!(parse_action_line("").is_err());
        assert!(parse_action_line("read").is_err());
        assert!(parse_action_line("read fs:abc limit").is_err());
    }
}
