use crate::baseline::{self, HistoryEntry};
use crate::capture::RunRecord;
use crate::embed::{combine, cosine, hash_embed};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sled::Db;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceCorpus {
    pub name: String,
    pub entries: Vec<ReferenceEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceEntry {
    pub label: String,
    pub embedding: Vec<f32>,
}

impl ReferenceCorpus {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }
    pub fn mean_embedding(&self) -> Option<Vec<f32>> {
        if self.entries.is_empty() {
            return None;
        }
        let dim = self.entries[0].embedding.len();
        let mut acc = vec![0.0f32; dim];
        for e in &self.entries {
            for (i, v) in e.embedding.iter().enumerate() {
                acc[i] += v;
            }
        }
        let n = self.entries.len() as f32;
        for v in acc.iter_mut() {
            *v /= n;
        }
        let norm = acc.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in acc.iter_mut() {
                *v /= norm;
            }
        }
        Some(acc)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DriftComponents {
    pub local: Option<f32>,
    pub reference: Option<f32>,
    pub cross_model: Option<f32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DriftScore {
    pub total: Option<f32>,
    pub components: DriftComponents,
}

pub fn model_drift(
    rec: &RunRecord,
    db: &Db,
    ref_corpus: Option<&ReferenceCorpus>,
) -> Result<DriftScore> {
    let in_text = combine(
        &rec.input.prompt,
        &rec
            .input
            .rag_files
            .iter()
            .map(|f| (f.path.clone(), f.body.clone()))
            .collect::<Vec<_>>(),
    );
    let out_text = combine(
        &rec.output.text,
        &rec
            .output
            .edited_files
            .iter()
            .map(|f| (f.path.clone(), f.body.clone()))
            .collect::<Vec<_>>(),
    );
    let cur = hash_embed(&format!("{}\n{}", in_text, out_text));
    let prompt_hash = hex(Sha256::digest(rec.input.prompt.as_bytes()));

    let local = local_drift(&cur, db, &rec.model, &rec.task)?;
    let reference = ref_corpus.and_then(|c| ref_drift(&cur, c));
    let cross_model = cross_model_drift(&cur, db, &rec.model, &prompt_hash)?;

    let mut components = vec![local, reference, cross_model];
    let present: Vec<f32> = components.iter().filter_map(|x| *x).collect();
    let total = if present.is_empty() {
        None
    } else {
        Some(present.iter().sum::<f32>() / present.len() as f32)
    };
    components.resize(3, None);

    Ok(DriftScore {
        total,
        components: DriftComponents {
            local: components[0],
            reference: components[1],
            cross_model: components[2],
        },
    })
}

fn local_drift(cur: &[f32], db: &Db, model: &str, task: &str) -> Result<Option<f32>> {
    let history = baseline::last_n_for(db, model, task, baseline::DEFAULT_HISTORY_LIMIT)?;
    if history.is_empty() {
        return Ok(None);
    }
    let mean = mean_embedding(&history);
    let d = (1.0 - cosine(cur, &mean)).clamp(0.0, 1.0);
    Ok(Some(d))
}

fn ref_drift(cur: &[f32], corpus: &ReferenceCorpus) -> Option<f32> {
    let mean = corpus.mean_embedding()?;
    let d = (1.0 - cosine(cur, &mean)).clamp(0.0, 1.0);
    Some(d)
}

fn cross_model_drift(
    cur: &[f32],
    db: &Db,
    model: &str,
    prompt_hash: &str,
) -> Result<Option<f32>> {
    let other = baseline::latest_other_model_for_prompt(db, model, prompt_hash)?;
    let Some(other) = other else { return Ok(None) };
    let d = (1.0 - cosine(cur, &other.embedding)).clamp(0.0, 1.0);
    Ok(Some(d))
}

fn mean_embedding(history: &[HistoryEntry]) -> Vec<f32> {
    if history.is_empty() {
        return vec![];
    }
    let dim = history[0].embedding.len();
    let mut acc = vec![0.0f32; dim];
    for e in history {
        for (i, v) in e.embedding.iter().enumerate() {
            acc[i] += v;
        }
    }
    let n = history.len() as f32;
    for v in acc.iter_mut() {
        *v /= n;
    }
    let norm = acc.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in acc.iter_mut() {
            *v /= norm;
        }
    }
    acc
}

fn hex(d: impl AsRef<[u8]>) -> String {
    let mut s = String::with_capacity(d.as_ref().len() * 2);
    for b in d.as_ref() {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

pub fn dump_history_summary(db: &Db) -> Result<BTreeMap<String, usize>> {
    let mut by_task: BTreeMap<String, usize> = BTreeMap::new();
    for item in db.iter().filter_map(|r| r.ok()) {
        if let Ok(e) = serde_json::from_slice::<HistoryEntry>(&item.1) {
            *by_task.entry(format!("{}::{}", e.task, e.model)).or_insert(0) += 1;
        }
    }
    Ok(by_task)
}
