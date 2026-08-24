//! Orchestrates the creation and maintenance of symbolic links for the VFS.

use crate::graph::FileThemeGraph;
use crate::vfs::VfsReference;
use std::fs;
use std::path::{Path, PathBuf};

pub struct VfsOrchestrator {
    pub base_dir: PathBuf,
    pub reference_map: Vec<VfsReference>,
}

impl VfsOrchestrator {
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            reference_map: Vec::new(),
        }
    }

    /// Regenerates the virtual directory structure based on the current FileThemeGraph.
    pub fn sync_graph(&mut self, graph: &FileThemeGraph) -> anyhow::Result<()> {
        let vfs_root = self.base_dir.join("virtual");
        if !vfs_root.exists() {
            fs::create_dir_all(&vfs_root)?;
        }

        // Collect every virtual path the graph wants to exist, so we can prune
        // links whose source file was renamed/deleted since the last sync.
        let mut wanted: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

        for file in &graph.files {
            let file_themes: Vec<&crate::graph::ThemeNode> = graph
                .edges
                .iter()
                .filter(|e| e.file_id == file.id)
                .filter_map(|e| graph.get_theme(e.theme_id))
                .collect();

            for theme in file_themes {
                let virtual_path = vfs_root
                    .join(theme.category.as_deref().unwrap_or("Misc"))
                    .join(&theme.domain)
                    .join(&theme.mode);

                fs::create_dir_all(&virtual_path)?;
                let file_name = file.path.file_name().unwrap_or_default();
                let symlink_path = virtual_path.join(file_name);
                wanted.insert(symlink_path.clone());

                // Canonicalize the target: ingest walks CWD-relative paths
                // ("./doc.md"), and `symlink("./doc.md")` inside a nested VFS
                // dir resolves relative to the LINK's directory — producing a
                // dangling link. Absolute targets make links location-true.
                let target_abs = file
                    .path
                    .canonicalize()
                    .unwrap_or_else(|_| file.path.clone());

                // `.exists()` FOLLOWS symlinks, so a link whose target was
                // deleted/renamed (dangling) reports false and `symlink()`
                // then fails with EEXIST — crashing every re-ingest after
                // the source file moves. Check the link itself instead.
                match fs::symlink_metadata(&symlink_path) {
                    Ok(meta) => {
                        if meta.is_symlink() {
                            // Re-point stale/broken links at the current target.
                            #[cfg(unix)]
                            {
                                if fs::read_link(&symlink_path)
                                    .map(|t| t != target_abs)
                                    .unwrap_or(true)
                                {
                                    let _ = fs::remove_file(&symlink_path);
                                    std::os::unix::fs::symlink(&target_abs, &symlink_path)?;
                                }
                            }
                        }
                        // Regular file/dir already occupying the path: leave it alone.
                    }
                    Err(_) => {
                        #[cfg(unix)]
                        std::os::unix::fs::symlink(&target_abs, &symlink_path)?;
                    }
                }

                self.reference_map.push(VfsReference {
                    virtual_path: symlink_path,
                    physical_path: file.path.clone(),
                    last_synced: chrono::Utc::now(),
                });
            }
        }

        // Prune dangling leftovers: any symlink under vfs_root that this sync
        // did not (re)create points at a renamed/deleted source. Remove it so
        // repeated ingests converge instead of accumulating broken links.
        prune_stale_links(&vfs_root, &wanted)?;

        Ok(())
    }

    /// Checks if a file move within the virtual structure was intentional.
    pub fn verify_vfs_move(
        &mut self,
        virtual_link: &Path,
        physical_source: &Path,
    ) -> anyhow::Result<bool> {
        println!(
            "Detected move of virtual link to: {}",
            virtual_link.display()
        );
        print!("Was this move intentional? (y/n): ");
        use std::io::{self, Write};
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() == "y" {
            // Update reference and trigger classification update
            if let Some(ref_idx) = self
                .reference_map
                .iter()
                .position(|r| r.physical_path == physical_source)
            {
                self.reference_map[ref_idx].virtual_path = virtual_link.to_path_buf();
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Walk `root` and remove every symlink not present in `keep`. Regular files
/// and directories are left untouched — only VFS-owned links are pruned.
fn prune_stale_links(root: &Path, keep: &std::collections::HashSet<PathBuf>) -> anyhow::Result<()> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue, // vanished between sync and prune: fine
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // symlink_metadata does NOT follow links: dangling links are seen.
            let meta = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_symlink() {
                if !keep.contains(&path) {
                    let _ = fs::remove_file(&path);
                }
            } else if meta.is_dir() {
                stack.push(path);
            }
        }
    }
    Ok(())
}
