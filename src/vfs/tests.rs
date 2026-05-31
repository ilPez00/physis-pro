#[cfg(test)]
mod tests {
    use crate::vfs::orchestrator::VfsOrchestrator;
    use crate::graph::FileThemeGraph;
    use std::path::PathBuf;

    #[test]
    fn test_vfs_sync() {
        let dir = tempfile::tempdir().unwrap();
        let mut orch = VfsOrchestrator::new(dir.path().to_path_buf());
        
        let mut graph = FileThemeGraph::new();
        graph.add_theme("Test", Some("Cat"), "DOM", "MODE", "axis", vec![0.1; 4]);
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "content").unwrap();
        graph.add_file(file_path.clone(), "Test", "Report", vec![0.1; 4]);
        graph.add_edge(0, 0, 1.0);
        
        orch.sync_graph(&graph).unwrap();
        
        let vfs_path = dir.path().join("virtual/Cat/DOM/MODE/test.txt");
        assert!(vfs_path.exists());
    }
}
