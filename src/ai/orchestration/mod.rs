pub mod agent_registry;

use crate::ai::orchestration::agent_registry::{Agent, AgentRegistry};
use crate::models::CoherenceRating;

pub struct Orchestrator {
    pub registry: AgentRegistry,
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            registry: AgentRegistry::new(),
        }
    }

    /// Update coherence and decide if agent needs re-alignment
    pub fn update_agent_coherence(&mut self, agent_id: &str, rating: CoherenceRating) {
        if let Some(agent) = self.registry.get_mut(agent_id) {
            match rating {
                CoherenceRating::Success => agent.current_coherence += 0.1,
                CoherenceRating::Failure => agent.current_coherence -= 0.5,
                CoherenceRating::Inert => agent.current_coherence -= 0.05,
            }
            agent.current_coherence = agent.current_coherence.clamp(-1.0, 1.0);
        }
    }
}
