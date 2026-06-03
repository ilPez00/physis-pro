// Agent Registry: Tracks agent state, capabilities, and coherence scores
use std::collections::HashMap;

pub struct Agent {
    pub id: String,
    pub capabilities: Vec<String>,
    pub current_coherence: f32,
}

pub struct AgentRegistry {
    agents: HashMap<String, Agent>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self { agents: HashMap::new() }
    }

    pub fn register(&mut self, agent: Agent) {
        self.agents.insert(agent.id.clone(), agent);
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Agent> {
        self.agents.get_mut(id)
    }
}
