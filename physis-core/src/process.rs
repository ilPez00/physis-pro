//! Process semantics for industrial, operational, and reasoning workflows.
//!
//! Provides first-class models for:
//!   - Goal, Plan, Task, Process
//!   - Event, State, Transition
//!   - Resource, Dependency, Constraint
//!   - Measurement, Outcome, Failure, Intervention
//!
//! Bridges the loop:
//!   PLAN → ACTION → OBSERVATION → DEVIATION → INTERVENTION → OUTCOME

use serde::{Deserialize, Serialize};

use crate::models::Score;
use crate::temporal::TemporalValidity;

/// Operational or strategic goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessGoal {
    pub id: String,
    pub title: String,
    pub description: String,
    pub target_metric: Option<String>,
    pub target_value: Option<f64>,
    pub achieved: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// A structured plan containing tasks and dependencies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessPlan {
    pub id: String,
    pub goal_id: Option<String>,
    pub name: String,
    pub tasks: Vec<ProcessTask>,
    pub constraints: Vec<ProcessConstraint>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// A task within a process or plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessTask {
    pub id: String,
    pub title: String,
    pub state: TaskState,
    pub assigned_resources: Vec<String>,
    pub dependencies: Vec<String>,
    pub expected_duration_secs: Option<u64>,
}

/// State of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Pending,
    InProgress,
    Completed,
    Blocked,
    Failed,
    Intervened,
}

/// A discrete state of a system, machine, or process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessState {
    pub id: String,
    pub name: String,
    pub parameters: std::collections::HashMap<String, f64>,
    pub active_since: chrono::DateTime<chrono::Utc>,
    pub valid_until: Option<chrono::DateTime<chrono::Utc>>,
}

/// A state transition caused by an event or action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub id: String,
    pub from_state: String,
    pub to_state: String,
    pub trigger_event: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub duration_secs: Option<f64>,
}

/// Resource used in a process (machine, operator, material, tooling).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessResource {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub capacity: f64,
    pub current_utilization: f64,
}

/// Operational constraint (e.g. max temperature, deadline, dependency).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessConstraint {
    pub id: String,
    pub name: String,
    pub metric: String,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub is_hard_constraint: bool,
}

/// A physical or operational measurement recorded during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMeasurement {
    pub id: String,
    pub metric: String,
    pub value: f64,
    pub unit: String,
    pub machine_or_source: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub is_nominal: bool,
}

/// Observed deviation from plan or expected nominal state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessDeviation {
    pub id: String,
    pub task_or_process_id: String,
    pub metric: String,
    pub expected_value: f64,
    pub observed_value: f64,
    pub severity: Score,
    pub detected_at: chrono::DateTime<chrono::Utc>,
    pub description: String,
}

/// An intervention performed in response to a deviation or failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessIntervention {
    pub id: String,
    pub deviation_id: Option<String>,
    pub operator: String,
    pub action_taken: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub expected_recovery: String,
    pub actual_recovery: Option<String>,
    pub resolved: bool,
}

/// Final observed outcome of a process cycle or PDCA iteration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessOutcome {
    pub id: String,
    pub plan_id: Option<String>,
    pub success: bool,
    pub output_produced: Option<String>,
    pub scrap_or_error_count: usize,
    pub completion_time: chrono::DateTime<chrono::Utc>,
    pub summary: String,
}

/// Full execution trace of the PDCA / Lean cycle:
/// PLAN → ACTION → OBSERVATION → DEVIATION → INTERVENTION → OUTCOME
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessCycle {
    pub id: String,
    pub plan: Option<ProcessPlan>,
    pub measurements: Vec<ProcessMeasurement>,
    pub deviations: Vec<ProcessDeviation>,
    pub interventions: Vec<ProcessIntervention>,
    pub outcome: Option<ProcessOutcome>,
    pub temporal: Option<TemporalValidity>,
}

impl ProcessCycle {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            plan: None,
            measurements: Vec::new(),
            deviations: Vec::new(),
            interventions: Vec::new(),
            outcome: None,
            temporal: Some(TemporalValidity::permanent()),
        }
    }

    pub fn record_measurement(&mut self, measurement: ProcessMeasurement) {
        self.measurements.push(measurement);
    }

    pub fn record_deviation(&mut self, deviation: ProcessDeviation) {
        self.deviations.push(deviation);
    }

    pub fn record_intervention(&mut self, intervention: ProcessIntervention) {
        self.interventions.push(intervention);
    }

    pub fn complete_outcome(&mut self, outcome: ProcessOutcome) {
        self.outcome = Some(outcome);
    }
}
