//! Disk indexer (PLAN 1.3): a fast-path file catalog with `--watch`.
//!
//! `scan_project` reads and hashes every text file on every run — fine for
//! classification, wasteful for "what changed on disk". The indexer walks
//! metadata only (path, size, mtime), re-hashes just the entries whose
//! `(size, mtime)` moved, and persists the catalog to sled so the next run
//! starts warm. Unchanged files cost one `stat`, zero reads.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// One cataloged file. `hash` may be empty only transiently (first sight).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub path: String,
    pub size: u64,
    pub mtime: f64,
    pub hash: String,
}

impl IndexEntry {
    /// True when neither size nor mtime moved, so the stored hash still holds.
    fn matches_stat(&self, size: u64, mtime: f64) -> bool {
        self.size == size && self.mtime == mtime
    }
}

/// What one indexing pass found, vs the persisted catalog.
#[derive(Debug, Default)]
pub struct IndexReport {
    pub total: usize,
    /// Entries served from the stat fast path (no file read).
    pub fast_reused: usize,
    /// Entries actually read + hashed this pass.
    pub hashed_now: usize,
    pub new: usize,
    pub changed: usize,
    pub deleted: usize,
    pub elapsed_ms: u128,
}

impl std::fmt::Display for IndexReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} files | {} reused (stat fast path), {} hashed | +{}/~{}/-{} | {}ms",
            self.total,
            self.fast_reused,
            self.hashed_now,
            self.new,
            self.changed,
            self.deleted,
            self.elapsed_ms
        )
    }
}

/// A durable catalog for one root directory.
pub struct FileIndex {
    root: PathBuf,
    pub entries: HashMap<String, IndexEntry>,
}

impl FileIndex {
    /// Open (or start fresh) the catalog rooted at `root`.
    pub fn new(root: &Path) -> Self {
        let canonical = root
            .canonicalize()
            .unwrap_or_else(|_| root.to_path_buf());
        Self {
            root: canonical,
            entries: HashMap::new(),
        }
    }

    /// sled key for this catalog: `index:<canonical root>`. One blob per root
    /// keeps load/save O(catalog) and avoids sled-tree ceremony for what is a
    /// plain snapshot.
    fn db_key(&self) -> String {
        format!("index:{}", self.root.display())
    }

    /// Load the previously saved catalog for this root, if any.
    pub fn load(db: &sled::Db, root: &Path) -> anyhow::Result<Self> {
        let mut idx = Self::new(root);
        if let Some(raw) = db.get(idx.db_key())? {
            idx.entries = serde_json::from_slice(&raw)?;
        }
        Ok(idx)
    }

    /// Persist the catalog.
    pub fn save(&self, db: &sled::Db) -> anyhow::Result<()> {
        db.insert(self.db_key(), serde_json::to_vec(&self.entries)?)?;
        Ok(())
    }

    /// Walk the tree collecting metadata only (no file reads). Same exclude
    /// rules as the scanner plus the engine's own state dir — a catalog that
    /// indexes its own sled file churns forever. Unlike the scanner, binary
    /// files are cataloged too: a disk index that skips half the disk is a lie.
    fn walk_metadata(root: &Path) -> Vec<IndexEntry> {
        let canonical = root
            .canonicalize()
            .unwrap_or_else(|_| root.to_path_buf());
        let exclude: std::collections::HashSet<String> = crate::scanner::DEFAULT_EXCLUDE_DIRS
            .iter()
            .map(|d| d.to_string())
            .chain([".physis".to_string()])
            .collect();

        rayon_scope_walk(&canonical, &exclude)
    }

    /// One incremental pass: walk, re-hash only stat-changed entries, diff vs
    /// the catalog, update it. Returns the report; call `save` to persist.
    pub fn index_once(&mut self) -> anyhow::Result<IndexReport> {
        let started = Instant::now();
        let walked = Self::walk_metadata(&self.root);

        let mut report = IndexReport {
            total: walked.len(),
            ..Default::default()
        };

        let mut next: HashMap<String, IndexEntry> = HashMap::with_capacity(walked.len());

        // Re-hash in parallel: only entries whose stat signature moved.
        let stale: Vec<(String, u64, f64)> = walked
            .iter()
            .filter(|e| {
                match self.entries.get(&e.path) {
                    Some(prev) => !prev.matches_stat(e.size, e.mtime),
                    None => true,
                }
            })
            .map(|e| (e.path.clone(), e.size, e.mtime))
            .collect();

        use rayon::prelude::*;
        let hashes: Vec<Option<String>> = stale
            .par_iter()
            .map(|(rel, _, _)| {
                compute_file_hash(&self.root.join(rel))
            })
            .collect();

        let fresh_hashes: HashMap<String, String> = stale
            .iter()
            .zip(hashes)
            .filter_map(|((rel, _, _), h)| h.map(|h| (rel.clone(), h)))
            .collect();
        report.hashed_now = fresh_hashes.len();
        report.fast_reused = report.total - stale.len();

        let mut old_paths: std::collections::HashSet<&String> =
            self.entries.keys().collect();

        for entry in walked {
            let hash = match fresh_hashes.get(&entry.path) {
                Some(h) => Some(h.clone()),
                None => self.entries.get(&entry.path).map(|p| p.hash.clone()),
            };
            let Some(hash) = hash else {
                continue; // unreadable this pass; drop rather than cache a guess
            };

            match self.entries.get(&entry.path) {
                None => report.new += 1,
                Some(prev) => {
                    if prev.hash != hash {
                        report.changed += 1;
                    }
                }
            }
            old_paths.remove(&entry.path);

            next.insert(
                entry.path.clone(),
                IndexEntry {
                    path: entry.path,
                    size: entry.size,
                    mtime: entry.mtime,
                    hash,
                },
            );
        }
        report.deleted = old_paths.len();

        self.entries = next;
        report.elapsed_ms = started.elapsed().as_millis();
        Ok(report)
    }
}

/// Parallel metadata walk. Split out so `index_once` stays readable.
fn rayon_scope_walk(root: &Path, exclude: &std::collections::HashSet<String>) -> Vec<IndexEntry> {
    use rayon::prelude::*;
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                !exclude.contains(name.as_ref())
            } else {
                true
            }
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .par_bridge()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let rel = e.path().strip_prefix(root).ok()?;
            Some(IndexEntry {
                path: rel.to_string_lossy().to_string(),
                size: meta.len(),
                mtime: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0),
                hash: String::new(),
            })
        })
        .collect()
}

/// SHA-256 of a file, streaming. `None` when unreadable.
pub fn compute_file_hash(path: &Path) -> Option<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(root: &Path, rel: &str, content: &[u8]) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    #[test]
    fn first_pass_indexes_everything_second_is_all_fast_path() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), "a.txt", b"hello");
        seed(dir.path(), "sub/b.txt", b"world");

        let mut idx = FileIndex::new(dir.path());
        let r1 = idx.index_once().unwrap();
        assert_eq!(r1.total, 2);
        assert_eq!(r1.new, 2);
        assert_eq!(r1.hashed_now, 2);

        // Unchanged tree → every entry rides the stat fast path, nothing new.
        let r2 = idx.index_once().unwrap();
        assert_eq!(r2.total, 2);
        assert_eq!(r2.new, 0);
        assert_eq!(r2.changed, 0);
        assert_eq!(r2.deleted, 0);
        assert_eq!(r2.fast_reused, 2, "second pass must not re-read files");
        assert_eq!(r2.hashed_now, 0);
    }

    /// Content change must surface as `changed`, and exclusion rules apply
    /// (node_modules is cataloged nowhere).
    #[test]
    fn edits_and_excludes() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), "keep.txt", b"v1");
        seed(dir.path(), "node_modules/junk.js", b"noise");
        std::fs::create_dir_all(dir.path().join("target")).unwrap();

        let mut idx = FileIndex::new(dir.path());
        let r = idx.index_once().unwrap();
        assert_eq!(r.total, 1, "only keep.txt; excludes respected");
        assert_eq!(r.new, 1);

        // Rewrite with a different size → stat moves → re-hash → changed.
        std::thread::sleep(std::time::Duration::from_millis(20));
        seed(dir.path(), "keep.txt", b"v2-longer");
        let r = idx.index_once().unwrap();
        assert_eq!(r.changed, 1);
        assert_eq!(r.hashed_now, 1);

        // Deletion surfaces as deleted.
        std::fs::remove_file(dir.path().join("keep.txt")).unwrap();
        let r = idx.index_once().unwrap();
        assert_eq!(r.deleted, 1);
        assert_eq!(r.total, 0);
    }

    /// The catalog survives a restart: load into a fresh instance, then an
    /// unchanged pass reports zero deltas and full fast-path reuse.
    #[test]
    fn sled_roundtrip_keeps_fast_path_warm() {
        let dir = tempfile::tempdir().unwrap();
        let db = sled::Config::new()
            .path(dir.path().join("state.sled"))
            .open()
            .unwrap();
        let work = tempfile::tempdir().unwrap();
        seed(work.path(), "doc.md", b"# title");

        let mut idx = FileIndex::new(work.path());
        idx.index_once().unwrap();
        idx.save(&db).unwrap();

        drop(idx); // "restart"
        let mut restored = FileIndex::load(&db, work.path()).unwrap();
        assert_eq!(restored.entries.len(), 1);

        let r = restored.index_once().unwrap();
        assert_eq!((r.new, r.changed, r.deleted), (0, 0, 0));
        assert_eq!(r.fast_reused, 1, "restored catalog stays warm");
    }
}
