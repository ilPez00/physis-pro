use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};
use reqwest::Client;
use serde_json::json;

use crate::gantt::GanttScheduler;
use crate::graph::RawNodeKey;

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

impl Default for LifeOSTracker {
    fn default() -> Self {
        Self::new()
    }
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

    /// Create Gantt tasks for any synced goals not already in the scheduler.
    pub fn apply_to_scheduler(&self, scheduler: &mut GanttScheduler) {
        for goal in &self.goals {
            if !scheduler.has_task(&goal.id) {
                let mut hasher = DefaultHasher::new();
                goal.id.hash(&mut hasher);
                let node_key = RawNodeKey { data: hasher.finish() };
                // Default 1-hour duration; domain will adjust on recompute
                scheduler.add_task(
                    node_key,
                    goal.title.clone(),
                    3600.0,
                    0.0,
                    0,
                    vec![],
                );
            }
        }
    }

    /// Best-effort push of schedule drift notifications to Praxis and Agora.
    pub async fn push_schedule_drift(
        &self,
        praxis_url: &str,
        agora_url: &str,
        at_risk_tasks: &[String],
        health: f32,
    ) {
        let payload = json!({
            "type": "schedule_drift",
            "schedule_health": health,
            "at_risk_tasks": at_risk_tasks,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        let _ = self.client
            .post(format!("{}/api/v1/notifications", praxis_url))
            .json(&payload)
            .send()
            .await;
        let _ = self.client
            .post(format!("{}/api/v1/events/schedule", agora_url))
            .json(&payload)
            .send()
            .await;
    }
}
