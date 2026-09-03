use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagFile {
    pub path: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditedFile {
    pub path: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputSide {
    pub prompt: String,
    #[serde(default)]
    pub rag_files: Vec<RagFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputSide {
    pub text: String,
    #[serde(default)]
    pub edited_files: Vec<EditedFile>,
    #[serde(default)]
    pub deleted_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub run_id: Option<Uuid>,
    pub model: String,
    pub task: String,
    pub created_at: Option<DateTime<Utc>>,
    pub input: InputSide,
    pub output: OutputSide,
}

impl RunRecord {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("read run file {}", path.display()))?;
        let mut rec: Self = serde_json::from_str(&raw)
            .with_context(|| format!("parse run file {}", path.display()))?;
        if rec.run_id.is_none() {
            rec.run_id = Some(Uuid::new_v4());
        }
        if rec.created_at.is_none() {
            rec.created_at = Some(Utc::now());
        }
        Ok(rec)
    }
}
