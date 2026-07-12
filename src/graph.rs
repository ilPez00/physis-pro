use std::collections::HashMap;
use std::path::PathBuf;

use slotmap::{DenseSlotMap, new_key_type, Key, KeyData};
use serde::{Serialize, Deserialize};
use bytemuck::{Pod, Zeroable};
use crossbeam_channel::{Sender, unbounded};
use crate::ai::onnx_worker::{OnnxRequest, OnnxResponse};
use crate::storage::MmappedStorage;

new_key_type! {
    pub struct NodeKey;
}

/// Pod-compatible key representation.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, Serialize, Deserialize)]
pub struct RawNodeKey {
    pub data: u64,
}

impl From<NodeKey> for RawNodeKey {
    fn from(key: NodeKey) -> Self {
        Self { data: key.data().as_ffi() }
    }
}

impl From<RawNodeKey> for NodeKey {
    fn from(raw: RawNodeKey) -> Self {
        KeyData::from_ffi(raw.data).into()
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, Serialize, Deserialize)]
pub struct NodePayload {
    pub activation_energy: f32,
    pub decay_metrics: f32,
    pub prune_flag: u8,
    pub compress_flag: u8,
    pub _padding: [u8; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, Serialize, Deserialize)]
pub struct Edge {
    pub source: RawNodeKey,
    pub target: RawNodeKey,
    pub weight: f32,
    pub semantic_type: u32,
}

#[derive(Debug)]
pub struct HolonicGraph {
    pub nodes: DenseSlotMap<NodeKey, NodePayload>,
    pub edges: Vec<Edge>,
    pub embeddings: DenseSlotMap<NodeKey, Vec<f32>>,
}

impl Default for HolonicGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl HolonicGraph {
    pub fn new() -> Self {
        Self {
            nodes: DenseSlotMap::with_key(),
            edges: Vec::new(),
            embeddings: DenseSlotMap::with_key(),
        }
    }

    pub fn add_node(&mut self, payload: NodePayload, embedding: Vec<f32>) -> NodeKey {
        let key = self.nodes.insert(payload);
        self.embeddings.insert_with_key(|_| embedding);
        key
    }

    /// Phase IV: Structural Dreaming & Pruning.
    /// Purges transient edges and decays activation energy.
    pub fn dream_cycle(&mut self, confidence_threshold: f32) {
        // 1. Prune edges below threshold
        self.edges.retain(|e| e.weight > confidence_threshold);

        // 2. Decay activation energy and apply prune flags
        for (_key, payload) in self.nodes.iter_mut() {
            payload.activation_energy *= 0.95; // 5% decay per cycle
            if payload.activation_energy < 0.1 {
                payload.prune_flag = 1;
            }
        }

        // 3. Physical purge of flagged nodes
        let to_remove: Vec<NodeKey> = self.nodes.iter()
            .filter(|(_, p)| p.prune_flag == 1)
            .map(|(k, _)| k)
            .collect();

        for key in to_remove {
            self.nodes.remove(key);
            self.embeddings.remove(key);
            // Remove associated edges
            let raw_key = RawNodeKey::from(key);
            self.edges.retain(|e| e.source.data != raw_key.data && e.target.data != raw_key.data);
        }
    }

    pub fn save_to_mmap(&self, storage: &mut MmappedStorage) -> anyhow::Result<()> {
        let nodes_vec: Vec<NodePayload> = self.nodes.values().cloned().collect();
        let nodes_bytes = bytemuck::cast_slice(&nodes_vec);
        if nodes_bytes.len() <= storage.mmap.len() {
            storage.mmap[0..nodes_bytes.len()].copy_from_slice(nodes_bytes);
        }
        Ok(())
    }
}

/// Zero-copy accessor for the memory-mapped graph file.
pub struct MmappedGraph<'a> {
    pub storage: &'a mut MmappedStorage,
}

impl<'a> MmappedGraph<'a> {
    pub fn new(storage: &'a mut MmappedStorage) -> Self {
        Self { storage }
    }

    pub fn nodes_mut(&mut self) -> &mut [NodePayload] {
        let header = *self.storage.header_mut();
        let offset = std::mem::size_of::<crate::storage::GraphHeader>();
        self.storage.as_slice_mut(offset, header.node_count as usize)
    }

    pub fn edges_mut(&mut self) -> &mut [Edge] {
        let header = *self.storage.header_mut();
        let node_offset = std::mem::size_of::<crate::storage::GraphHeader>();
        let edge_offset = node_offset + (header.node_count as usize * std::mem::size_of::<NodePayload>());
        self.storage.as_slice_mut(edge_offset, header.edge_count as usize)
    }
}

pub struct IngestMessage {
    pub signal_type: u8,
    pub payload: Vec<u8>,
}

/// The Ingest Holon entry point.
pub struct IngestRing {
    tx: Sender<IngestMessage>,
}

impl IngestRing {
    pub fn spawn(onnx_tx: Sender<OnnxRequest>, graph_tx: Sender<(NodePayload, Vec<f32>)>) -> Self {
        let (tx, rx) = unbounded::<IngestMessage>();
        
        std::thread::spawn(move || {
            let (reply_tx, reply_rx) = unbounded::<OnnxResponse>();
            while let Ok(msg) = rx.recv() {
                let _ = onnx_tx.send(OnnxRequest::IngestFilter {
                    signal_type: msg.signal_type,
                    raw_buffer: msg.payload,
                    reply_to: reply_tx.clone(),
                });
                
                if let Ok(OnnxResponse::IngestResult { cleaned_text: _, payload }) = reply_rx.recv() {
                    let dummy_embedding = vec![0.0; 32];
                    let _ = graph_tx.send((payload, dummy_embedding));
                }
            }
        });
        
        Self { tx }
    }

    pub fn push(&self, signal_type: u8, payload: Vec<u8>) {
        let _ = self.tx.send(IngestMessage { signal_type, payload });
    }
}

// ── File Theme Graph ──────────────────────────────────────────────────

/// A theme node in the file graph — corresponds to an ontology domain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeNode {
    pub id: usize,
    pub name: String,
    pub category: Option<String>,
    pub domain: String,
    pub mode: String,
    pub axis_kind: String,
    pub embedding: Vec<f32>,
}

/// A file node in the file graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    pub id: usize,
    pub path: PathBuf,
    pub title: String,
    pub kind: String,
    pub embedding: Vec<f32>,
}

/// Edge connecting a file to a theme with a relevance score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeFileEdge {
    pub theme_id: usize,
    pub file_id: usize,
    pub score: f32,
}

/// A browsable graph of files organized by theme.
/// Persisted as JSON for CLI browsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileThemeGraph {
    pub themes: Vec<ThemeNode>,
    pub files: Vec<FileNode>,
    pub edges: Vec<ThemeFileEdge>,
    pub theme_map: HashMap<String, usize>,
    next_theme_id: usize,
    next_file_id: usize,
}

impl FileThemeGraph {
    pub fn new() -> Self {
        Self {
            themes: Vec::new(),
            files: Vec::new(),
            edges: Vec::new(),
            theme_map: HashMap::new(),
            next_theme_id: 0,
            next_file_id: 0,
        }
    }

    pub fn add_theme(
        &mut self,
        name: &str,
        category: Option<&str>,
        domain: &str,
        mode: &str,
        axis_kind: &str,
        embedding: Vec<f32>,
    ) -> usize {
        if let Some(&id) = self.theme_map.get(name) {
            return id;
        }
        let id = self.next_theme_id;
        self.next_theme_id += 1;
        self.theme_map.insert(name.to_string(), id);
        self.themes.push(ThemeNode {
            id,
            name: name.to_string(),
            category: category.map(|s| s.to_string()),
            domain: domain.to_string(),
            mode: mode.to_string(),
            axis_kind: axis_kind.to_string(),
            embedding,
        });
        id
    }

    pub fn add_file(
        &mut self,
        path: PathBuf,
        title: &str,
        kind: &str,
        embedding: Vec<f32>,
    ) -> usize {
        let id = self.next_file_id;
        self.next_file_id += 1;
        self.files.push(FileNode {
            id,
            path,
            title: title.to_string(),
            kind: kind.to_string(),
            embedding,
        });
        id
    }

    pub fn add_edge(&mut self, theme_id: usize, file_id: usize, score: f32) {
        self.edges.push(ThemeFileEdge { theme_id, file_id, score });
    }

    pub fn list_categories(&self) -> Vec<String> {
        let mut cats: Vec<String> = self.themes.iter()
            .filter_map(|t| t.category.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        cats.sort();
        cats
    }

    pub fn themes_by_category(&self, category: &str) -> Vec<&ThemeNode> {
        let mut result: Vec<&ThemeNode> = self.themes.iter()
            .filter(|t| t.category.as_deref() == Some(category))
            .collect();
        result.sort_by(|a, b| a.name.cmp(&b.name));
        result
    }

    pub fn files_for_theme(&self, theme_id: usize) -> Vec<&FileNode> {
        let file_ids: std::collections::HashSet<usize> = self.edges.iter()
            .filter(|e| e.theme_id == theme_id)
            .map(|e| e.file_id)
            .collect();
        self.files.iter()
            .filter(|f| file_ids.contains(&f.id))
            .collect()
    }

    pub fn get_theme(&self, theme_id: usize) -> Option<&ThemeNode> {
        self.themes.iter().find(|t| t.id == theme_id)
    }

    pub fn find_related_themes(&self, theme_id: usize, threshold: f32) -> Vec<(usize, f32)> {
        let theme = match self.get_theme(theme_id) {
            Some(t) => t,
            None => return vec![],
        };
        let mut results: Vec<(usize, f32)> = self.themes.iter()
            .filter(|t| t.id != theme_id)
            .map(|t| {
                let sim = crate::models::cosine_sim(&theme.embedding, &t.embedding);
                (t.id, sim)
            })
            .filter(|(_, sim)| *sim > threshold)
            .collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    pub fn search_files(&self, query: &str) -> Vec<&FileNode> {
        let q = query.to_lowercase();
        let mut results: Vec<&FileNode> = self.files.iter()
            .filter(|f| {
                f.title.to_lowercase().contains(&q)
                    || f.path.to_string_lossy().to_lowercase().contains(&q)
                    || f.kind.to_lowercase().contains(&q)
            })
            .collect();
        results.sort_by(|a, b| a.title.cmp(&b.title));
        results
    }

    /// Search themes by name matching a text query (for wiki integration).
    pub fn search_themes_by_text(&self, query: &str) -> Vec<&ThemeNode> {
        let q = query.to_lowercase();
        let mut results: Vec<&ThemeNode> = self.themes.iter()
            .filter(|t| {
                t.name.to_lowercase().contains(&q)
                    || t.category.as_deref().map(|c| c.to_lowercase().contains(&q)).unwrap_or(false)
                    || t.domain.to_lowercase().contains(&q)
                    || t.mode.to_lowercase().contains(&q)
                    || t.axis_kind.to_lowercase().contains(&q)
            })
            .collect();
        results.sort_by(|a, b| a.name.cmp(&b.name));
        results
    }

    /// Export the graph to uberwiki markdown pages in a wiki directory.
    /// Creates an index page + per-category pages with theme details and file links.
    pub fn export_to_wiki(&self, wiki_dir: &std::path::Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(wiki_dir)?;

        // Index page
        let mut index = String::new();
        index.push_str("# File Theme Graph\n\n");
        index.push_str(&format!("- **{} themes** across {} categories\n", self.themes.len(), self.list_categories().len()));
        index.push_str(&format!("- **{} ingested files**\n", self.files.len()));
        index.push_str(&format!("- **{} theme-file edges**\n\n", self.edges.len()));

        let categories = self.list_categories();
        for cat in &categories {
            let themes = self.themes_by_category(cat);
            let safe_name = cat.to_lowercase().replace(' ', "_");
            index.push_str(&format!("- [[{}|{}]] — {} themes\n", safe_name, cat, themes.len()));
        }

        index.push_str("\n---\n\n### All Files\n\n");
        for file in &self.files {
            let themes: Vec<&str> = self.edges.iter()
                .filter(|e| e.file_id == file.id)
                .filter_map(|e| self.get_theme(e.theme_id))
                .map(|t| t.name.as_str())
                .collect();
            index.push_str(&format!("- **{}** — {} — {}\n", file.title, file.path.display(), themes.join(", ")));
        }

        std::fs::write(wiki_dir.join("index.md"), &index)?;

        // Per-category pages
        for cat in &categories {
            let safe_name = cat.to_lowercase().replace(' ', "_");
            let themes = self.themes_by_category(cat);
            let mut page = format!("# {} — File Theme Graph\n\n", cat);
            page.push_str(&format!("{} ontology domains\n\n", themes.len()));

            for theme in themes {
                page.push_str(&format!("## {}\n\n", theme.name));
                page.push_str(&format!("- **Domain:** {} × {}\n", theme.domain, theme.mode));
                page.push_str(&format!("- **Axis:** {} ({})\n", theme.axis_kind, theme.category.as_deref().unwrap_or("")));

                let files = self.files_for_theme(theme.id);
                if !files.is_empty() {
                    page.push_str("\n### Linked Files\n\n");
                    for file in &files {
                        page.push_str(&format!("- [[{}|{}]]\n", file.title, file.path.display()));
                    }
                }

                let related = self.find_related_themes(theme.id, 0.5);
                if !related.is_empty() {
                    page.push_str("\n### Related Themes\n\n");
                    for (rid, sim) in related.iter().take(5) {
                        if let Some(rt) = self.get_theme(*rid) {
                            let pct = (sim * 100.0) as u32;
                            page.push_str(&format!("- {} — {}% similar\n", rt.name, pct));
                        }
                    }
                }
                page.push('\n');
            }

            std::fs::write(wiki_dir.join(format!("{}.md", safe_name)), &page)?;
        }

        // Per-file pages
        if !self.files.is_empty() {
            let files_dir = wiki_dir.join("files");
            std::fs::create_dir_all(&files_dir)?;
            for file in &self.files {
                let safe_title = file.title.to_lowercase().replace(' ', "_");
                let mut page = format!("# {}\n\n", file.title);
                page.push_str(&format!("- **Path:** {}\n", file.path.display()));
                page.push_str(&format!("- **Kind:** {}\n", file.kind));

                let file_themes: Vec<&ThemeNode> = self.edges.iter()
                    .filter(|e| e.file_id == file.id)
                    .filter_map(|e| self.get_theme(e.theme_id))
                    .collect();

                if !file_themes.is_empty() {
                    page.push_str("\n### Themes\n\n");
                    for theme in &file_themes {
                        page.push_str(&format!("- [[{}|{}]]\n", theme.name.to_lowercase().replace(' ', "_"), theme.name));
                    }
                }

                std::fs::write(files_dir.join(format!("{}.md", safe_title)), &page)?;
            }
        }

        Ok(())
    }

    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let graph: FileThemeGraph = serde_json::from_str(&json)?;
        Ok(graph)
    }
}

impl Default for FileThemeGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a FileThemeGraph from a set of ingested docs and ontology domains.
/// Uses the provided embedder to classify each file against known themes.
pub fn build_theme_graph(
    docs: &[crate::ingest::IngestedDoc],
    embedder: &dyn crate::embed::VectorEmbed,
    _ontology: &crate::config::OntologyLoader,
    domains: &HashMap<String, crate::models::DomainDef>,
) -> FileThemeGraph {
    let mut graph = FileThemeGraph::new();

    // Add all ontology domains as theme nodes (with their text embeddings)
    for def in domains.values() {
        let mut text = def.name.clone();
        for hint in &def.hints {
            text.push(' ');
            text.push_str(hint);
        }
        let embedding = embedder.embed(&text);
        let domain_val = def.domain.as_deref().unwrap_or("");
        let mode_val = def.mode.as_deref().unwrap_or("");
        let axis_kind_val = def.axis_kind.as_deref().unwrap_or("");
        graph.add_theme(
            &def.name,
            def.category.as_deref(),
            domain_val,
            mode_val,
            axis_kind_val,
            embedding,
        );
    }

    // Classify each file and add edges
    for doc in docs {
        let file_embedding = embedder.embed(&doc.body);
        let file_id = graph.add_file(
            doc.path.clone(),
            &doc.title,
            &format!("{:?}", doc.kind),
            file_embedding.clone(),
        );

        // Find matching themes via cosine similarity
        let mut theme_scores: Vec<(usize, f32)> = graph.themes.iter()
            .map(|t| {
                let sim = crate::models::cosine_sim(&file_embedding, &t.embedding);
                (t.id, sim)
            })
            .filter(|(_, sim)| *sim > 0.25)
            .collect();
        theme_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        for (theme_id, score) in theme_scores.iter().take(5) {
            graph.add_edge(*theme_id, file_id, *score);
        }
    }

    graph
}

#[cfg(test)]
mod file_theme_graph_tests {
    use super::*;
    use crate::embed::RandomProjectionEmbedder;
    use crate::ingest::IngestedDoc;

    fn make_test_graph() -> FileThemeGraph {
        let mut g = FileThemeGraph::new();
        g.add_theme("Email", Some("Communication"), "BOND", "WORK", "office", vec![0.9, 0.1]);
        g.add_theme("Budget", Some("Finance"), "CONSTRUCT", "CREATE", "office", vec![0.1, 0.9]);
        g.add_file(PathBuf::from("inbox/msg.eml"), "Q4 report", "Email", vec![0.85, 0.15]);
        g.add_file(PathBuf::from("finance/budget.xlsx"), "Budget 2025", "Spreadsheet", vec![0.15, 0.85]);
        g.add_edge(0, 0, 0.95); // Email → Q4 report
        g.add_edge(1, 1, 0.90); // Budget → Budget 2025
        g
    }

    #[test]
    fn test_new_graph_empty() {
        let g = FileThemeGraph::new();
        assert!(g.themes.is_empty());
        assert!(g.files.is_empty());
    }

    #[test]
    fn test_add_theme_dedup() {
        let mut g = FileThemeGraph::new();
        let id1 = g.add_theme("Test", None, "STUDY", "WORK", "human", vec![0.5; 4]);
        let id2 = g.add_theme("Test", None, "STUDY", "WORK", "human", vec![0.5; 4]);
        assert_eq!(id1, id2);
        assert_eq!(g.themes.len(), 1);
    }

    #[test]
    fn test_categories() {
        let g = make_test_graph();
        let cats = g.list_categories();
        assert!(cats.contains(&"Communication".to_string()));
        assert!(cats.contains(&"Finance".to_string()));
    }

    #[test]
    fn test_files_for_theme() {
        let g = make_test_graph();
        let files = g.files_for_theme(0);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].title, "Q4 report");
    }

    #[test]
    fn test_search_files() {
        let g = make_test_graph();
        let results = g.search_files("budget");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Budget 2025");
    }

    #[test]
    fn test_roundtrip_json() {
        let g = make_test_graph();
        let json = serde_json::to_string(&g).unwrap();
        let loaded: FileThemeGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.themes.len(), 2);
        assert_eq!(loaded.files.len(), 2);
        assert_eq!(loaded.edges.len(), 2);
    }

    #[test]
    fn test_related_themes() {
        let mut g = FileThemeGraph::new();
        g.add_theme("A", None, "STUDY", "WORK", "human", vec![1.0, 0.0]);
        g.add_theme("B", None, "STUDY", "WORK", "human", vec![0.9, 0.1]); // similar to A
        g.add_theme("C", None, "BOND", "WALK", "human", vec![0.0, 1.0]); // orthogonal to A
        let related = g.find_related_themes(0, 0.5);
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].0, 1); // B is related to A
    }

    #[test]
    fn test_search_themes_by_text() {
        let g = make_test_graph();
        let results = g.search_themes_by_text("email");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Email");
    }

    #[test]
    fn test_export_to_wiki() {
        let g = make_test_graph();
        let dir = tempfile::tempdir().unwrap();
        let wiki_dir = dir.path().join("wiki");
        g.export_to_wiki(&wiki_dir).unwrap();

        assert!(wiki_dir.join("index.md").exists());
        assert!(wiki_dir.join("communication.md").exists());
        assert!(wiki_dir.join("finance.md").exists());
        assert!(wiki_dir.join("files").join("q4_report.md").exists());
    }

    #[test]
    fn test_build_from_docs() {
        let docs = vec![
            IngestedDoc {
                path: PathBuf::from("test.txt"),
                kind: crate::ingest::DocKind::Report,
                title: "Test".into(),
                body: "some financial report about budget planning".into(),
                timestamp: chrono::Utc::now(),
                ontology_tags: vec![],
                meta: crate::ingest::DocMeta::default(),
            },
        ];
        let embedder = RandomProjectionEmbedder::new(4);
        let config = crate::config::PhysisConfig::default();
        let ontology = crate::config::OntologyLoader::load_all(&config);
        let graph = build_theme_graph(&docs, &embedder, &ontology, &ontology.office_domains);
        assert!(!graph.themes.is_empty());
        // File should exist
        assert_eq!(graph.files.len(), 1);
    }
}
