//! Routing ladder (plan S3). One function decides which agent answers a
//! request, and it says why. It consumes the three things the plan names — a
//! `Capability`, the `RouteTable`, and the delegation `Cost`/`Policy` — rather
//! than inventing a second router.
//!
//! Rules the exit gate tests:
//! - An offline policy chooses local agents only.
//! - The cost ceiling is respected for automatic selection.
//! - An explicit `@agent` override wins over the automatic ladder.
//! - There is never a silent paid or remote substitution: if the only agent
//!   that could answer is not permitted or not reachable, the decision is
//!   `None` with a reason, not a different agent.

use serde::Serialize;

use super::{status, Agent, AgentKind, Roster, Transport};
use crate::capability::Capability;
use crate::ported::system_delegation::{Cost, Policy};
use crate::routes::RouteTable;

/// The cost tier a transport implies. Declarations, not measurements: a local
/// process is `Local`, the keyless Zen tier is `FreeTier`, a direct JEV key is
/// `Paid`.
pub fn cost_of(agent: &Agent) -> Cost {
    match agent.kind {
        AgentKind::Jev => match agent.transport {
            Transport::Jev => Cost::Paid,
            _ => Cost::FreeTier,
        },
        AgentKind::Hermes => Cost::FreeTier,
        AgentKind::Opencode | AgentKind::Verifier | AgentKind::Local | AgentKind::Human => {
            Cost::Local
        }
    }
}

/// Ladder rank: cheaper first. Used to order automatic candidates.
fn rank(cost: Cost) -> u8 {
    match cost {
        Cost::Local => 0,
        Cost::FreeTier => 1,
        Cost::Paid => 2,
    }
}

/// One routing request.
#[derive(Debug, Clone)]
pub struct RouteRequest<'a> {
    /// An explicit `@agent` override.
    pub agent: Option<&'a str>,
    /// The capability verb the caller wants (`read`, `inspect`, `execute`, …).
    pub verb: &'a str,
    /// The kind of object the verb acts on (`file`, `dir`, `repo`, …).
    pub object_kind: &'a str,
    /// The classified domain, when known. Domain is the strongest measured
    /// axis (`docs/WHAT_PHYSIS_IS.md` §2), so it is the reliable routing key.
    pub domain: Option<&'a str>,
    /// The classified mode, when known. Optional: a domain route still fires
    /// when the mode is unknown or misclassified.
    pub mode: Option<&'a str>,
    pub policy: Policy,
}

impl<'a> RouteRequest<'a> {
    pub fn new(verb: &'a str, object_kind: &'a str) -> Self {
        Self {
            agent: None,
            verb,
            object_kind,
            domain: None,
            mode: None,
            policy: Policy::default(),
        }
    }

    pub fn with_agent(mut self, agent: &'a str) -> Self {
        self.agent = Some(agent);
        self
    }

    /// Bind the classified region. `mode` is optional by design.
    pub fn with_region(mut self, domain: &'a str, mode: Option<&'a str>) -> Self {
        self.domain = Some(domain);
        self.mode = mode;
        self
    }

    pub fn with_policy(mut self, policy: Policy) -> Self {
        self.policy = policy;
        self
    }
}

/// The outcome. `agent: None` is a refusal, always with a reason.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Decision {
    pub agent: Option<String>,
    pub transport: String,
    pub cost: Option<String>,
    pub reason: String,
    /// Candidate ids considered for the automatic pick, ladder order.
    pub considered: Vec<String>,
    pub override_used: bool,
}

fn cost_name(cost: Cost) -> String {
    match cost {
        Cost::Local => "local",
        Cost::FreeTier => "free_tier",
        Cost::Paid => "paid",
    }
    .to_string()
}

impl Decision {
    fn refuse(reason: impl Into<String>) -> Self {
        Self {
            agent: None,
            transport: "unavailable".to_string(),
            cost: None,
            reason: reason.into(),
            considered: Vec::new(),
            override_used: false,
        }
    }

    /// One line for a CLI or a card.
    pub fn render(&self) -> String {
        match &self.agent {
            Some(id) => format!(
                "route: @{id} via {} ({}) — {}",
                self.transport,
                self.cost.as_deref().unwrap_or("?"),
                self.reason
            ),
            None => format!("route: unavailable — {}", self.reason),
        }
    }
}

/// Is the agent permitted by policy and reachable?
fn permitted(agent: &Agent, policy: &Policy) -> Result<Cost, String> {
    let cost = cost_of(agent);
    if !policy.costs.contains(&cost) {
        return Err(format!("cost {} not in policy", cost_name(cost)));
    }
    if agent.permissions.network && !policy.allow_remote_model {
        return Err("remote model not allowed by policy".to_string());
    }
    let s = status(agent);
    if !s.available {
        return Err(s.reason.unwrap_or_else(|| "unavailable".to_string()));
    }
    Ok(cost)
}

/// Decide which agent answers. Deterministic: same inputs, same decision.
pub fn choose(
    roster: &Roster,
    caps: &[Capability],
    table: &RouteTable,
    req: &RouteRequest,
) -> Decision {
    // The capability must exist: no capability means no route, not a guess.
    let cap = caps
        .iter()
        .find(|c| c.has_verb(req.verb) && c.supports_kind(req.object_kind));
    if cap.is_none() {
        return Decision::refuse(format!(
            "no capability for `{}` on `{}`",
            req.verb, req.object_kind
        ));
    }

    // An explicit override is the operator's decision and wins over the ladder.
    // It is still reported, so it is never a silent paid or remote choice.
    if let Some(name) = req.agent {
        let Some(agent) = roster.get(name) else {
            return Decision::refuse(format!("unknown agent @{name}"));
        };
        let cost = cost_of(agent);
        let s = status(agent);
        if !s.available {
            return Decision::refuse(format!(
                "@{name} unavailable: {} (no substitution for an explicit request)",
                s.reason.as_deref().unwrap_or("no transport")
            ));
        }
        let outside = if req.policy.costs.contains(&cost) {
            String::new()
        } else {
            format!(
                "; cost {} is outside the default policy (explicit)",
                cost_name(cost)
            )
        };
        return Decision {
            agent: Some(agent.id.clone()),
            transport: s.transport,
            cost: Some(cost_name(cost)),
            reason: format!("explicit override{outside}"),
            considered: vec![agent.id.clone()],
            override_used: true,
        };
    }

    // A route for the region can name a preferred agent. The exact cell is a
    // deliberate narrow binding, tried first; the domain route is the fallback
    // that still fires when the mode is unknown or misclassified, because
    // domain is the stronger measured axis.
    let mut preferred: Option<(String, &'static str)> = None;
    if let Some(d) = req.domain {
        if let Some(m) = req.mode {
            if let Some(a) = table
                .for_cell(d, m)
                .into_iter()
                .map(|r| r.action.trim().to_lowercase())
                .find(|a| roster.get(a).is_some())
            {
                preferred = Some((a, "cell"));
            }
        }
        if preferred.is_none() {
            if let Some(a) = table
                .for_domain(d)
                .into_iter()
                .map(|r| r.action.trim().to_lowercase())
                .find(|a| roster.get(a).is_some())
            {
                preferred = Some((a, "domain"));
            }
        }
    }

    // Automatic ladder: permitted, reachable agents, cheapest first.
    let mut considered: Vec<(u8, String, Cost, String)> = Vec::new();
    for agent in &roster.agents {
        if !agent.permissions.may(req.verb) {
            continue;
        }
        match permitted(agent, &req.policy) {
            Ok(cost) => {
                considered.push((rank(cost), agent.id.clone(), cost, status(agent).transport))
            }
            Err(_) => continue,
        }
    }
    considered.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    if considered.is_empty() {
        return Decision::refuse(format!(
            "no permitted agent for `{}` on `{}` (policy costs: {:?})",
            req.verb,
            req.object_kind,
            req.policy
                .costs
                .iter()
                .map(|c| cost_name(*c))
                .collect::<Vec<_>>()
        ));
    }

    // Prefer the route's named agent when it is among the permitted candidates.
    let preferred_id = preferred.as_ref().map(|(a, _)| a.clone());
    let pick = preferred_id
        .and_then(|p| considered.iter().find(|c| c.1 == p).cloned())
        .unwrap_or_else(|| considered[0].clone());

    let reason = match &preferred {
        Some((a, axis)) if *a == pick.1 => format!("route table preferred @{a} for the {axis}"),
        _ => format!(
            "ladder: cheapest permitted agent (cost {})",
            cost_name(pick.2)
        ),
    };

    Decision {
        agent: Some(pick.1.clone()),
        transport: pick.3.clone(),
        cost: Some(cost_name(pick.2)),
        reason,
        considered: considered.into_iter().map(|c| c.1).collect(),
        override_used: false,
    }
}

/// Hardware-aware routing (PH-101 Task 11): route the request's verb through
/// the core compute ladder (capable → preferred → scan-best → CPU) using a
/// live device scan. Companion to [`choose`], which routes the *agent*;
/// this routes the *compute*. Callers surface `rationale` next to the agent
/// decision so placement is cited, not assumed.
pub fn route_hardware(req: &RouteRequest) -> physis_core::cortex::router::RoutingDecision {
    use physis_core::cortex::router::{route, RoutingContext};
    use physis_core::devices::HardwareDiscovery;
    route(&RoutingContext {
        verb: req.verb.to_string(),
        preferred: physis_core::backend::BackendKind::from_env(),
        devices: HardwareDiscovery::scan(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::default_roster;
    use crate::capability::default_capabilities;

    fn offline() -> Policy {
        Policy {
            costs: vec![Cost::Local],
            allow_remote_model: false,
            timeout_seconds: 30,
        }
    }

    #[test]
    fn offline_policy_never_chooses_a_network_agent() {
        let roster = default_roster();
        let caps = default_capabilities();
        let table = RouteTable::new();
        let req = RouteRequest::new("read", "file").with_policy(offline());
        let d = choose(&roster, &caps, &table, &req);
        assert!(d.agent.is_some(), "{}", d.render());
        let id = d.agent.unwrap();
        let agent = roster.get(&id).unwrap();
        assert!(
            !agent.permissions.network,
            "chose a network agent offline: {id}"
        );
        assert_eq!(cost_of(agent), Cost::Local);
    }

    #[test]
    fn cost_ceiling_excludes_paid_agents() {
        let roster = default_roster();
        let caps = default_capabilities();
        let table = RouteTable::new();
        let free = Policy {
            costs: vec![Cost::Local, Cost::FreeTier],
            allow_remote_model: true,
            timeout_seconds: 30,
        };
        let d = choose(
            &roster,
            &caps,
            &table,
            &RouteRequest::new("read", "file").with_policy(free),
        );
        let rendered = d.render();
        let cost = d.cost.unwrap();
        assert_ne!(cost, "paid", "paid must be excluded: {rendered}");
    }

    #[test]
    fn explicit_override_wins_even_outside_the_default_policy() {
        let roster = default_roster();
        let caps = default_capabilities();
        let table = RouteTable::new();
        // The default policy would pick a local agent; the operator asks for jev.
        let d = choose(
            &roster,
            &caps,
            &table,
            &RouteRequest::new("read", "file")
                .with_agent("jev")
                .with_policy(offline()),
        );
        assert!(d.override_used);
        assert_eq!(d.agent.as_deref(), Some("jev"), "{}", d.render());
    }

    #[test]
    fn no_silent_paid_or_remote_substitution() {
        let roster = default_roster();
        let caps = default_capabilities();
        let table = RouteTable::new();
        // Only free-tier agents permitted, and every network agent is excluded.
        let policy = Policy {
            costs: vec![Cost::Local],
            allow_remote_model: false,
            timeout_seconds: 30,
        };
        let d = choose(
            &roster,
            &caps,
            &table,
            &RouteRequest::new("read", "file").with_policy(policy),
        );
        if let Some(id) = &d.agent {
            let a = roster.get(id).unwrap();
            assert_eq!(cost_of(a), Cost::Local);
            assert!(!a.permissions.network);
        }
    }

    #[test]
    fn unknown_capability_is_refused_not_guessed() {
        let roster = default_roster();
        let caps = default_capabilities();
        let table = RouteTable::new();
        let d = choose(
            &roster,
            &caps,
            &table,
            &RouteRequest::new("frobnicate", "file"),
        );
        assert!(d.agent.is_none());
        assert!(d.reason.contains("no capability"), "{}", d.reason);
    }

    #[test]
    fn unknown_override_is_refused() {
        let roster = default_roster();
        let caps = default_capabilities();
        let table = RouteTable::new();
        let d = choose(
            &roster,
            &caps,
            &table,
            &RouteRequest::new("read", "file").with_agent("nobody"),
        );
        assert!(d.agent.is_none());
        assert!(d.reason.contains("unknown agent"), "{}", d.reason);
    }

    fn local_and_free() -> Policy {
        Policy {
            costs: vec![Cost::Local, Cost::FreeTier],
            allow_remote_model: true,
            timeout_seconds: 30,
        }
    }

    #[test]
    fn route_table_prefers_a_narrow_cell_binding() {
        use crate::routes::AgentRoute;
        let roster = default_roster();
        let caps = default_capabilities();
        let mut table = RouteTable::new();
        table
            .upsert(AgentRoute {
                name: "verify-on-fabricate".into(),
                cell: "FABRICATE/WORK".into(),
                action: "verifier".into(),
                examples: vec![],
                enabled: true,
            })
            .unwrap();
        let d = choose(
            &roster,
            &caps,
            &table,
            &RouteRequest::new("read", "file")
                .with_region("FABRICATE", Some("WORK"))
                .with_policy(local_and_free()),
        );
        assert_eq!(d.agent.as_deref(), Some("verifier"), "{}", d.render());
        assert!(d.reason.contains("cell"), "{}", d.reason);
    }

    #[test]
    fn domain_route_is_the_fallback_when_the_mode_is_unknown() {
        use crate::routes::AgentRoute;
        let roster = default_roster();
        let caps = default_capabilities();
        let mut table = RouteTable::new();
        // A domain-level route (no mode). It must still fire when the mode is
        // unknown — the weaker axis must not be able to suppress the route.
        table
            .upsert(AgentRoute {
                name: "verify-on-fabricate".into(),
                cell: "FABRICATE".into(),
                action: "verifier".into(),
                examples: vec![],
                enabled: true,
            })
            .unwrap();
        let d = choose(
            &roster,
            &caps,
            &table,
            &RouteRequest::new("read", "file")
                .with_region("FABRICATE", None)
                .with_policy(local_and_free()),
        );
        assert_eq!(d.agent.as_deref(), Some("verifier"), "{}", d.render());
        assert!(d.reason.contains("domain"), "{}", d.reason);
    }

    #[test]
    fn hardware_routing_cites_its_ladder() {
        let d = route_hardware(&RouteRequest::new("embed", "file"));
        assert_eq!(
            d.backend,
            physis_core::backend::BackendKind::Cpu,
            "no device backend wired yet: {:?}",
            d.rationale
        );
        assert!(!d.rationale.is_empty(), "placement must show its work");
    }
}
