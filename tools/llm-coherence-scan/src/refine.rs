use crate::capture::RunRecord;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const HALF_LIFE_SECS: i64 = 60 * 60 * 24 * 7;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalienceEntry {
    pub cell: String,
    pub raw_score: f32,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl SalienceEntry {
    pub fn decayed(&self, now: chrono::DateTime<chrono::Utc>) -> f32 {
        let dt = (now - self.updated_at).num_seconds().max(0) as f32;
        let factor = 0.5_f32.powf(dt / HALF_LIFE_SECS as f32);
        self.raw_score * factor
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SalienceStore {
    pub entries: HashMap<String, SalienceEntry>,
}

impl SalienceStore {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("read salience {}", path.display()))?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("mkdir for salience {}", path.display()))?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        std::fs::write(path, raw).with_context(|| format!("write salience {}", path.display()))?;
        Ok(())
    }

    pub fn bump(&mut self, cell: &str, delta: f32, now: chrono::DateTime<chrono::Utc>) {
        let entry = self
            .entries
            .entry(cell.to_string())
            .or_insert(SalienceEntry {
                cell: cell.to_string(),
                raw_score: 0.0,
                updated_at: now,
            });
        entry.raw_score += delta;
        entry.updated_at = now;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamReport {
    pub silent_failures: Vec<String>,
    pub asserted_failures: Vec<String>,
    pub iterations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryProposal {
    pub id: String,
    pub proposed_cell: String,
    pub evidence_path: String,
    pub evidence_excerpt: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ProposalStore {
    pub pending: Vec<DiscoveryProposal>,
}

impl ProposalStore {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("read proposals {}", path.display()))?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

pub fn cell_for(path: &str) -> String {
    let p = Path::new(path);
    let ext = p
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    let stem = p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    format!("{ext}::{stem}")
}

pub fn update_outcome_salience(
    rec: &RunRecord,
    store: &mut SalienceStore,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    for edited in &rec.output.edited_files {
        let cell = cell_for(&edited.path);
        store.bump(&cell, 1.0, now);
    }
    for deleted in &rec.output.deleted_files {
        let cell = cell_for(deleted);
        store.bump(&cell, -1.0, now);
    }
    Ok(())
}

pub fn run_dream_pass(
    rec: &RunRecord,
    risk_total: f32,
    asserted_failures: &[String],
) -> DreamReport {
    let silent = if risk_total > 0.6 && asserted_failures.is_empty() {
        vec!["input-output divergence exceeds 0.6 with no asserted failure — possible silent failure".to_string()]
    } else {
        vec![]
    };
    let mut all = rec
        .input
        .rag_files
        .iter()
        .map(|f| format!("rag::{}", f.path))
        .collect::<Vec<_>>();
    all.extend(
        rec.output
            .edited_files
            .iter()
            .map(|f| format!("edit::{}", f.path)),
    );
    DreamReport {
        silent_failures: silent,
        asserted_failures: asserted_failures.to_vec(),
        iterations: all.len().max(1),
    }
}

pub fn discover_proposals(
    rec: &RunRecord,
    store: &SalienceStore,
    now: chrono::DateTime<chrono::Utc>,
) -> Vec<DiscoveryProposal> {
    let mut out = Vec::new();
    for edited in &rec.output.edited_files {
        let cell = cell_for(&edited.path);
        let decayed = store
            .entries
            .get(&cell)
            .map(|e| e.decayed(now))
            .unwrap_or(0.0);
        if decayed.abs() < 0.5 {
            out.push(DiscoveryProposal {
                id: format!("prop-{}", uuid::Uuid::new_v4()),
                proposed_cell: format!("{}::v2", cell),
                evidence_path: edited.path.clone(),
                evidence_excerpt: edited.body.chars().take(80).collect(),
                created_at: now,
            });
        }
    }
    out
}

pub fn data_dir(root: &Path) -> PathBuf {
    root.join("physis-scan")
}

pub fn salience_path(root: &Path) -> PathBuf {
    data_dir(root).join("salience.json")
}

pub fn proposals_path(root: &Path) -> PathBuf {
    data_dir(root).join("proposals.json")
}

pub fn overlay_path(root: &Path) -> PathBuf {
    data_dir(root).join("overlays.json")
}
