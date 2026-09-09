use crate::capture::RunRecord;
use crate::embed::{combine, hash_embed};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sled::Db;
use std::path::Path;

const HISTORY_LIMIT: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub run_id: String,
    pub model: String,
    pub task: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub embedding: Vec<f32>,
    pub prompt_hash: String,
}

pub fn open_db(path: &Path) -> Result<Db> {
    sled::open(path).with_context(|| format!("open sled db at {}", path.display()))
}

pub fn record(db: &Db, rec: &RunRecord) -> Result<()> {
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
    let combined = format!("{}\n{}", in_text, out_text);
    let emb = hash_embed(&combined).to_vec();
    let prompt_hash = hex(Sha256::digest(rec.input.prompt.as_bytes()));
    let entry = HistoryEntry {
        run_id: rec.run_id.unwrap().to_string(),
        model: rec.model.clone(),
        task: rec.task.clone(),
        created_at: rec.created_at.unwrap(),
        embedding: emb,
        prompt_hash,
    };
    let key = format!("{}::{}::{}", entry.task, entry.model, entry.created_at.timestamp_millis());
    let bytes = serde_json::to_vec(&entry)?;
    db.insert(key.as_str(), bytes)?;
    db.flush()?;
    Ok(())
}

pub fn last_n_for(db: &Db, model: &str, task: &str, n: usize) -> Result<Vec<HistoryEntry>> {
    let prefix = format!("{task}::{model}::");
    let mut out: Vec<HistoryEntry> = db
        .scan_prefix(&prefix)
        .filter_map(|r| r.ok())
        .filter_map(|(_, v)| serde_json::from_slice::<HistoryEntry>(&v).ok())
        .collect();
    out.sort_by_key(|e| std::cmp::Reverse(e.created_at));
    out.truncate(n);
    Ok(out)
}

pub fn latest_other_model_for_prompt(
    db: &Db,
    model: &str,
    prompt_hash: &str,
) -> Result<Option<HistoryEntry>> {
    let mut out: Vec<HistoryEntry> = db
        .iter()
        .filter_map(|r| r.ok())
        .filter_map(|(_, v)| serde_json::from_slice::<HistoryEntry>(&v).ok())
        .filter(|e| e.model != model && e.prompt_hash == *prompt_hash)
        .collect();
    out.sort_by_key(|e| std::cmp::Reverse(e.created_at));
    Ok(out.into_iter().next())
}

fn hex(d: impl AsRef<[u8]>) -> String {
    let mut s = String::with_capacity(d.as_ref().len() * 2);
    for b in d.as_ref() {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

pub const DEFAULT_HISTORY_LIMIT: usize = HISTORY_LIMIT;
