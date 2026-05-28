//! llmwiki/uberwiki bridge — searches the project wiki for context.
//! Uses BM25 (Tantivy) + semantic (fastembed) with RRF fusion when available.
//! Falls back to simple filesystem grep when the wiki binary is not running.

use std::path::Path;
use std::process::Command;

/// Search the llmwiki index for a query.
/// Tries to call the `wiki` CLI binary first; falls back to grep.
pub fn search_wiki(query: &str, wiki_path: &Path) -> Vec<String> {
    let wiki_bin = which("wiki").or_else(|| which("llmwiki"));
    if let Some(bin) = wiki_bin {
        match Command::new(bin).arg("search").arg(query).arg("--path").arg(wiki_path).output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return stdout.lines().map(|l| l.to_string()).collect();
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

fn walk_files(dir: &Path, query: &str, results: &mut Vec<String>) -> std::io::Result<()> {
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
                    if content.to_lowercase().contains(&query.to_lowercase()) {
                        results.push(format!("{}: ...{}...", path.display(),
                            &content[content.to_lowercase().find(&query.to_lowercase())
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

    #[test]
    fn test_search_wiki_nonexistent_path() {
        let results = search_wiki("test", Path::new("/nonexistent/wiki/path"));
        assert!(results.is_empty());
    }
}
