//! Agents as first-class objects (plan S1). An agent is data: a roster entry in
//! `config/agents.json` (identity, capabilities, permissions, transport). What
//! it *is doing* is observed state, tracked by the lifecycle state machine.
//!
//! This module owns no execution. Adapters (S2) invoke the real processes; here
//! we only describe, list, address and transition agents. Availability is probed
//! honestly from the environment, so an absent binary or key reports a reason
//! rather than a silent success.

pub mod view;

use serde::{Deserialize, Serialize};

/// Which agent this is. The kind fixes the default transport story and the
/// shape of the card; it is not a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    Jev,
    Hermes,
    Opencode,
    Verifier,
    Local,
    Human,
}

/// Lifecycle state. Observed, not configured: the roster's initial value is the
/// starting point, transitions move it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Idle,
    Active,
    Paused,
    Archived,
    Killed,
}

impl AgentState {
    pub fn label(&self) -> &'static str {
        match self {
            AgentState::Idle => "idle",
            AgentState::Active => "active",
            AgentState::Paused => "paused",
            AgentState::Archived => "archived",
            AgentState::Killed => "killed",
        }
    }

    /// Legal transitions. A killed agent stays killed; an archived agent can
    /// only be killed. Everything else can be paused, resumed, archived or
    /// killed. This is the rule the exit gate tests.
    pub fn can_transition_to(self, next: AgentState) -> bool {
        use AgentState::*;
        match self {
            Killed => false,
            Archived => matches!(next, Killed),
            Idle => matches!(next, Active | Paused | Archived | Killed),
            Active => matches!(next, Idle | Paused | Archived | Killed),
            Paused => matches!(next, Idle | Active | Archived | Killed),
        }
    }
}

/// How the agent is reached. Shown on the card, never assumed: `Zen` and `Cli`
/// are different claims about what is actually answering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    /// No transport configured.
    #[default]
    None,
    /// OpenCode Zen free tier (`oczen/*`).
    Zen,
    /// A CLI on PATH (`hermes`, `opencode`).
    Cli,
    /// A direct JEV service key.
    Jev,
    /// In-process, no network.
    Local,
}

/// Deny by default: empty lists mean nothing is allowed, `network` is false.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Permission {
    #[serde(default)]
    pub readable_kinds: Vec<String>,
    #[serde(default)]
    pub writable_kinds: Vec<String>,
    #[serde(default)]
    pub allowed_verbs: Vec<String>,
    #[serde(default)]
    pub hosts: Vec<String>,
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub max_cost: Option<f64>,
}

impl Permission {
    pub fn may_read(&self, kind: &str) -> bool {
        self.readable_kinds.iter().any(|k| k == kind)
    }

    pub fn may_write(&self, kind: &str) -> bool {
        self.writable_kinds.iter().any(|k| k == kind)
    }

    pub fn may(&self, verb: &str) -> bool {
        self.allowed_verbs.iter().any(|v| v == verb)
    }
}

/// One agent. `id` is the addressable handle (`@jev`), `name` the display form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub kind: AgentKind,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub transport: Transport,
    #[serde(default = "default_state")]
    pub state: AgentState,
    #[serde(default)]
    pub permissions: Permission,
}

fn default_state() -> AgentState {
    AgentState::Idle
}

/// A probed availability claim. `available=false` always carries a reason, so a
/// missing key or binary is visible instead of guessed.
#[derive(Debug, Clone, PartialEq)]
pub struct Status {
    pub available: bool,
    /// The transport that would actually carry the call, labelled for display.
    pub transport: String,
    pub reason: Option<String>,
}

/// Is `bin` on PATH? No subprocess: split PATH and stat.
fn on_path(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|p| p.join(bin).is_file()))
        .unwrap_or(false)
}

fn jev_zen_model() -> String {
    std::env::var("PHYSIS_JEV_ZEN_MODEL")
        .ok()
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "oczen/laguna-s-2.1-free".to_string())
}

fn jev_key_present() -> bool {
    ["JEV_API_KEY", "PHYSIS_JEV_API_KEY"]
        .iter()
        .any(|k| std::env::var(k).map(|v| !v.is_empty()).unwrap_or(false))
}

/// Probe what is actually reachable for this agent. Pure with respect to the
/// process environment: no side effects, no mutation.
pub fn status(agent: &Agent) -> Status {
    match agent.kind {
        AgentKind::Jev => {
            if jev_key_present() {
                Status {
                    available: true,
                    transport: format!("jev:{}", agent.model.clone().unwrap_or_else(jev_zen_model)),
                    reason: None,
                }
            } else if on_path("opencode") {
                Status {
                    available: true,
                    transport: format!("zen:{}", jev_zen_model()),
                    reason: None,
                }
            } else {
                Status {
                    available: false,
                    transport: "unavailable".to_string(),
                    reason: Some("no JEV key and no opencode on PATH".to_string()),
                }
            }
        }
        AgentKind::Hermes => cli_status("hermes"),
        AgentKind::Opencode => cli_status("opencode"),
        AgentKind::Verifier | AgentKind::Local | AgentKind::Human => Status {
            available: true,
            transport: "local".to_string(),
            reason: None,
        },
    }
}

fn cli_status(bin: &str) -> Status {
    if on_path(bin) {
        Status {
            available: true,
            transport: format!("cli:{bin}"),
            reason: None,
        }
    } else {
        Status {
            available: false,
            transport: "unavailable".to_string(),
            reason: Some(format!("{bin} not on PATH")),
        }
    }
}

/// The agent roster: `config/agents.json`, versioned, loaded as data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Roster {
    #[serde(default = "default_roster_version")]
    pub version: u32,
    pub agents: Vec<Agent>,
}

fn default_roster_version() -> u32 {
    1
}

impl Roster {
    pub fn parse(json: &str) -> Result<Self, String> {
        let roster: Roster = serde_json::from_str(json).map_err(|e| format!("agents.json: {e}"))?;
        if roster.version != 1 {
            return Err(format!(
                "agents.json: unsupported version {} (this build reads 1)",
                roster.version
            ));
        }
        Ok(roster)
    }

    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text)
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn get(&self, id: &str) -> Option<&Agent> {
        self.agents.iter().find(|a| a.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Agent> {
        self.agents.iter_mut().find(|a| a.id == id)
    }
}

/// The roster this binary ships with, compiled in so a missing file cannot
/// silently produce an empty agent list.
pub fn default_roster() -> Roster {
    const AGENTS_JSON: &str = include_str!("../../config/agents.json");
    Roster::parse(AGENTS_JSON).expect("config/agents.json is compiled in and must parse")
}

/// Move an agent to `next`, or refuse with a reason. Refusal is the point: an
/// illegal transition is reported, not applied.
pub fn transition(agent: &mut Agent, next: AgentState) -> Result<(), String> {
    if agent.state == next {
        return Ok(());
    }
    if !agent.state.can_transition_to(next) {
        return Err(format!(
            "illegal transition for {}: {} -> {}",
            agent.id,
            agent.state.label(),
            next.label()
        ));
    }
    agent.state = next;
    Ok(())
}

/// Parse an `@agent` address. Returns `(agent, host, rest)`:
/// `@jev inspect this` -> ("jev", None, "inspect this");
/// `@hermes@omo inspect` -> ("hermes", Some("omo"), "inspect").
pub fn parse_agent_address(text: &str) -> Option<(String, Option<String>, String)> {
    let rest_of = text.trim().strip_prefix('@')?;
    let mut it = rest_of.splitn(2, char::is_whitespace);
    let head = it.next().unwrap_or("");
    let rest = it.next().unwrap_or("").trim().to_string();
    if head.is_empty() {
        return None;
    }
    match head.split_once('@') {
        Some((name, host)) if !name.is_empty() && !host.is_empty() => {
            Some((name.to_string(), Some(host.to_string()), rest))
        }
        _ => Some((head.to_string(), None, rest)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roster_round_trips_through_json() {
        let roster = default_roster();
        assert!(roster.get("jev").is_some());
        assert!(roster.get("opencode").is_some());

        let text = serde_json::to_string(&roster).unwrap();
        let back = Roster::parse(&text).unwrap();
        assert_eq!(roster, back);
    }

    #[test]
    fn malformed_or_wrong_version_roster_is_an_error() {
        assert!(Roster::parse("{ not json").is_err());
        assert!(Roster::parse("{\"version\":9,\"agents\":[]}").is_err());
    }

    #[test]
    fn illegal_lifecycle_transition_is_refused() {
        let mut roster = default_roster();
        let a = roster.get_mut("jev").unwrap();

        transition(a, AgentState::Killed).unwrap();
        let err = transition(a, AgentState::Active).unwrap_err();
        assert!(err.contains("illegal transition"), "{err}");

        // Archived can only be killed.
        let mut roster2 = default_roster();
        let b = roster2.get_mut("hermes").unwrap();
        transition(b, AgentState::Archived).unwrap();
        assert!(transition(b, AgentState::Active).is_err());
        assert!(transition(b, AgentState::Killed).is_ok());
    }

    #[test]
    fn permissions_deny_by_default() {
        let p = Permission::default();
        assert!(!p.network);
        assert!(!p.may_write("file"));
        assert!(!p.may("execute"));

        let roster = default_roster();
        let jev = roster.get("jev").unwrap();
        assert!(!jev.permissions.may_write("file"), "JEV must not write");
        assert!(jev.permissions.may_read("file"));
    }

    #[test]
    fn address_parsing_covers_scope_and_host() {
        assert_eq!(
            parse_agent_address("@jev inspect this"),
            Some(("jev".into(), None, "inspect this".into()))
        );
        assert_eq!(
            parse_agent_address("@hermes@omo inspect worker"),
            Some(("hermes".into(), Some("omo".into()), "inspect worker".into()))
        );
        assert_eq!(parse_agent_address("no at sign"), None);
        assert_eq!(parse_agent_address("@"), None);
    }

    #[test]
    fn status_is_honest_about_a_missing_transport() {
        let mut agent = default_roster().get("jev").cloned().unwrap();
        agent.kind = AgentKind::Hermes;
        // Whether hermes is on PATH here or not, the claim must carry a reason
        // when unavailable and never claim availability with no transport.
        let s = status(&agent);
        if s.available {
            assert!(s.transport != "unavailable");
        } else {
            assert!(s.reason.is_some());
            assert_eq!(s.transport, "unavailable");
        }
    }
}
