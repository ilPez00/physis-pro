//! Filesystem watching and network scanning — hash-cache-based change detection.
use std::path::PathBuf;

use crate::scanner::{compute_diff, load_hash_cache, save_hash_cache, scan_project, ScanDiff};
use crate::trie::DynamicVectorTrie;

/// Events emitted by the network scanner: file changes, agent discovery/loss, or errors.
pub enum NetworkEvent {
    FilesChanged(ScanDiff),
    AgentDiscovered(String),
    AgentLost(String),
    Error(String),
}

/// Scans watched directories for file changes using a hash cache.
#[derive(Debug, Clone)]
pub struct NetworkScanner {
    pub watch_dirs: Vec<PathBuf>,
    pub cache_path: PathBuf,
    #[allow(dead_code)]
    scan_interval_secs: u64,
}

impl NetworkScanner {
    /// Creates a scanner with directories to watch, a cache file path, and scan interval.
    pub fn new(watch_dirs: Vec<PathBuf>, cache_path: PathBuf, scan_interval_secs: u64) -> Self {
        Self {
            watch_dirs,
            cache_path,
            scan_interval_secs,
        }
    }

    /// Scans all watched directories and returns diffs against the cached state.
    pub fn scan_all(&self) -> Vec<ScanDiff> {
        let mut results = Vec::new();
        let cache = load_hash_cache(&self.cache_path);

        for dir in &self.watch_dirs {
            if !dir.exists() {
                continue;
            }
            let files = scan_project(dir, None);
            let diff = compute_diff(&files, &cache);
            results.push(diff);
        }

        results
    }

    /// Applies scan diffs to the hash cache and persists to disk.
    pub fn update_cache(&self, diffs: &[ScanDiff]) {
        let mut cache = load_hash_cache(&self.cache_path);
        for diff in diffs {
            for f in &diff.new {
                cache.insert(f.path.clone(), f.hash.clone());
            }
            for f in &diff.changed {
                cache.insert(f.path.clone(), f.hash.clone());
            }
            for p in &diff.deleted {
                cache.remove(p);
            }
        }
        let _ = save_hash_cache(&self.cache_path, &cache);
    }

    /// Inserts new/changed paths into the trie and removes deleted ones.
    pub fn apply_to_trie(&self, trie: &mut DynamicVectorTrie, diffs: &[ScanDiff]) {
        for diff in diffs {
            for f in &diff.new {
                let tids = trie.tokenize_path_mut(&f.path);
                trie.insert(&tids);
                for line in &f.structural_lines {
                    let line_tids = trie.tokenize_mut(line);
                    trie.insert(&line_tids);
                }
            }
            for f in &diff.changed {
                let _ = trie.delete(&trie.tokenize_path(&f.path));
                let tids = trie.tokenize_path_mut(&f.path);
                trie.insert(&tids);
            }
            for p in &diff.deleted {
                let _ = trie.delete(&trie.tokenize_path(p));
            }
        }
    }

    /// Perform a VFS health check by verifying all symlinks in the orchestrator's virtual structure.
    pub fn vfs_health_check(
        &self,
        orchestrator: &mut crate::vfs::orchestrator::VfsOrchestrator,
    ) -> anyhow::Result<()> {
        let mut to_remove = Vec::new();
        for reference in &orchestrator.reference_map {
            if !reference.physical_path.exists() {
                // Physical file gone, remove symlink and entry
                if reference.virtual_path.exists() {
                    let _ = std::fs::remove_file(&reference.virtual_path);
                }
                to_remove.push(reference.virtual_path.clone());
            } else if !reference.virtual_path.exists() {
                // Symlink gone, recreate it
                #[cfg(unix)]
                std::os::unix::fs::symlink(&reference.physical_path, &reference.virtual_path)?;
            }
        }
        orchestrator
            .reference_map
            .retain(|r| !to_remove.contains(&r.virtual_path));
        Ok(())
    }
}

#[cfg(feature = "network")]
pub mod watcher {
    use super::NetworkEvent;
    use notify::{Config, EventKind, RecursiveMode, Watcher};
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::time::Duration;

    /// Starts a filesystem watcher that polls directories for create/modify/remove events.
    pub fn start_watcher(
        watch_dirs: Vec<PathBuf>,
        _tx: mpsc::Sender<NetworkEvent>,
    ) -> anyhow::Result<notify::RecommendedWatcher> {
        let (event_tx, _event_rx) = mpsc::channel();
        let mut watcher =
            notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    match event.kind {
                        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                            let _ = event_tx.send(());
                        }
                        _ => {}
                    }
                }
            })?;

        watcher.configure(Config::default().with_poll_interval(Duration::from_secs(2)))?;
        for dir in watch_dirs {
            if dir.exists() {
                watcher.watch(&dir, RecursiveMode::Recursive)?;
            }
        }

        Ok(watcher)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{FileInfo, ScanDiffSummary};
    use crate::trie::DynamicVectorTrie;

    #[test]
    fn test_scan_all_no_dirs() {
        let scanner = NetworkScanner::new(vec![], PathBuf::from("/tmp/physis_test_cache.json"), 60);
        let diffs = scanner.scan_all();
        assert!(diffs.is_empty());
    }

    #[test]
    fn test_apply_to_trie() {
        let mut trie = DynamicVectorTrie::new();
        let scanner = NetworkScanner::new(vec![], PathBuf::from("/tmp/physis_test_cache.json"), 60);

        let file = FileInfo {
            path: "test/file.rs".to_string(),
            abs_path: "/tmp/test/file.rs".to_string(),
            ext: ".rs".to_string(),
            size: 10,
            mtime: 0.0,
            hash: "abc".to_string(),
            structural_lines: vec!["fn main".to_string()],
        };

        let diff = ScanDiff {
            new: vec![file],
            changed: vec![],
            deleted: vec![],
            unchanged: vec![],
            summary: ScanDiffSummary {
                new: 1,
                changed: 0,
                deleted: 0,
                unchanged: 0,
                total: 1,
            },
        };

        scanner.apply_to_trie(&mut trie, &[diff]);

        let tids = trie.tokenize_path("test/file.rs");
        assert!(trie.search(&tids), "new file should be in trie");

        let main_tids = trie.tokenize("fn main");
        assert!(!main_tids.is_empty(), "structural line tokens should exist");
    }

    #[test]
    fn test_apply_deleted_from_trie() {
        let mut trie = DynamicVectorTrie::new();
        trie.insert_path("old/file.rs");

        let scanner = NetworkScanner::new(vec![], PathBuf::from("/tmp/physis_test_cache.json"), 60);

        let diff = ScanDiff {
            new: vec![],
            changed: vec![],
            deleted: vec!["old/file.rs".to_string()],
            unchanged: vec![],
            summary: ScanDiffSummary {
                new: 0,
                changed: 0,
                deleted: 1,
                unchanged: 0,
                total: 0,
            },
        };

        scanner.apply_to_trie(&mut trie, &[diff]);

        let tids = trie.tokenize_path("old/file.rs");
        if !tids.is_empty() {
            assert!(!trie.search(&tids), "deleted file should not be in trie");
        }
    }
}
