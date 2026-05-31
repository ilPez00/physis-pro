//! Orchestrates the creation and maintenance of symbolic links for the VFS.

use std::fs;
use std::path::{Path, PathBuf};
use crate::graph::FileThemeGraph;
use crate::vfs::VfsReference;

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

        for file in &graph.files {
            let file_themes: Vec<&crate::graph::ThemeNode> = graph.edges.iter()
                .filter(|e| e.file_id == file.id)
                .filter_map(|e| graph.get_theme(e.theme_id))
                .collect();

            for theme in file_themes {
                let virtual_path = vfs_root
                    .join(&theme.category.as_deref().unwrap_or("Misc"))
                    .join(&theme.domain)
                    .join(&theme.mode);

                fs::create_dir_all(&virtual_path)?;
                let symlink_path = virtual_path.join(file.path.file_name().unwrap());

                if !symlink_path.exists() {
                    #[cfg(unix)]
                    std::os::unix::fs::symlink(&file.path, &symlink_path)?;
                }

                self.reference_map.push(VfsReference {
                    virtual_path: symlink_path,
                    physical_path: file.path.clone(),
                    last_synced: chrono::Utc::now(),
                });
            }
        }
        Ok(())
    }

    /// Checks if a file move within the virtual structure was intentional.
    pub fn verify_vfs_move(&mut self, virtual_link: &Path, physical_source: &Path) -> anyhow::Result<bool> {
        println!("Detected move of virtual link to: {}", virtual_link.display());
        print!("Was this move intentional? (y/n): ");
        use std::io::{self, Write};
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if input.trim().to_lowercase() == "y" {
            // Update reference and trigger classification update
            if let Some(ref_idx) = self.reference_map.iter().position(|r| r.physical_path == physical_source) {
                self.reference_map[ref_idx].virtual_path = virtual_link.to_path_buf();
                return Ok(true);
            }
        }
        Ok(false)
    }
}
