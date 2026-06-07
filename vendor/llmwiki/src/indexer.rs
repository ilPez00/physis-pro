use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use anyhow::{Context, Result};
use hex::encode as hex_encode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::inverted;
use crate::parser;
#[cfg(all(not(test), feature = "semantic"))]
use crate::semantic;

#[derive(Serialize, Deserialize, Clone)]
pub struct FileMeta {
    pub id: u32,
    pub path: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub confidence: Option<f64>,
    pub updated: Option<String>,
}

pub struct IndexStats {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub skipped: usize,
}

pub fn ensure_index_dirs(wiki_root: &Path) -> Result<()> {
    let index_dir = wiki_root.join(".index");
    let tantivy_dir = index_dir.join("tantivy");
    fs::create_dir_all(&tantivy_dir)
        .with_context(|| format!("Failed to create {}", tantivy_dir.display()))?;
    Ok(())
}

pub fn index_root(wiki_root: &Path, incremental: bool) -> Result<IndexStats> {
    ensure_index_dirs(wiki_root)?;

    let index_dir = wiki_root.join(".index");
    let checksum_path = index_dir.join("checksum.json");
    let files_json_path = index_dir.join("files.json");
    let meta_jsonl_path = index_dir.join("meta.jsonl");

    // Load existing checksums: path -> sha256 hex
    let old_checksums: HashMap<String, String> = if checksum_path.exists() {
        let data = fs::read_to_string(&checksum_path)?;
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        HashMap::new()
    };

    // Load existing file id map: path -> id
    let file_ids: HashMap<String, u32> = if files_json_path.exists() {
        let data = fs::read_to_string(&files_json_path)?;
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        HashMap::new()
    };

    // Load existing meta entries by path for incremental use
    let existing_meta: HashMap<String, FileMeta> = if meta_jsonl_path.exists() && incremental {
        load_meta(wiki_root)?
            .into_iter()
            .map(|m| (m.path.clone(), m))
            .collect()
    } else {
        HashMap::new()
    };

    let mut stats = IndexStats {
        added: 0,
        updated: 0,
        removed: 0,
        skipped: 0,
    };

    let mut new_checksums: HashMap<String, String> = HashMap::new();
    let mut new_file_ids: HashMap<String, u32> = HashMap::new();
    let mut result_metas: Vec<FileMeta> = Vec::new();
    let mut next_id: u32 = file_ids.values().copied().max().unwrap_or(0) + 1;

    // Walk all .md files, skipping .index/
    let walker = WalkDir::new(wiki_root)
        .into_iter()
        .filter_entry(|e| {
            // Skip .index directory
            let name = e.file_name().to_string_lossy();
            !(e.file_type().is_dir() && name == ".index")
        })
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().map(|x| x == "md").unwrap_or(false)
        });

    let mut seen_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

    for entry in walker {
        let abs_path = entry.path();
        let rel_path = abs_path
            .strip_prefix(wiki_root)
            .unwrap_or(abs_path)
            .to_string_lossy()
            .replace('\\', "/");

        seen_paths.insert(rel_path.clone());

        // Compute checksum
        let content = fs::read_to_string(abs_path)
            .with_context(|| format!("Failed to read {}", abs_path.display()))?;
        let hash = hex_encode(Sha256::digest(content.as_bytes()));

        let old_hash = old_checksums.get(&rel_path);
        new_checksums.insert(rel_path.clone(), hash.clone());

        if incremental && old_hash.map(|h| h == &hash).unwrap_or(false) {
            // Unchanged: reuse existing meta
            if let Some(meta) = existing_meta.get(&rel_path) {
                new_file_ids.insert(rel_path.clone(), meta.id);
                result_metas.push(meta.clone());
                stats.skipped += 1;
                continue;
            }
        }

        // Parse frontmatter
        match parser::parse_frontmatter(&content) {
            Ok(page_meta) => {
                let id = *file_ids.get(&rel_path).unwrap_or(&next_id);
                if !file_ids.contains_key(&rel_path) {
                    next_id += 1;
                }
                new_file_ids.insert(rel_path.clone(), id);

                let is_new = old_hash.is_none();
                if is_new {
                    stats.added += 1;
                } else {
                    stats.updated += 1;
                }

                result_metas.push(FileMeta {
                    id,
                    path: rel_path,
                    title: page_meta.title,
                    description: page_meta.description,
                    tags: page_meta.tags,
                    confidence: page_meta.confidence,
                    updated: page_meta.updated,
                });
            }
            Err(e) => {
                eprintln!("Warning: skipping {} — {}", rel_path, e);
                stats.skipped += 1;
            }
        }
    }

    // Detect removed files
    for path in old_checksums.keys() {
        if !seen_paths.contains(path) {
            stats.removed += 1;
        }
    }

    // Write meta.jsonl
    write_jsonl(&meta_jsonl_path, &result_metas)?;

    // Write files.json atomically
    write_json_atomic(&files_json_path, &new_file_ids)?;

    // Write checksum.json atomically
    write_json_atomic(&checksum_path, &new_checksums)?;

    // Rebuild Tantivy index: always on full index, only if files changed on incremental
    let should_rebuild = !incremental
        || stats.added > 0
        || stats.updated > 0
        || stats.removed > 0;
    if should_rebuild {
        inverted::build_tantivy_index(wiki_root, &result_metas)?;
        #[cfg(all(not(test), feature = "semantic"))]
        {
            eprintln!("Building vector index...");
            semantic::build_vector_index(wiki_root, &result_metas)?;
        }
    }

    Ok(stats)
}

pub fn load_meta(wiki_root: &Path) -> Result<Vec<FileMeta>> {
    let meta_path = wiki_root.join(".index").join("meta.jsonl");
    if !meta_path.exists() {
        return Ok(Vec::new());
    }
    let file = fs::File::open(&meta_path)
        .with_context(|| format!("Failed to open {}", meta_path.display()))?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let meta: FileMeta = serde_json::from_str(&line)
            .with_context(|| format!("Failed to parse meta line: {}", line))?;
        entries.push(meta);
    }
    Ok(entries)
}

fn write_jsonl(path: &Path, entries: &[FileMeta]) -> Result<()> {
    let tmp_path = path.with_extension("jsonl.tmp");
    {
        let mut file = fs::File::create(&tmp_path)?;
        for entry in entries {
            let line = serde_json::to_string(entry)?;
            writeln!(file, "{}", line)?;
        }
        file.flush()?;
    }
    fs::rename(&tmp_path, path)?;
    Ok(())
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let tmp_path = path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&tmp_path)?;
        let data = serde_json::to_string_pretty(value)?;
        file.write_all(data.as_bytes())?;
        file.flush()?;
    }
    fs::rename(&tmp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_wiki(dir: &Path) {
        let bio = dir.join("bio");
        fs::create_dir_all(&bio).unwrap();
        fs::write(
            bio.join("test.md"),
            "---\ntitle: Test\ndescription: A test page.\ntags: [test]\nconfidence: 0.8\n---\n\n# Test\n",
        )
        .unwrap();
    }

    #[test]
    fn test_index_and_load() {
        let tmp = TempDir::new().unwrap();
        make_wiki(tmp.path());
        let stats = index_root(tmp.path(), false).unwrap();
        assert_eq!(stats.added, 1);
        assert_eq!(stats.skipped, 0);

        let metas = load_meta(tmp.path()).unwrap();
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].title, "Test");
    }

    #[test]
    fn test_incremental_skip() {
        let tmp = TempDir::new().unwrap();
        make_wiki(tmp.path());
        index_root(tmp.path(), false).unwrap();
        let stats = index_root(tmp.path(), true).unwrap();
        assert_eq!(stats.skipped, 1);
        assert_eq!(stats.added, 0);
    }
}
