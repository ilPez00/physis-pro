//! Virtual File System (VFS) orchestration — maintains a read-only view
//! of physical files organized by ontology domains using symbolic links.

pub mod orchestrator;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

/// VFS state mapping for reverse lookup (virtual symlink → physical path).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VfsReference {
    pub virtual_path: PathBuf,
    pub physical_path: PathBuf,
    pub last_synced: chrono::DateTime<chrono::Utc>,
}
