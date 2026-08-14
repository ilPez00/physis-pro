use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use notify::event::ModifyKind;
use notify::{recommended_watcher, EventKind, RecursiveMode, Watcher};

use crate::indexer;

pub fn watch(wiki_root: &Path) -> Result<()> {
    let wiki_root = wiki_root
        .canonicalize()
        .unwrap_or_else(|_| wiki_root.to_path_buf());

    let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
    let mut watcher = recommended_watcher(move |res| {
        let _ = tx.send(res);
    })?;
    watcher.watch(&wiki_root, RecursiveMode::Recursive)?;
    eprintln!("Watching {} for changes...", wiki_root.display());

    let mut pending: HashSet<String> = HashSet::new();
    let mut last_event: Option<Instant> = None;
    let mut removed: Vec<String> = Vec::new();

    loop {
        match rx.try_recv() {
            Ok(Ok(event)) => {
                let relevant = matches!(
                    event.kind,
                    EventKind::Create(_)
                        | EventKind::Modify(ModifyKind::Data(_))
                        | EventKind::Remove(_)
                );
                if relevant {
                    for path in &event.paths {
                        if path.extension().map(|e| e == "md").unwrap_or(false) {
                            if let Ok(rel) = path.strip_prefix(&wiki_root) {
                                let rel_str = rel.to_string_lossy().replace('\\', "/");
                                if !rel_str.starts_with(".index/") {
                                    if matches!(event.kind, EventKind::Remove(_)) {
                                        removed.push(rel_str.clone());
                                    }
                                    pending.insert(rel_str);
                                    last_event = Some(Instant::now());
                                }
                            }
                        }
                    }
                }
            }
            Ok(Err(e)) => eprintln!("Watch error: {}", e),
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => break,
        }

        if let Some(last) = last_event {
            if last.elapsed() >= Duration::from_millis(500) && !pending.is_empty() {
                for rel in &removed {
                    let _ = remove_from_index(&wiki_root, rel);
                }
                removed.clear();

                let changed: Vec<String> = pending.drain().collect();
                let flush_start = Instant::now();

                match indexer::index_root(&wiki_root, true) {
                    Ok(_) => {
                        let elapsed = flush_start.elapsed().as_millis();
                        for path in &changed {
                            eprintln!("reindexed: {} ({}ms)", path, elapsed);
                        }
                        eprintln!("Index updated: {} files changed", changed.len());
                    }
                    Err(e) => eprintln!("Reindex error: {}", e),
                }

                last_event = None;
            }
        }

        thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}

fn remove_from_index(wiki_root: &Path, rel_path: &str) -> Result<()> {
    let index_dir = wiki_root.join(".index");

    let checksum_path = index_dir.join("checksum.json");
    if checksum_path.exists() {
        let data = fs::read_to_string(&checksum_path)?;
        let mut checksums: HashMap<String, String> =
            serde_json::from_str(&data).unwrap_or_default();
        checksums.remove(rel_path);
        let tmp = checksum_path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(&checksums)?)?;
        fs::rename(&tmp, &checksum_path)?;
    }

    let meta_path = index_dir.join("meta.jsonl");
    if meta_path.exists() {
        let content = fs::read_to_string(&meta_path)?;
        let filtered: Vec<&str> = content
            .lines()
            .filter(|line| {
                if line.trim().is_empty() {
                    return false;
                }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                    return v.get("path").and_then(|p| p.as_str()) != Some(rel_path);
                }
                true
            })
            .collect();
        let mut new_content = filtered.join("\n");
        if !new_content.is_empty() {
            new_content.push('\n');
        }
        let tmp = meta_path.with_extension("jsonl.tmp");
        fs::write(&tmp, new_content)?;
        fs::rename(&tmp, &meta_path)?;
    }

    Ok(())
}
