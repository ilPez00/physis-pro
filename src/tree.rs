//! Directory-hierarchy + document-metadata tree (scan v2).
//!
//! The scan keeps a persistent tree of every scanned directory: its layout and
//! each document's metadata (mtime, size, content hash, structural labels).
//! A **Merkle-style subtree fingerprint** (over children names, kinds, and file
//! mtime/size) lets the next scan skip a whole unchanged subtree without even
//! re-reading its files — a document is hashed and embedded at most once, and
//! only files whose fingerprint actually changed are ever re-processed.
//!
//! This is the "map the hierarchy, skip what's already processed, diff
//! previous vs current" half of the scan. The embedding/registration half lives
//! in the scan handler, which consumes [`TreeDiff`] and produces **one node per
//! document** (see `src/web/ingest.rs`).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// Persistent metadata for one scanned file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileMeta {
    pub mtime_secs: f64,
    pub size: u64,
    /// Whether the file passes the include rules (text, not excluded). Files
    /// that don't are tracked in the tree (so their appearance/disappearance
    /// is detected) but never hashed or embedded.
    pub included: bool,
    /// Sha256 of file content. `None` until the file has actually been hashed —
    /// files in unchanged subtrees are never hashed, which is the point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// Path-independent structural lines captured at scan time — the document's
    /// embedding text and content-recall handle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
}

/// One node of the hierarchy: a directory (with children) or a file (with
/// metadata).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DirNode {
    pub name: String,
    #[serde(default)]
    pub children: BTreeMap<String, DirNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<FileMeta>,
    /// Merkle subtree fingerprint. Recomputed on demand; never persisted.
    #[serde(skip)]
    pub fingerprint: String,
    /// Included files under this node (dirs) or 0/1 (files). Computed during
    /// fingerprinting so a fully-skipped subtree still reports its size.
    #[serde(skip)]
    pub file_count: usize,
}

impl DirNode {
    pub fn is_file(&self) -> bool {
        self.file.is_some()
    }
}

/// The full hierarchy + metadata of one scanned root.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DirTree {
    pub root: DirNode,
}

impl DirTree {
    /// Stat-only walk (no content reads). Skips the excluded dir names (the
    /// standard ones plus the persistent store dir, so a scan root that happens
    /// to contain the data dir never ingests its own store).
    pub fn meta_walk(root: &Path) -> Self {
        let exclude: HashSet<String> = crate::scanner::DEFAULT_EXCLUDE_DIRS
            .iter()
            .copied()
            .chain(["state.sled"])
            .map(|s| s.to_string())
            .collect();
        let mut files: Vec<(PathBuf, FileMeta)> = Vec::new();
        walk(root, root, &exclude, &mut files);
        let mut tree = DirTree { root: DirNode { name: "<root>".into(), ..Default::default() } };
        for (rel, meta) in files {
            insert(&mut tree.root, &rel, meta);
        }
        tree
    }

    /// Recompute every subtree fingerprint (cheap — metadata only). Must be
    /// called on both trees before diffing.
    pub fn fingerprint(&mut self) -> String {
        fp(&mut self.root)
    }

    /// Diff `self` (the previous scan) against `current`. Equal-fingerprint
    /// subtrees are skipped whole; only genuinely different files surface.
    pub fn diff(&self, current: &DirTree) -> TreeDiff {
        let mut out = TreeDiff::default();
        diff_dir(&self.root, &current.root, Path::new(""), &mut out);
        out
    }

    /// A doc's stored content hash, if any (for the changed-but-same-content
    /// downgrade).
    pub fn stored_hash(&self, rel: &Path) -> Option<&str> {
        node_at(&self.root, rel).and_then(|n| n.file.as_ref()).and_then(|f| f.content_hash.as_deref())
    }

    /// Mutable node at a relative path (to stamp content hash + labels).
    pub fn node_at_mut(&mut self, rel: &Path) -> Option<&mut DirNode> {
        let mut cur = &mut self.root;
        for comp in rel.components() {
            let name = comp.as_os_str().to_string_lossy().to_string();
            cur = cur.children.get_mut(&name)?;
        }
        Some(cur)
    }

    /// Rebuild `FileInfo`s for every included file that has been processed
    /// (has a content hash) — feeds the mapper (goals) without re-reading
    /// unchanged files. `structural_lines` may be empty for files with no
    /// extractable structure; the mapper still creates a path goal for them.
    pub fn file_infos(&self, root: &Path) -> Vec<crate::scanner::FileInfo> {
        let mut out = Vec::new();
        collect_file_infos(&self.root, root, "", &mut out);
        out
    }

    pub fn save(&self, data_dir: &Path, root: &Path) -> anyhow::Result<()> {
        let path = tree_path(data_dir, root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    pub fn load(data_dir: &Path, root: &Path) -> Option<Self> {
        let path = tree_path(data_dir, root);
        std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
    }
}

/// The result of diffing a previous tree against the current walk.
#[derive(Debug, Default)]
pub struct TreeDiff {
    /// Files that appeared (included, needs embed + node).
    pub new_files: Vec<PathBuf>,
    /// Files whose mtime/size changed (candidates — content hash confirms).
    pub changed_files: Vec<PathBuf>,
    /// Removed paths (relative).
    pub deleted: Vec<PathBuf>,
    /// Whole unchanged subtrees skipped without re-reading.
    pub skipped_subtrees: usize,
    /// Files confirmed unchanged after hashing (included, unchanged content).
    pub unchanged_files: usize,
}

fn walk(dir: &Path, root: &Path, exclude: &HashSet<String>, out: &mut Vec<(PathBuf, FileMeta)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let p = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if exclude.contains(&name) {
                continue;
            }
            walk(&p, root, exclude, out);
        } else if ft.is_file() {
            let Ok(rel) = p.strip_prefix(root) else { continue };
            let Ok(meta) = std::fs::metadata(&p) else { continue };
            let included = crate::scanner::should_include_file(&p, exclude, root);
            out.push((
                rel.to_path_buf(),
                FileMeta {
                    mtime_secs: meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs_f64())
                        .unwrap_or(0.0),
                    size: meta.len(),
                    included,
                    content_hash: None,
                    labels: Vec::new(),
                },
            ));
        }
    }
}

fn insert(node: &mut DirNode, rel: &Path, meta: FileMeta) {
    let Some(first) = rel.components().next() else { return };
    let name = first.as_os_str().to_string_lossy().to_string();
    if rel.components().count() == 1 {
        let name2 = name.clone();
        node.children.insert(name, DirNode { name: name2, file: Some(meta), ..Default::default() });
    } else {
        let child = node
            .children
            .entry(name.clone())
            .or_insert_with(|| DirNode { name, ..Default::default() });
        let rest: PathBuf = rel.components().skip(1).collect();
        insert(child, &rest, meta);
    }
}

fn node_at<'a>(node: &'a DirNode, rel: &Path) -> Option<&'a DirNode> {
    let mut cur = node;
    for comp in rel.components() {
        let name = comp.as_os_str().to_string_lossy().to_string();
        cur = cur.children.get(&name)?;
    }
    Some(cur)
}

fn collect_file_infos(node: &DirNode, root: &Path, prefix: &str, out: &mut Vec<crate::scanner::FileInfo>) {
    for (name, child) in &node.children {
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        match &child.file {
            Some(f) if f.included && f.content_hash.is_some() => {
                let abs = root.join(&rel);
                let ext = child
                    .name
                    .rsplit('.')
                    .next()
                    .map(|e| format!(".{}", e.to_lowercase()))
                    .unwrap_or_default();
                out.push(crate::scanner::FileInfo {
                    path: rel.clone(),
                    abs_path: abs.to_string_lossy().to_string(),
                    ext,
                    size: f.size,
                    mtime: f.mtime_secs,
                    hash: f.content_hash.clone().unwrap_or_default(),
                    structural_lines: f.labels.clone(),
                });
            }
            Some(_) => {}
            None => collect_file_infos(child, root, &rel, out),
        }
    }
}

/// Merkle subtree fingerprint: sha256 over sorted children of
/// (name, kind, file→mtime/size/included, dir→child fingerprint). Also counts
/// included files per subtree (for skipped-subtree unchanged reporting).
fn fp(node: &mut DirNode) -> String {
    let mut hasher = Sha256::new();
    let mut count = 0usize;
    for (name, child) in node.children.iter_mut() {
        hasher.update(name.as_bytes());
        hasher.update([0u8]);
        match child.file.as_ref() {
            Some(f) => {
                hasher.update([1u8]);
                hasher.update(f.mtime_secs.to_bits().to_le_bytes());
                hasher.update(f.size.to_le_bytes());
                hasher.update([if f.included { 1 } else { 0 }]);
                child.file_count = if f.included { 1 } else { 0 };
                count += child.file_count;
            }
            None => {
                hasher.update([2u8]);
                hasher.update(fp(child).as_bytes());
                count += child.file_count;
            }
        }
    }
    node.file_count = count;
    let f = format!("{:x}", hasher.finalize());
    node.fingerprint = f.clone();
    f
}

fn diff_dir(prev: &DirNode, curr: &DirNode, path: &Path, out: &mut TreeDiff) {
    if prev.fingerprint == curr.fingerprint {
        out.skipped_subtrees += 1;
        // The whole subtree is unchanged — report its files as such so an
        // idempotent re-scan reads "everything unchanged", not "nothing".
        out.unchanged_files += curr.file_count;
        return;
    }
    for (name, child) in &curr.children {
        let child_path = path.join(name);
        let pchild = prev.children.get(name);
        if child.is_file() {
            let cur = child.file.as_ref().unwrap();
            match pchild.and_then(|p| p.file.as_ref()) {
                None => {
                    if cur.included {
                        out.new_files.push(child_path);
                    }
                }
                Some(pf) => {
                    if pf.mtime_secs == cur.mtime_secs
                        && pf.size == cur.size
                        && pf.included == cur.included
                    {
                        // Fingerprint differed only because a sibling changed.
                        out.unchanged_files += 1;
                    } else if cur.included {
                        out.changed_files.push(child_path);
                    }
                }
            }
        } else {
            diff_dir(
                pchild.unwrap_or(&EMPTY_DIR),
                child,
                &child_path,
                out,
            );
        }
    }
    for (name, _) in &prev.children {
        if !curr.children.contains_key(name) {
            let gone = path.join(name);
            collect_deleted(prev.children.get(name).unwrap(), &gone, out);
        }
    }
}

static EMPTY_DIR: DirNode = DirNode {
    name: String::new(),
    children: BTreeMap::new(),
    file: None,
    fingerprint: String::new(),
    file_count: 0,
};

fn collect_deleted(node: &DirNode, path: &Path, out: &mut TreeDiff) {
    if node.is_file() {
        out.deleted.push(path.to_path_buf());
    } else {
        for (name, child) in &node.children {
            collect_deleted(child, &path.join(name), out);
        }
    }
}

fn tree_path(data_dir: &Path, root: &Path) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(root.to_string_lossy().as_bytes());
    let fp = format!("{:x}", hasher.finalize());
    data_dir.join(format!("tree_{fp}.json"))
}

/// One document resolved by a scan run: confirmed new/changed, ready to embed
/// and register as a single node.
#[derive(Debug, Clone)]
pub struct ScannedDoc {
    pub rel: PathBuf,
    pub abs: PathBuf,
    pub content_hash: String,
    /// Path-independent structural text (the embedding source). Empty for docs
    /// with no extractable structure — callers fall back to the rel path.
    pub lines: Vec<String>,
    /// True when the file appeared (mechanism Generated); false when it was a
    /// content change (mechanism Corrected).
    pub was_new: bool,
}

/// Everything a scan run needs to know about one directory: the previous tree
/// loaded + current stat-walk diffed, plus the docs that actually need
/// embedding. Shared by `/scan` and watch-mode, so both inherit the same
/// subtree-skipping + one-node-per-doc semantics.
#[derive(Debug)]
pub struct ScanRun {
    pub current: DirTree,
    pub diff: TreeDiff,
    pub docs: Vec<ScannedDoc>,
}

impl DirTree {
    /// Load the previous tree, stat-walk the root, diff (skipping unchanged
    /// subtrees), and hash + extract only the confirmed new/changed docs.
    pub fn scan_run(data_dir: &Path, root: &Path) -> ScanRun {
        let prev = DirTree::load(data_dir, root);
        let mut prev_fp = prev.clone().unwrap_or_default();
        prev_fp.fingerprint();
        let mut current = DirTree::meta_walk(root);
        current.fingerprint();
        let mut diff = prev_fp.diff(&current);

        let mut docs = Vec::new();
        for rel in diff.new_files.iter().chain(diff.changed_files.iter()) {
            let was_new = diff.new_files.contains(rel);
            let abs = root.join(rel);
            let Some(fi) = crate::scanner::process_file(&abs, root) else {
                continue;
            };
            if let Some(node) = current.node_at_mut(rel) {
                if let Some(f) = node.file.as_mut() {
                    f.content_hash = Some(fi.hash.clone());
                    f.labels = fi.structural_lines.clone();
                }
            }
            // mtime/size bumped but content identical → no re-embed.
            if let Some(prev_hash) = prev.as_ref().and_then(|t| t.stored_hash(rel)) {
                if prev_hash == fi.hash {
                    diff.unchanged_files += 1;
                    continue;
                }
            }
            docs.push(ScannedDoc {
                rel: rel.clone(),
                abs,
                content_hash: fi.hash,
                lines: fi.structural_lines,
                was_new,
            });
        }
        ScanRun { current, diff, docs }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tdir(tag: &str) -> (tempfile::TempDir, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(tag);
        (d, p)
    }

    fn touch(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, body).unwrap();
    }

    #[test]
    fn meta_walk_builds_hierarchy_and_skips_excluded_dirs() {
        let (_d, root) = tdir("walk");
        touch(&root, "src/main.rs", "fn main() {}");
        touch(&root, "src/lib.rs", "pub fn f() {}");
        touch(&root, ".git/config", "x");
        touch(&root, "notes.md", "# Hi");

        let mut tree = DirTree::meta_walk(&root);
        tree.fingerprint();
        assert!(tree.root.children.contains_key("src"));
        assert!(tree.root.children.contains_key("notes.md"));
        // .git is excluded — not in the tree.
        assert!(!tree.root.children.contains_key(".git"));
        assert_eq!(tree.root.children["notes.md"].file.as_ref().unwrap().included, true);
        // A binary (non-included) file still appears but flagged.
        std::fs::write(root.join("img.png"), [0u8]).unwrap();
        let t2 = DirTree::meta_walk(&root);
        assert!(t2.root.children.contains_key("img.png"));
        assert_eq!(t2.root.children["img.png"].file.as_ref().unwrap().included, false);
    }

    #[test]
    fn fingerprint_stability_and_detection() {
        let (_d, root) = tdir("fp");
        touch(&root, "a.txt", "one");
        touch(&root, "sub/b.txt", "two");

        let mut t1 = DirTree::meta_walk(&root);
        let fp1 = t1.fingerprint();
        let mut t2 = DirTree::meta_walk(&root);
        assert_eq!(t2.fingerprint(), fp1, "identical tree, identical fingerprint");

        // Touching a file changes its subtree fingerprint.
        std::thread::sleep(std::time::Duration::from_millis(20));
        touch(&root, "a.txt", "one-two");
        let mut t3 = DirTree::meta_walk(&root);
        assert_ne!(t3.fingerprint(), fp1, "content change surfaces in fingerprint");

        // A subtree untouched keeps its fingerprint.
        let mut t4 = DirTree::meta_walk(&root);
        t4.fingerprint();
        assert_eq!(
            t4.root.children["sub"].fingerprint,
            t1.root.children["sub"].fingerprint,
            "unchanged subtree fingerprint stable"
        );
    }

    #[test]
    fn diff_skips_unchanged_subtrees_and_lists_changes() {
        let (_d, root) = tdir("diff");
        touch(&root, "stable/a.txt", "same");
        touch(&root, "changing/b.txt", "v1");

        let mut prev = DirTree::meta_walk(&root);
        prev.fingerprint();
        let stable_fp_prev = prev.root.children["stable"].fingerprint.clone();

        // Change one file under `changing`, add another.
        std::thread::sleep(std::time::Duration::from_millis(20));
        touch(&root, "changing/b.txt", "v2");
        touch(&root, "changing/new.txt", "brand new");

        let mut curr = DirTree::meta_walk(&root);
        curr.fingerprint();

        // The untouched `stable` subtree kept its fingerprint → diff skips it.
        assert_eq!(curr.root.children["stable"].fingerprint, stable_fp_prev);
        let diff = prev.diff(&curr);
        assert!(diff.changed_files.contains(&PathBuf::from("changing/b.txt")));
        assert!(diff.new_files.contains(&PathBuf::from("changing/new.txt")));
        assert!(diff.skipped_subtrees >= 1, "stable subtree skipped");
    }

    #[test]
    fn diff_detects_deletions() {
        let (_d, root) = tdir("del");
        touch(&root, "a.txt", "1");
        touch(&root, "gone/b.txt", "2");
        let mut prev = DirTree::meta_walk(&root);
        prev.fingerprint();

        std::fs::remove_file(root.join("gone/b.txt")).unwrap();
        std::fs::remove_dir(root.join("gone")).unwrap();

        let mut curr = DirTree::meta_walk(&root);
        curr.fingerprint();
        let diff = prev.diff(&curr);
        assert_eq!(diff.deleted, vec![PathBuf::from("gone/b.txt")]);
    }

    #[test]
    fn save_then_load_round_trips_and_stored_hash() {
        let (_d, root) = tdir("save");
        touch(&root, "doc.txt", "hello");
        let data = root.join(".physis");
        let mut tree = DirTree::meta_walk(&root);
        // Simulate a hashed file.
        if let Some(f) = tree.root.children.get_mut("doc.txt").and_then(|n| n.file.as_mut()) {
            f.content_hash = Some("abc".into());
            f.labels = vec!["hello".into()];
        }
        tree.save(&data, &root).unwrap();
        let loaded = DirTree::load(&data, &root).expect("loaded");
        assert_eq!(loaded.stored_hash(Path::new("doc.txt")), Some("abc"));
    }

    #[test]
    fn scan_run_is_idempotent_and_lists_docs() {
        let (_d, root) = tdir("run");
        touch(&root, "a.txt", "first content");
        touch(&root, "sub/b.txt", "second content");

        // Store lives OUTSIDE the scan root (the production shape) — otherwise
        // the walk ingests its own store.
        let data = root.parent().unwrap().join(".store");
        std::fs::create_dir_all(&data).unwrap();

        // First run: both docs are new. Callers persist run.current before the
        // next run (as /scan and watch-mode do).
        let run1 = DirTree::scan_run(&data, &root);
        assert_eq!(run1.docs.len(), 2, "both new on first run");
        assert!(run1.docs.iter().all(|d| d.was_new));
        run1.current.save(&data, &root).unwrap();

        // Second run, nothing changed: subtree skip, no docs, everything
        // reported unchanged.
        let run2 = DirTree::scan_run(&data, &root);
        assert!(run2.docs.is_empty(), "no re-embed on unchanged tree");
        assert_eq!(run2.diff.unchanged_files, 2);
        run2.current.save(&data, &root).unwrap();

        // Touch one file's content: exactly that one is a changed doc.
        std::thread::sleep(std::time::Duration::from_millis(20));
        touch(&root, "a.txt", "first content v2");
        let run3 = DirTree::scan_run(&data, &root);
        assert_eq!(run3.docs.len(), 1);
        assert!(!run3.docs[0].was_new, "edit is Corrected, not Generated");
        assert_eq!(run3.docs[0].rel, PathBuf::from("a.txt"));
    }
}