use physis_pro::actor::PDCActor;
use physis_pro::config::{OntologyLoader, PhysisConfig};
use physis_pro::dream::DreamEngine;
use physis_pro::embed::RandomProjectionEmbedder;
use physis_pro::embed::VectorEmbed;
use physis_pro::mapper::OntologyMapper;
use physis_pro::scanner;

#[test]
fn test_mediation_pipeline() {
    let dir = std::env::temp_dir().join("physis_smoke_test");
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("main.rs"), "fn main() { println!(\"hello\"); }").unwrap();
    std::fs::write(dir.join("README.md"), "# Project\nDocs here.").unwrap();
    std::fs::write(dir.join("config.json"), r#"{"key": "value"}"#).unwrap();

    // Step 1: scan_project
    let files = scanner::scan_project(&dir, None);
    assert!(!files.is_empty(), "scanner must find files");
    assert!(
        files.iter().any(|f| f.path.ends_with("main.rs")),
        "must find main.rs"
    );
    assert!(
        files.iter().any(|f| f.path.ends_with("README.md")),
        "must find README.md"
    );

    // Step 2: OntologyMapper
    let config = PhysisConfig::default();
    let ontology = OntologyLoader::load_all(&config);
    let mut mapper = OntologyMapper::new(ontology, 64);
    let goals = mapper.map_filesystem(&dir, None);
    assert!(!goals.is_empty(), "mapper must produce goals");

    // Step 3: PDCActor
    let actor = PDCActor::new(10.0, 5);
    let stats = actor.stats(&goals);
    assert_eq!(stats.total_actions, 0, "new actor has no actions");
    assert!(
        (stats.avg_grade - 0.0).abs() < 0.001,
        "default avg grade is 0.0"
    );

    // Step 4: Query the trie
    let _results = mapper.query("main");

    // Step 5: DreamEngine
    let mut dream_engine = DreamEngine::new();
    let dreams = dream_engine.generate_dreams(&goals, 3);
    assert_eq!(dreams.len(), 3, "must generate exactly 3 dreams");

    // Step 6: Embed determinism
    let embedder = RandomProjectionEmbedder::new(64);
    for goal in &goals {
        let v1 = embedder.embed(&goal.id);
        let v2 = embedder.embed(&goal.id);
        assert_eq!(v1, v2, "embedding must be deterministic per goal");
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}
