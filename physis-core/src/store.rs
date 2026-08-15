//! Simple JSON-file persistence under `~/.physis-core/`.

use std::path::{Path, PathBuf};

/// Data directory for physis-core state.
pub fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".physis-core")
}

/// Path where the coherence-node graph is persisted.
pub fn nodes_path() -> PathBuf {
    data_dir().join("nodes.json")
}

/// Path where the quality failures are persisted.
pub fn quality_path() -> PathBuf {
    data_dir().join("quality.json")
}

/// Path where studio custom ontology edits are persisted.
pub fn custom_ontology_path() -> PathBuf {
    data_dir().join("custom_ontology.json")
}

/// Ensure the data dir exists.
pub fn ensure_data_dir() -> anyhow::Result<()> {
    std::fs::create_dir_all(data_dir())?;
    Ok(())
}

/// Read a text file that may not exist yet; missing → empty string.
pub fn read_optional(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_live_under_data_dir() {
        assert!(nodes_path().starts_with(data_dir()));
        assert!(quality_path().starts_with(data_dir()));
        assert!(custom_ontology_path().starts_with(data_dir()));
    }
}
