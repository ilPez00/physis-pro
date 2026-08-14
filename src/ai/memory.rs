use crate::ai::AiResult;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sled::Db;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub text: String,
    pub kind: String,
    pub mode: String,
    pub timestamp: i64,
    pub extra: HashMap<String, String>,
}

pub struct EpisodicMemory {
    db: Db,
}

impl EpisodicMemory {
    pub fn new(db: Db) -> Self {
        log::info!("episodic memory initialized");
        Self { db }
    }

    pub fn store(&self, text: &str, kind: &str, mode: &str, extra: HashMap<String, String>) -> AiResult<String> {
        let id = Uuid::new_v4().to_string();
        let entry = MemoryEntry {
            id: id.clone(),
            text: text.chars().take(2000).collect(),
            kind: kind.to_string(),
            mode: mode.to_string(),
            timestamp: Utc::now().timestamp(),
            extra,
        };
        let key = format!("mem:{id}");
        let value = serde_json::to_vec(&entry)?;
        self.db.insert(key.as_bytes(), value)?;
        self.db.flush()?;
        Ok(id)
    }

    pub fn query(&self, limit: usize) -> AiResult<Vec<MemoryEntry>> {
        let mut results = Vec::new();
        for item in self.db.scan_prefix("mem:".as_bytes()).rev() {
            let (_, value) = item?;
            if let Ok(entry) = serde_json::from_slice::<MemoryEntry>(&value) {
                results.push(entry);
                if results.len() >= limit {
                    break;
                }
            }
        }
        Ok(results)
    }

    pub fn query_by_kind(&self, kind: &str, limit: usize) -> AiResult<Vec<MemoryEntry>> {
        let mut results = Vec::new();
        for item in self.db.scan_prefix("mem:".as_bytes()).rev() {
            let (_, value) = item?;
            if let Ok(entry) = serde_json::from_slice::<MemoryEntry>(&value) {
                if entry.kind == kind {
                    results.push(entry);
                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }
        Ok(results)
    }

    pub fn search(&self, text: &str, limit: usize) -> AiResult<Vec<MemoryEntry>> {
        let query = text.to_lowercase();
        let mut results = Vec::new();
        for item in self.db.scan_prefix("mem:".as_bytes()).rev() {
            let (_, value) = item?;
            if let Ok(entry) = serde_json::from_slice::<MemoryEntry>(&value) {
                if entry.text.to_lowercase().contains(&query) {
                    results.push(entry);
                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }
        Ok(results)
    }

    pub fn count(&self) -> AiResult<u64> {
        let mut count = 0u64;
        for item in self.db.scan_prefix("mem:".as_bytes()) {
            let _ = item?;
            count += 1;
        }
        Ok(count)
    }

    pub fn delete_before(&self, days: i64) -> AiResult<u64> {
        let cutoff = Utc::now().timestamp() - (days * 86400);
        let mut deleted = 0u64;
        let to_remove: Vec<Vec<u8>> = self
            .db
            .scan_prefix("mem:".as_bytes())
            .filter_map(|item| {
                item.ok().and_then(|(key, value)| {
                    serde_json::from_slice::<MemoryEntry>(&value)
                        .ok()
                        .filter(|e| e.timestamp < cutoff)
                        .map(|_| key.to_vec())
                })
            })
            .collect();

        for key in &to_remove {
            self.db.remove(key)?;
            deleted += 1;
        }
        self.db.flush()?;
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn temp_memory() -> (EpisodicMemory, tempfile::TempDir) {
        let dir = tempfile::TempDir::with_prefix("physis-mem-test-").unwrap();
        let db: Db = sled::open(dir.path()).unwrap();
        (EpisodicMemory::new(db), dir)
    }

    #[test]
    fn test_store_and_query() {
        let (mem, _dir) = temp_memory();
        let id = mem.store("hello world", "chat", "COHERENCE", HashMap::new()).unwrap();
        let results = mem.query(10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
        assert_eq!(results[0].text, "hello world");
        assert_eq!(results[0].kind, "chat");
    }

    #[test]
    fn test_store_truncates_long_text() {
        let (mem, _dir) = temp_memory();
        let long = "x".repeat(3000);
        mem.store(&long, "test", "PLAN", HashMap::new()).unwrap();
        let results = mem.query(1).unwrap();
        assert_eq!(results[0].text.len(), 2000);
    }

    #[test]
    fn test_store_with_extra() {
        let (mem, _dir) = temp_memory();
        let mut extra = HashMap::new();
        extra.insert("source".into(), "test".into());
        mem.store("tagged", "log", "DO", extra.clone()).unwrap();
        let results = mem.query(1).unwrap();
        assert_eq!(results[0].extra, extra);
    }

    #[test]
    fn test_query_limit() {
        let (mem, _dir) = temp_memory();
        for i in 0..5 {
            mem.store(&format!("item {i}"), "bulk", "CHECK", HashMap::new()).unwrap();
        }
        let results = mem.query(3).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_query_by_kind() {
        let (mem, _dir) = temp_memory();
        mem.store("a", "alpha", "COHERENCE", HashMap::new()).unwrap();
        mem.store("b", "beta", "COHERENCE", HashMap::new()).unwrap();
        mem.store("c", "alpha", "COHERENCE", HashMap::new()).unwrap();
        let alphas = mem.query_by_kind("alpha", 10).unwrap();
        assert_eq!(alphas.len(), 2);
        let betas = mem.query_by_kind("beta", 10).unwrap();
        assert_eq!(betas.len(), 1);
    }

    #[test]
    fn test_search_text() {
        let (mem, _dir) = temp_memory();
        mem.store("the quick brown fox", "doc", "COHERENCE", HashMap::new()).unwrap();
        mem.store("jumps over the lazy dog", "doc", "COHERENCE", HashMap::new()).unwrap();
        let fox = mem.search("fox", 10).unwrap();
        assert_eq!(fox.len(), 1);
        let the = mem.search("the", 10).unwrap();
        assert_eq!(the.len(), 2);
    }

    #[test]
    fn test_count() {
        let (mem, _dir) = temp_memory();
        assert_eq!(mem.count().unwrap(), 0);
        mem.store("one", "t", "PLAN", HashMap::new()).unwrap();
        assert_eq!(mem.count().unwrap(), 1);
        mem.store("two", "t", "PLAN", HashMap::new()).unwrap();
        assert_eq!(mem.count().unwrap(), 2);
    }

    #[test]
    fn test_delete_before() {
        let (mem, _dir) = temp_memory();
        mem.store("keep", "t", "PLAN", HashMap::new()).unwrap();
        let deleted = mem.delete_before(0).unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(mem.count().unwrap(), 1);
    }

    #[test]
    fn test_query_empty() {
        let (mem, _dir) = temp_memory();
        let results = mem.query(10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_empty() {
        let (mem, _dir) = temp_memory();
        let results = mem.search("anything", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_memory_entry_serde_roundtrip() {
        let entry = MemoryEntry {
            id: "abc".into(),
            text: "test".into(),
            kind: "chat".into(),
            mode: "COHERENCE".into(),
            timestamp: 1000,
            extra: [("key".into(), "val".into())].into(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: MemoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, entry.id);
        assert_eq!(parsed.extra.get("key").unwrap(), "val");
    }
}
