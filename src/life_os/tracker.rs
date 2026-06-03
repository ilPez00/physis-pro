use serde::{Deserialize, Serialize};
use reqwest::Client;

#[derive(Serialize, Deserialize, Clone)]
pub struct LifeOSGoal {
    pub id: String,
    pub title: String,
    pub coherence_target: f32,
    pub progress: f32,
}

pub struct LifeOSTracker {
    pub goals: Vec<LifeOSGoal>,
    client: Client,
}

impl LifeOSTracker {
    pub fn new() -> Self {
        Self { 
            goals: Vec::new(),
            client: Client::new(),
        }
    }

    // Sync is now a non-blocking method that returns a handle.
    // The web_server will handle the await.
    pub async fn sync_external_data(&mut self, praxis_url: &str, agora_url: &str) -> Result<(), String> {
        let praxis_res: Vec<LifeOSGoal> = self.client
            .get(format!("{}/api/v1/goals", praxis_url))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        
        self.goals = praxis_res;
        
        // Agora engagement sync
        let _agora: serde_json::Value = self.client
            .get(format!("{}/api/v1/analytics/engagement", agora_url))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
            
        Ok(())
    }
}
