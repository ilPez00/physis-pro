//! The `@` grammar (plan S5).
//!
//! One parser for the magic textbox, so the HUD and any other frontend agree on
//! what an agent line means. Parsing is total: every input is either a
//! well-formed call or an error naming what was wrong — never a silent fallback
//! to a search.
//!
//! Grammar:
//!
//! ```text
//! call    := "@all" request
//!          | stage ("|" stage)+
//!          | stage
//! stage   := "@" agent ("@" host)? request
//! request := quoted | free text
//! ```
//!
//! `@all` is only valid alone: a broadcast cannot be one stage of a pipeline,
//! because there is no single output for the next stage to consume.

/// One stage of a call: an agent (optionally on a host) and what it is asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage {
    pub agent: String,
    pub host: Option<String>,
    pub request: String,
}

/// A parsed agent line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentCall {
    /// `@agent[@host] request`
    Single(Stage),
    /// `@all request`
    All { request: String },
    /// `@a x | @b y` — stages run in order.
    Pipeline { stages: Vec<Stage> },
}

impl AgentCall {
    /// The agents this call addresses, in order.
    pub fn agents(&self) -> Vec<&str> {
        match self {
            AgentCall::Single(s) => vec![s.agent.as_str()],
            AgentCall::All { .. } => Vec::new(),
            AgentCall::Pipeline { stages } => stages.iter().map(|s| s.agent.as_str()).collect(),
        }
    }

    /// A one-line description shown before dispatch, so the operator sees the
    /// interpretation rather than guessing it.
    pub fn describe(&self) -> String {
        match self {
            AgentCall::Single(s) => match &s.host {
                Some(h) => format!("@{}@{} {}", s.agent, h, s.request)
                    .trim_end()
                    .to_string(),
                None => format!("@{} {}", s.agent, s.request).trim_end().to_string(),
            },
            AgentCall::All { request } => format!("@all {request}").trim_end().to_string(),
            AgentCall::Pipeline { stages } => stages
                .iter()
                .map(|s| match &s.host {
                    Some(h) => format!("@{}@{} {}", s.agent, h, s.request),
                    None => format!("@{} {}", s.agent, s.request),
                })
                .collect::<Vec<_>>()
                .join(" | "),
        }
    }
}

/// Parse an agent line. The text must start with `@`.
pub fn parse(text: &str) -> Result<AgentCall, String> {
    let t = text.trim();
    if !t.starts_with('@') {
        return Err("an agent call starts with @".to_string());
    }
    let raw = split_stages(t)?;
    if raw.len() == 1 {
        let stage = parse_stage(&raw[0])?;
        if stage.agent == "all" {
            return Ok(AgentCall::All {
                request: stage.request,
            });
        }
        return Ok(AgentCall::Single(stage));
    }
    let mut stages = Vec::new();
    for s in &raw {
        let stage = parse_stage(s)?;
        if stage.agent == "all" {
            return Err("@all cannot be one stage of a pipeline".to_string());
        }
        stages.push(stage);
    }
    Ok(AgentCall::Pipeline { stages })
}

/// Split on `|`, ignoring pipes inside quotes. An unclosed quote is an error,
/// not a guess about where the string ended.
fn split_stages(text: &str) -> Result<Vec<String>, String> {
    let mut stages = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for ch in text.chars() {
        match ch {
            '"' => {
                in_quote = !in_quote;
                cur.push(ch);
            }
            '|' if !in_quote => {
                stages.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    if in_quote {
        return Err("unclosed quote".to_string());
    }
    stages.push(cur.trim().to_string());
    Ok(stages)
}

fn parse_stage(stage: &str) -> Result<Stage, String> {
    let s = stage.trim();
    let rest = s
        .strip_prefix('@')
        .ok_or_else(|| format!("pipeline stage must start with @: {s:?}"))?;
    let mut it = rest.splitn(2, char::is_whitespace);
    let head = it.next().unwrap_or("");
    let request = it.next().unwrap_or("").trim().to_string();
    if head.is_empty() {
        return Err("empty agent name".to_string());
    }
    let (agent, host) = match head.split_once('@') {
        Some((a, h)) if !a.is_empty() && !h.is_empty() => (a.to_string(), Some(h.to_string())),
        Some(_) => return Err(format!("malformed agent@host: {head:?}")),
        None => (head.to_string(), None),
    };
    Ok(Stage {
        agent,
        host,
        request: strip_quotes(&request),
    })
}

/// A fully quoted request loses its quotes; anything else is left as written.
fn strip_quotes(s: &str) -> String {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn single(agent: &str, host: Option<&str>, request: &str) -> AgentCall {
        AgentCall::Single(Stage {
            agent: agent.into(),
            host: host.map(|h| h.into()),
            request: request.into(),
        })
    }

    #[test]
    fn grammar_table() {
        // Valid: bare, host-scoped, quoted, all, pipeline.
        assert_eq!(
            parse("@jev inspect").unwrap(),
            single("jev", None, "inspect")
        );
        assert_eq!(
            parse("@hermes@omo inspect worker").unwrap(),
            single("hermes", Some("omo"), "inspect worker")
        );
        assert_eq!(
            parse(r#"@jev "the auth flow""#).unwrap(),
            single("jev", None, "the auth flow")
        );
        assert_eq!(
            parse("@all summarise the day").unwrap(),
            AgentCall::All {
                request: "summarise the day".into()
            }
        );
        assert_eq!(
            parse("@jev inspect | @verifier check").unwrap(),
            AgentCall::Pipeline {
                stages: vec![
                    Stage {
                        agent: "jev".into(),
                        host: None,
                        request: "inspect".into()
                    },
                    Stage {
                        agent: "verifier".into(),
                        host: None,
                        request: "check".into()
                    },
                ]
            }
        );
    }

    #[test]
    fn malformed_is_an_error_with_a_reason() {
        for (input, needle) in [
            ("not an agent", "starts with @"),
            ("@", "empty agent name"),
            ("@jev@", "malformed agent@host"),
            ("@jev a | broken", "must start with @"),
            ("@jev \"unclosed", "unclosed quote"),
            ("@all x | @jev y", "cannot be one stage"),
        ] {
            let err = parse(input).unwrap_err();
            assert!(err.contains(needle), "input {input:?} gave {err:?}");
        }
    }

    #[test]
    fn quoted_pipes_do_not_split_a_pipeline() {
        let call = parse(r#"@jev "compare a | b""#).unwrap();
        assert_eq!(call, single("jev", None, "compare a | b"));
        assert_eq!(call.agents(), vec!["jev"]);
    }

    #[test]
    fn describe_shows_the_interpretation() {
        assert_eq!(parse("@jev inspect").unwrap().describe(), "@jev inspect");
        assert_eq!(
            parse("@hermes@omo go").unwrap().describe(),
            "@hermes@omo go"
        );
        assert_eq!(parse("@a x | @b y").unwrap().describe(), "@a x | @b y");
    }
}
