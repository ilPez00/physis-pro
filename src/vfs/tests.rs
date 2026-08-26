#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::graph::FileThemeGraph;
    use crate::vfs::orchestrator::VfsOrchestrator;
    use std::fs;

    #[test]
    fn test_vfs_sync() {
        let dir = tempfile::tempdir().unwrap();
        let mut orch = VfsOrchestrator::new(dir.path().to_path_buf());

        let mut graph = FileThemeGraph::new();
        graph.add_theme("Test", Some("Cat"), "DOM", "MODE", "axis", vec![0.1; 4]);
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();
        graph.add_file(file_path.clone(), "Test", "Report", vec![0.1; 4], None);
        graph.add_edge(0, 0, 1.0);

        orch.sync_graph(&graph).unwrap();

        let vfs_path = dir.path().join("virtual/Cat/DOM/MODE/test.txt");
        assert!(vfs_path.exists());
    }

    /// Regression (QA 8.1): re-ingesting after the source file was renamed must
    /// not crash with EEXIST. The old code used `.exists()` which follows
    /// symlinks — a dangling link reported "absent" and `symlink()` then hit
    /// the existing link inode. `sync_graph` must also REPAIR the stale link.
    #[test]
    fn test_vfs_resync_after_source_rename_repairs_dangling_link() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();

        let make_graph = |target: &std::path::Path| {
            let mut graph = FileThemeGraph::new();
            graph.add_theme("Test", Some("Cat"), "DOM", "MODE", "axis", vec![0.1; 4]);
            graph.add_file(target.to_path_buf(), "Test", "Report", vec![0.1; 4], None);
            graph.add_edge(0, 0, 1.0);
            graph
        };

        // First sync: link created.
        let mut orch = VfsOrchestrator::new(dir.path().to_path_buf());
        orch.sync_graph(&make_graph(&file_path)).unwrap();

        // Source renamed → the previously created symlink is now dangling.
        let renamed_path = dir.path().join("renamed.txt");
        std::fs::rename(&file_path, &renamed_path).unwrap();

        // Second sync MUST succeed (used to fail: File exists os error 17)…
        orch.sync_graph(&make_graph(&renamed_path)).unwrap();

        // …and the NEW link for the renamed file must resolve.
        let vfs_path_new = dir.path().join("virtual/Cat/DOM/MODE/renamed.txt");
        assert!(
            vfs_path_new.exists(),
            "link for renamed file should resolve"
        );
        assert_eq!(fs::read_link(&vfs_path_new).unwrap(), renamed_path);

        // …while the STALE link for the old file name must be pruned —
        // sync_graph mirrors the graph, it never leaves dangling links behind.
        let vfs_path_old = dir.path().join("virtual/Cat/DOM/MODE/test.txt");
        assert!(
            fs::symlink_metadata(&vfs_path_old).is_err(),
            "stale link for removed file name should be pruned"
        );
    }

    /// Re-running sync with unchanged inputs is idempotent and keeps the link valid.
    #[test]
    fn test_vfs_resync_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();

        let mut graph = FileThemeGraph::new();
        graph.add_theme("Test", Some("Cat"), "DOM", "MODE", "axis", vec![0.1; 4]);
        graph.add_file(file_path.clone(), "Test", "Report", vec![0.1; 4], None);
        graph.add_edge(0, 0, 1.0);

        let mut orch = VfsOrchestrator::new(dir.path().to_path_buf());
        orch.sync_graph(&graph).unwrap();
        orch.sync_graph(&graph).unwrap(); // must not error

        let vfs_path = dir.path().join("virtual/Cat/DOM/MODE/test.txt");
        assert!(vfs_path.exists());
        assert_eq!(
            fs::read_link(&vfs_path).unwrap(),
            file_path,
            "unchanged target should keep pointing at the same path"
        );
    }
}
