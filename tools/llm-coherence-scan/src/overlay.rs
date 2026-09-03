use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct OverlayStore {
    pub overlays: HashMap<String, OverlayEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayEntry {
    pub cell: String,
    pub context: String,
    pub note: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub fn load(root: &Path) -> Result<OverlayStore> {
    let p = crate::refine::overlay_path(root);
    if !p.exists() {
        return Ok(OverlayStore::default());
    }
    let raw = std::fs::read_to_string(&p)?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

pub fn save(root: &Path, store: &OverlayStore) -> Result<()> {
    let p = crate::refine::overlay_path(root);
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(p, serde_json::to_string_pretty(store)?)?;
    Ok(())
}

pub fn check(rec: &crate::capture::RunRecord, store: &OverlayStore) -> Vec<OverlayEntry> {
    let mut hits = Vec::new();
    for edited in &rec.output.edited_files {
        let cell = crate::refine::cell_for(&edited.path);
        for o in store.overlays.values() {
            if o.cell == cell {
                hits.push(o.clone());
            }
        }
    }
    hits
}
