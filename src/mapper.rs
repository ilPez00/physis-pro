//! Maps filesystem trees into ontological trie structures with vector embeddings.
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::config::OntologyLoader;
use crate::embed::{RandomProjectionEmbedder, VectorEmbed};
use crate::models::Goal;
use crate::scanner::{self, FileInfo};
use crate::trie::DynamicVectorTrie;

/// Maps filesystem paths and goals into a dynamic vector trie for semantic search.
#[derive(Debug, Clone)]
pub struct OntologyMapper {
    pub ontology: OntologyLoader,
    pub trie: DynamicVectorTrie,
    embed_dim: usize,
}

impl OntologyMapper {
    /// Creates a new mapper with the given ontology definition and embedding dimension.
    pub fn new(ontology: OntologyLoader, embed_dim: usize) -> Self {
        Self {
            ontology,
            trie: DynamicVectorTrie::new(),
            embed_dim,
        }
    }

    /// Scans a directory and inserts file paths as tokens into the trie, returning Goal vectors.
    pub fn map_filesystem(
        &mut self,
        root_dir: &Path,
        extra_exclude: Option<&HashSet<String>>,
    ) -> Vec<Goal> {
        self.map_filesystem_labeled(root_dir, extra_exclude).0
    }

    /// Like `map_filesystem`, but also returns each file's readable structural
    /// path (parallel to the goals). Callers can register these as labeled core
    /// nodes so scanned files become recallable by content in the wiki.
    pub fn map_filesystem_labeled(
        &mut self,
        root_dir: &Path,
        extra_exclude: Option<&HashSet<String>>,
    ) -> (Vec<Goal>, Vec<String>) {
        let files = scanner::scan_project(root_dir, extra_exclude);
        let mut goals = Vec::new();
        let mut labels = Vec::new();
        let embedder = RandomProjectionEmbedder::new(self.embed_dim);

        for file in &files {
            let path_str = file.structural_path();
            let tids = self.trie.tokenize_path_mut(&file.path);
            self.trie.insert(&tids);

            let embedding = embedder.embed(&path_str);
            goals.push(Goal::new_vec(embedding));

            for line in &file.structural_lines {
                let line_path = format!("{} → {}", path_str, line);
                let line_tids = self.trie.tokenize_mut(&line_path);
                self.trie.insert(&line_tids);
            }

            labels.push(path_str);
        }

        (goals, labels)
    }

    /// Inserts goal embeddings into the trie as tokenized paths.
    pub fn map_goals_to_trie(&mut self, goals: &[Goal]) {
        for goal in goals {
            let embedding_str: String = goal.embedding.iter()
                .map(|v| format!("{:.4}", v))
                .take(8)
                .collect::<Vec<_>>()
                .join(" ");
            let tokens = self.trie.tokenize_mut(&embedding_str);
            self.trie.insert(&tokens);
        }
    }

    #[allow(dead_code)]
    fn resolve_domain_for_file(&self, file: &FileInfo) -> String {
        let ext_no_dot = file.ext.trim_start_matches('.');
        if ext_no_dot.is_empty() {
            return "file".to_string();
        }

        let lang_domain = match ext_no_dot {
            "rs" | "py" | "kt" | "java" | "ts" | "js" | "go" | "rb" | "swift" | "c" | "cpp" | "h" | "hpp" => "code",
            "md" | "txt" | "rst" | "adoc" => "documentation",
            "json" | "yaml" | "yml" | "toml" | "xml" => "config",
            "html" | "css" | "scss" | "less" => "web",
            "sh" | "bash" | "zsh" => "script",
            "sql" | "graphql" | "prisma" => "data",
            _ => "file",
        };

        let lower_path = file.path.to_lowercase();
        for def in self.ontology.machine_domains.values() {
            if def.hints.iter().any(|h| lower_path.contains(h)) {
                return def.name.clone();
            }
        }

        lang_domain.to_string()
    }

    /// Searches the trie for paths matching a whitespace-delimited query string.
    pub fn query(&self, query: &str) -> Vec<Vec<String>> {
        let words: Vec<&str> = query.split_whitespace().collect();
        let tids: Vec<u32> = words
            .iter()
            .filter_map(|w| self.trie.token_id(w))
            .collect();
        if tids.is_empty() {
            return vec![];
        }
        self.trie.prefix_search(&tids, 2, 10)
    }

    /// Returns trie statistics plus domain counts from the ontology.
    pub fn stats(&self) -> HashMap<String, usize> {
        let mut s = self.trie.stats();
        s.insert("human_domains".to_string(), self.ontology.human_domains.len());
        s.insert("machine_domains".to_string(), self.ontology.machine_domains.len());
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PhysisConfig;

    #[test]
    fn test_mapper_filesystem() {
        let config = PhysisConfig::default();
        let ontology = OntologyLoader::load_all(&config);
        let mut mapper = OntologyMapper::new(ontology, 64);

        let dir = std::env::temp_dir().join("physis_mapper_test");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("test.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.join("note.md"), "# Hello").unwrap();

        let goals = mapper.map_filesystem(&dir, None);
        assert!(!goals.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_query() {
        let config = PhysisConfig::default();
        let ontology = OntologyLoader::load_all(&config);
        let mut mapper = OntologyMapper::new(ontology, 64);

        mapper.trie.insert_path("project/philosophy/identity");
        mapper.trie.insert_path("project/ai/memory");

        let results = mapper.query("project");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_map_empty_directory() {
        let config = PhysisConfig::default();
        let ontology = OntologyLoader::load_all(&config);
        let mut mapper = OntologyMapper::new(ontology, 64);

        let dir = std::env::temp_dir().join("physis_mapper_empty_test");
        let _ = std::fs::create_dir_all(&dir);

        let goals = mapper.map_filesystem(&dir, None);
        assert!(goals.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_query_no_match() {
        let config = PhysisConfig::default();
        let ontology = OntologyLoader::load_all(&config);
        let mapper = OntologyMapper::new(ontology, 64);

        let results = mapper.query("nonexistent_query");
        assert!(results.is_empty());
    }

    #[test]
    fn test_structural_lines_in_trie() {
        let config = PhysisConfig::default();
        let ontology = OntologyLoader::load_all(&config);
        let mut mapper = OntologyMapper::new(ontology, 64);

        let dir = std::env::temp_dir().join("physis_mapper_struct_test");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("code.rs"), "fn hello() {}\nstruct Foo {}").unwrap();

        mapper.map_filesystem(&dir, None);
        let s = mapper.stats();
        assert!(*s.get("terminal_nodes").unwrap() > 0, "trie should have terminal nodes from structural lines");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_map_goals_to_trie() {
        let config = PhysisConfig::default();
        let ontology = OntologyLoader::load_all(&config);
        let mut mapper = OntologyMapper::new(ontology, 64);

        let embedder = RandomProjectionEmbedder::new(64);
        let goals = vec![
            Goal::new_vec(embedder.embed("project feature")),
            Goal::new_vec(embedder.embed("docs readme")),
        ];
        mapper.map_goals_to_trie(&goals);

        let s = mapper.stats();
        assert!(*s.get("nodes").unwrap() > 1, "trie should have nodes after inserting goals");
    }

    #[test]
    fn test_stats_after_mapping() {
        let config = PhysisConfig::default();
        let ontology = OntologyLoader::load_all(&config);
        let mut mapper = OntologyMapper::new(ontology, 64);

        mapper.trie.insert_path("test/stats/path");
        let s = mapper.stats();
        assert!(*s.get("human_domains").unwrap() >= 14, "human domains should be at least 14, got {}", s.get("human_domains").unwrap());
        assert!(*s.get("machine_domains").unwrap() >= 53, "machine domains should be at least 53, got {}", s.get("machine_domains").unwrap());
        assert!(*s.get("nodes").unwrap() > 1);
    }
}
