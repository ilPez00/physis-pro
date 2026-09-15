//! Durable persistence for adaptive documents (Phase 26 §26.6–§26.7).
//!
//! Layout under the store root:
//! - `document.json` — full [`Document`] state (raw bytes, blocks, links)
//! - `events.jsonl`  — append-only change log; store sequence is replay order
//! - `patches.json`  — proposals/applied/rejected/reverted patches by id
//!
//! Raw document bytes live inside `document.json` for this MVP; if a future
//! relocation splits them out, a missing blob must be reported, never
//! silently rebuilt (AD11).

use super::{Change, Document, Patch};
use crate::atomic::write_json_durable;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Patches {
    #[serde(default)]
    pub patches: Vec<Patch>,
}

pub struct Store {
    pub root: PathBuf,
}

impl Store {
    /// Open (not create) a store rooted at `root`; nothing touches disk
    /// until the first write.
    pub fn open(root: &Path) -> Store {
        Store { root: root.to_path_buf() }
    }

    pub fn load_document(&self) -> io::Result<Option<Document>> {
        let path = self.root.join("document.json");
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)?;
        serde_json::from_slice(&bytes).map(Some).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("document.json: {e}"))
        })
    }

    /// Load the patch bookkeeping; a missing file is an empty book.
    pub fn load_patches(&self) -> io::Result<Patches> {
        let path = self.root.join("patches.json");
        if !path.exists() {
            return Ok(Patches::default());
        }
        let bytes = std::fs::read(&path)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("patches.json: {e}")))
    }

    /// Load the append-only event log. Events replay in file order, which is
    /// store sequence; out-of-order timestamps do not reorder history (AD16).
    pub fn load_events(&self) -> io::Result<Vec<Change>> {
        let path = self.root.join("events.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let bytes = std::fs::read(&path)?;
        let mut events = Vec::new();
        for line in bytes.split(|&b| b == b'\n') {
            if line.is_empty() {
                continue;
            }
            events.push(serde_json::from_slice(line).map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidData, format!("events.jsonl: {e}"))
            })?);
        }
        Ok(events)
    }

    /// Persist document state durably, so an interrupted save never
    /// truncates existing state (Phase 26 §26.6).
    pub fn save_document(&self, doc: &Document) -> io::Result<()> {
        write_json_durable(&self.root.join("document.json"), doc)
    }

    pub fn save_patches(&self, patches: &Patches) -> io::Result<()> {
        write_json_durable(&self.root.join("patches.json"), patches)
    }

    /// Append one event line. Opens in append mode; a torn trailing line is
    /// reported by [`Self::load_events`] as `InvalidData`, never dropped.
    pub fn append_event(&self, change: &Change) -> io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join("events.jsonl"))?;
        let line = serde_json::to_string(change)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        f.write_all(line.as_bytes())?;
        f.write_all(b"\n")?;
        f.sync_all()
    }

    /// Idempotency lookup: a repeated patch id returns the recorded patch so
    /// a network retry is not a second application (AD08).
    pub fn patch_by_id<'a>(&self, patches: &'a Patches, id: &str) -> Option<&'a Patch> {
        patches.patches.iter().find(|p| p.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_store_loads_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(dir.path());
        assert!(s.load_document().unwrap().is_none());
        assert!(s.load_events().unwrap().is_empty());
        assert!(s.load_patches().unwrap().patches.is_empty());
    }

    #[test]
    fn document_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(dir.path());
        let doc = super::super::parse_blocks("d", 1, "# T\n\nBody.\n");
        s.save_document(&doc).unwrap();
        let back = s.load_document().unwrap().unwrap();
        assert_eq!(back.raw, "# T\n\nBody.\n");
        assert_eq!(back.revision, 1);
    }

    #[test]
    fn events_append_in_file_order() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(dir.path());
        for seq in 0..3u64 {
            s.append_event(&Change {
                event_id: format!("e{seq}"),
                document_id: "d".into(),
                block_id: "b".into(),
                before_revision: seq,
                after_revision: seq + 1,
                before_hash: 0,
                after_hash: 1,
                actor: "test".into(),
                operation: "test".into(),
                sequence: seq,
                recorded_at: 100 - seq, // deliberately out of order
            })
            .unwrap();
        }
        let events = s.load_events().unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[2].event_id, "e2");
        assert_eq!(events[2].recorded_at, 98);
    }

    #[test]
    fn patches_round_trip_and_lookup_by_id() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(dir.path());
        let book = Patches {
            patches: vec![Patch {
                id: "p1".into(),
                document_id: "d".into(),
                triggered_by_change_id: "e".into(),
                expected_document_revision: 1,
                expected_graph_revision: 0,
                replacements: vec![],
                review_only: vec![],
                status: super::super::PatchStatus::Proposed,
                applied_revision: None,
            }],
        };
        s.save_patches(&book).unwrap();
        let back = s.load_patches().unwrap();
        assert!(s.patch_by_id(&back, "p1").is_some());
        assert!(s.patch_by_id(&back, "nope").is_none());
    }
}
