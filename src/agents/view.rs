//! Pure render strings for the agent surface. The HUD prints these; it does not
//! build them, so the same text is testable without a terminal.

use super::{status, Agent, AgentKind, Roster};

fn kind_label(kind: AgentKind) -> &'static str {
    match kind {
        AgentKind::Jev => "jev",
        AgentKind::Hermes => "hermes",
        AgentKind::Opencode => "opencode",
        AgentKind::Verifier => "verifier",
        AgentKind::Local => "local",
        AgentKind::Human => "human",
    }
}

/// The AGENTS strip: one name and an availability dot per agent, plus a legend
/// so the marker is never colour-only and never confused with the list's
/// `● changed`.
pub fn render_strip(roster: &Roster) -> String {
    let parts: Vec<String> = roster
        .agents
        .iter()
        .map(|a| {
            let dot = if status(a).available { "●" } else { "○" };
            format!("{} {}", a.name, dot)
        })
        .collect();
    format!("AGENTS: {} · ● ready ○ unavailable", parts.join("  "))
}

/// The agent card, as shown in a pane when an agent is selected or addressed.
/// `scope` is the current semantic context, stated so the card is never a
/// detached chatbot header.
pub fn render_agent_card(agent: &Agent, scope: Option<&str>) -> String {
    let s = status(agent);
    let mut out = String::new();
    out.push_str(&format!("{}\n\n", agent.name.to_uppercase()));
    out.push_str(&format!("state:     {}\n", agent.state.label()));
    out.push_str(&format!("kind:      {}\n", kind_label(agent.kind)));
    if let Some(host) = &agent.host {
        out.push_str(&format!("host:      {host}\n"));
    }
    if let Some(model) = &agent.model {
        out.push_str(&format!("model:     {model}\n"));
    }
    out.push_str(&format!("transport: {}\n", s.transport));
    if !s.available {
        out.push_str(&format!(
            "status:    ○ unavailable — {}\n",
            s.reason.as_deref().unwrap_or("no transport")
        ));
    }
    if let Some(scope) = scope {
        out.push_str(&format!("context:   {scope}\n"));
    }

    out.push_str("\ncapabilities:\n");
    if agent.capabilities.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for c in &agent.capabilities {
            out.push_str(&format!("  - {c}\n"));
        }
    }

    let p = &agent.permissions;
    out.push_str("\npermissions (deny by default):\n");
    out.push_str(&format!(
        "  read:   {}\n",
        if p.readable_kinds.is_empty() {
            "none".to_string()
        } else {
            p.readable_kinds.join(", ")
        }
    ));
    out.push_str(&format!(
        "  write:  {}\n",
        if p.writable_kinds.is_empty() {
            "none".to_string()
        } else {
            p.writable_kinds.join(", ")
        }
    ));
    out.push_str(&format!(
        "  verbs:  {}\n",
        if p.allowed_verbs.is_empty() {
            "none".to_string()
        } else {
            p.allowed_verbs.join(", ")
        }
    ));
    out.push_str(&format!("  network: {}\n", p.network));
    out
}

/// Card for an address that does not resolve to a roster entry.
pub fn render_unknown_agent(name: &str, roster: &Roster) -> String {
    let known: Vec<&str> = roster.agents.iter().map(|a| a.id.as_str()).collect();
    format!(
        "no such agent: @{name}\n\nknown agents:\n{}",
        known
            .iter()
            .map(|k| format!("  @{k}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::default_roster;

    #[test]
    fn strip_names_every_agent_with_a_dot_and_a_legend() {
        let strip = render_strip(&default_roster());
        assert!(strip.starts_with("AGENTS:"));
        assert!(strip.contains("JEV "));
        assert!(strip.contains("Verifier "));
        assert!(strip.contains('●') || strip.contains('○'));
        assert!(
            strip.contains("ready"),
            "legend must state the marker: {strip}"
        );
    }

    #[test]
    fn card_names_state_transport_and_scope() {
        let roster = default_roster();
        let card = render_agent_card(roster.get("opencode").unwrap(), Some("physis-core"));
        assert!(card.starts_with("OPENCODE"));
        assert!(card.contains("state:     idle"));
        assert!(card.contains("context:   physis-core"));
        assert!(card.contains("permissions"));
        assert!(card.contains("write:  file"));
    }

    #[test]
    fn card_reports_a_reason_when_unavailable() {
        let roster = default_roster();
        let mut agent = roster.get("hermes").unwrap().clone();
        agent.host = Some("pansa".to_string());
        let card = render_agent_card(&agent, None);
        if !super::status(&agent).available {
            assert!(card.contains("unavailable —"), "{card}");
        }
    }
}
