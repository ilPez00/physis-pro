//! llmwiki/uberwiki bridge — searches the project wiki for context.
//! Uses BM25 (Tantivy) + semantic (fastembed) with RRF fusion when available.
//! Falls back to simple filesystem grep when the wiki binary is not running.
//! Also searches the FileThemeGraph for matching themes and files.

use std::path::Path;
use std::process::Command;

use crate::graph::FileThemeGraph;

/// Combined wiki search — queries the llmwiki index AND the file-theme graph.
/// Returns formatted results from both sources.
pub fn search_all(query: &str, wiki_path: &Path, graph_path: &Path) -> Vec<String> {
    let mut results = Vec::new();

    // 1. Search the llmwiki
    let wiki_results = search_wiki(query, wiki_path);
    for r in wiki_results {
        results.push(format!("[wiki] {}", r));
    }

    // 2. Search the file-theme graph
    if graph_path.exists() {
        if let Ok(graph) = FileThemeGraph::load(graph_path) {
            let graph_results = search_graph(query, &graph);
            for r in graph_results {
                results.push(format!("[graph] {}", r));
            }
        }
    }

    results
}

/// Search the llmwiki index for a query.
/// Tries to call the `wiki` CLI binary first; falls back to grep.
pub fn search_wiki(query: &str, wiki_path: &Path) -> Vec<String> {
    // In-process Tantivy BM25 (no subprocess). Falls through to the binary/grep
    // on empty results or error. Gated behind `wiki-embedded`.
    #[cfg(feature = "wiki-embedded")]
    {
        match llmwiki_physis::search_bm25(wiki_path, query, 10) {
            Ok(results) if !results.is_empty() => {
                return llmwiki_physis::format_results(&results)
                    .lines()
                    .map(|l| l.to_string())
                    .collect();
            }
            Ok(_) => {}
            Err(e) => log::warn!("[wiki] in-process search failed: {e}; falling back"),
        }
    }

    let wiki_bin = which("wiki").or_else(|| which("llmwiki"));
    if let Some(bin) = wiki_bin {
        match Command::new(bin).arg("search").arg(query).arg("--path").arg(wiki_path).output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let lines: Vec<String> = stdout.lines().map(|l| l.to_string()).collect();
                if !lines.is_empty() {
                    return lines;
                }
            }
            Err(e) => {
                log::warn!("wiki binary failed: {e}, falling back to grep");
            }
        }
    }
    // Fallback: recursive grep
    let mut results = Vec::new();
    if wiki_path.exists() {
        let _ = walk_files(wiki_path, query, &mut results);
    }
    results
}

/// Search the FileThemeGraph for matching themes and files (uberwiki integration).
pub fn search_graph(query: &str, graph: &FileThemeGraph) -> Vec<String> {
    let mut results = Vec::new();
    let _q = query.to_lowercase();

    // Match themes
    let matching_themes = graph.search_themes_by_text(query);
    for theme in matching_themes {
        let file_count = graph.edges.iter().filter(|e| e.theme_id == theme.id).count();
        let cat = theme.category.as_deref().unwrap_or("uncategorized");
        results.push(format!("theme: {} [{}×{}] ({}, {} files)",
            theme.name, theme.domain, theme.mode, cat, file_count));
    }

    // Match files
    let matching_files = graph.search_files(query);
    for file in matching_files {
        let themes: Vec<&str> = graph.edges.iter()
            .filter(|e| e.file_id == file.id)
            .filter_map(|e| graph.get_theme(e.theme_id))
            .map(|t| t.name.as_str())
            .collect();
        results.push(format!("file: {} ({}) — themes: {}",
            file.title, file.path.display(), themes.join(", ")));
    }

    results
}

fn walk_files(dir: &Path, query: &str, results: &mut Vec<String>) -> std::io::Result<()> {
    let q = query.to_lowercase();
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_files(&path, query, results)?;
        } else if let Some(ext) = path.extension() {
            if ext == "md" || ext == "txt" {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if content.to_lowercase().contains(&q) {
                        results.push(format!("{}: ...{}...", path.display(),
                            &content[content.to_lowercase().find(&q)
                                .map(|i| i.saturating_sub(20))
                                .unwrap_or(0)..]
                                .chars().take(100).collect::<String>()));
                    }
                }
            }
        }
    }
    Ok(())
}

fn which(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).filter_map(|dir| {
            let full = dir.join(name);
            if full.is_file() { Some(full) } else { None }
        }).next()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::FileThemeGraph;

    #[test]
    fn test_search_wiki_nonexistent_path() {
        let results = search_wiki("test", Path::new("/nonexistent/wiki/path"));
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_graph_themes() {
        let mut g = FileThemeGraph::new();
        g.add_theme("Budget Planning", Some("Finance"), "CONSTRUCT", "CREATE", "office", vec![0.1, 0.9]);
        g.add_theme("Email Management", Some("Communication"), "BOND", "WORK", "office", vec![0.9, 0.1]);
        let results = search_graph("budget", &g);
        assert!(results.iter().any(|r| r.contains("Budget Planning")));
        assert!(!results.iter().any(|r| r.contains("Email Management")));
    }

    #[test]
    fn test_search_graph_files() {
        let mut g = FileThemeGraph::new();
        g.add_theme("Budget", Some("Finance"), "CONSTRUCT", "CREATE", "office", vec![0.1, 0.9]);
        g.add_file(std::path::PathBuf::from("budget.xlsx"), "Q4 Budget", "Spreadsheet", vec![0.1, 0.9]);
        g.add_edge(0, 0, 0.95);
        let results = search_graph("Q4", &g);
        assert!(results.iter().any(|r| r.contains("Q4 Budget")));
    }

    #[test]
    fn test_search_graph_no_match() {
        let g = FileThemeGraph::new();
        let results = search_graph("nonexistent", &g);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_all_empty_graph() {
        let dir = tempfile::tempdir().unwrap();
        let graph_path = dir.path().join("graph.json");
        let wiki_path = dir.path().join("wiki");
        let results = search_all("test", &wiki_path, &graph_path);
        assert!(results.is_empty());
    }
}
