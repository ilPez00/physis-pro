/// Scoped context + telemetry + cheap intent + snapshot helpers.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Scoped context handed to an agent: pack result + honest telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSlice {
    pub intent: String,
    pub context: String,
    pub chunks: Vec<crate::pack::PackedChunk>,
    pub objects_considered: usize,
    pub objects_selected: usize,
    pub candidate_tokens: usize,
    pub final_tokens: usize,
    pub budget: usize,
    pub provider: String,
    pub latency_ms: u128,
}

impl WorldSlice {
    pub fn telemetry(&self) -> String {
        format!(
            "repository objects considered {}\nobjects selected {}\ncandidate/raw context tokens {}\nfinal context tokens {}\nmodel/provider used {}\nagent calls 0\nlatency {} ms",
            self.objects_considered, self.objects_selected,
            self.candidate_tokens, self.final_tokens,
            self.provider, self.latency_ms,
        )
    }
}

/// Snapshot of file hashes for pre/post comparison.
pub type HashSnapshot = BTreeMap<String, String>;

pub fn snapshot_hashes(files: &[crate::scanner::FileInfo]) -> HashSnapshot {
    files
        .iter()
        .map(|f| (f.path.clone(), f.hash.clone()))
        .collect()
}

/// Diff two hash snapshots into changed/added/removed path lists.
pub fn diff_snapshots(
    before: &HashSnapshot,
    after: &HashSnapshot,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut changed = Vec::new();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    for (p, h) in after {
        match before.get(p) {
            None => added.push(p.clone()),
            Some(old) if old != h => changed.push(p.clone()),
            _ => {}
        }
    }
    for p in before.keys() {
        if !after.contains_key(p) {
            removed.push(p.clone());
        }
    }
    changed.sort();
    added.sort();
    removed.sort();
    (changed, added, removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::FileInfo;

    fn fi(path: &str, hash: &str) -> FileInfo {
        FileInfo {
            path: path.to_string(),
            abs_path: path.to_string(),
            ext: ".rs".to_string(),
            size: 1,
            mtime: 0.0,
            hash: hash.to_string(),
            structural_lines: vec![],
        }
    }

    #[test]
    fn snapshot_diff_separates_changed_added_and_removed() {
        let before = snapshot_hashes(&[fi("a.rs", "h1"), fi("b.rs", "h2")]);
        let after = snapshot_hashes(&[fi("a.rs", "h1x"), fi("c.rs", "h3")]);
        let (changed, added, removed) = diff_snapshots(&before, &after);
        assert_eq!(changed, vec!["a.rs"]);
        assert_eq!(added, vec!["c.rs"]);
        assert_eq!(removed, vec!["b.rs"]);
    }

    #[test]
    fn identical_snapshots_produce_an_empty_delta() {
        let s = snapshot_hashes(&[fi("a.rs", "h1")]);
        let (changed, added, removed) = diff_snapshots(&s, &s);
        assert!(changed.is_empty() && added.is_empty() && removed.is_empty());
    }

    #[test]
    fn telemetry_reports_every_required_counter() {
        let s = WorldSlice {
            intent: "q".into(),
            context: String::new(),
            chunks: vec![],
            objects_considered: 100,
            objects_selected: 5,
            candidate_tokens: 9000,
            final_tokens: 1800,
            budget: 2000,
            provider: "test".into(),
            latency_ms: 12,
        };
        let t = s.telemetry();
        for needle in [
            "objects considered",
            "objects selected",
            "candidate/raw context tokens",
            "final context tokens",
            "model/provider used",
            "agent calls",
            "latency",
        ] {
            assert!(t.contains(needle), "missing {needle} in:\n{t}");
        }
    }
}
