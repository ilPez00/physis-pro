//! Editable intent (plan S7).
//!
//! The intent is the operator's own statement of what this workspace is for. It
//! is data, not a prompt: a Note-kind object with a provenance-stamped edit
//! history, persisted so it survives a restart. Every edit records who changed
//! it, when, from what, to what, and why — so `why` can show the intent's own
//! lineage and a packet can name the intent it was built under.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The object kind an intent is filed under, in the registry vocabulary.
pub const INTENT_KIND: &str = "note";

/// One provenance-stamped edit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentEdit {
    pub at: chrono::DateTime<chrono::Utc>,
    pub by: String,
    /// The statement before the edit (empty on creation).
    #[serde(default)]
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub reason: String,
}

/// The operator's intent: a statement plus how it got that way.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentObject {
    pub id: String,
    pub statement: String,
    /// The goal this intent serves, when it is subordinate to one.
    #[serde(default)]
    pub goal_id: Option<String>,
    #[serde(default)]
    pub history: Vec<IntentEdit>,
}

impl IntentObject {
    pub fn new(id: impl Into<String>, statement: impl Into<String>, by: impl Into<String>) -> Self {
        let statement = statement.into();
        Self {
            id: id.into(),
            statement: statement.clone(),
            goal_id: None,
            history: vec![IntentEdit {
                at: chrono::Utc::now(),
                by: by.into(),
                from: String::new(),
                to: statement,
                reason: "created".to_string(),
            }],
        }
    }

    /// The most recent edit, if any.
    pub fn last_edit(&self) -> Option<&IntentEdit> {
        self.history.last()
    }

    /// Apply an edit, appending a provenance entry. An edit that changes
    /// nothing is still recorded: "someone looked and left it" is provenance.
    pub fn edit(
        &mut self,
        to: impl Into<String>,
        by: impl Into<String>,
        reason: impl Into<String>,
    ) {
        let to = to.into();
        self.history.push(IntentEdit {
            at: chrono::Utc::now(),
            by: by.into(),
            from: self.statement.clone(),
            to: to.clone(),
            reason: reason.into(),
        });
        self.statement = to;
    }

    pub fn render(&self) -> String {
        let mut out = format!("{}\n  {}\n", self.id, self.statement);
        if let Some(g) = &self.goal_id {
            out.push_str(&format!("  goal: {g}\n"));
        }
        out.push_str(&format!("  edits: {}\n", self.history.len()));
        for e in self.history.iter().rev().take(5) {
            out.push_str(&format!(
                "    {} by {} — {}{}\n",
                e.at.to_rfc3339(),
                e.by,
                e.to,
                if e.reason.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", e.reason)
                }
            ));
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentStore {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub intents: BTreeMap<String, IntentObject>,
}

fn default_version() -> u32 {
    1
}

impl Default for IntentStore {
    fn default() -> Self {
        Self {
            version: 1,
            intents: BTreeMap::new(),
        }
    }
}

impl IntentStore {
    /// The store for a workspace root: `.physis/intents.json`.
    pub fn path(root: &Path) -> PathBuf {
        root.join(".physis").join("intents.json")
    }

    /// Load, or start empty when absent. A malformed file is reported and
    /// treated as empty rather than taking the command down with it.
    pub fn load(root: &Path) -> Self {
        let path = Self::path(root);
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<IntentStore>(&text) {
                Ok(mut s) => {
                    if s.version != 1 {
                        eprintln!(
                            "physis: intents.json version {} unsupported; starting empty",
                            s.version
                        );
                        return Self::default();
                    }
                    s.version = 1;
                    s
                }
                Err(e) => {
                    eprintln!("physis: intents.json unreadable ({e}); starting empty");
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, root: &Path) -> Result<(), String> {
        let path = Self::path(root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn get(&self, id: &str) -> Option<&IntentObject> {
        self.intents.get(id)
    }

    /// Create or edit the intent, stamping the change. Returns the new object.
    pub fn set(&mut self, id: &str, statement: &str, by: &str, reason: &str) -> &IntentObject {
        let entry = self
            .intents
            .entry(id.to_string())
            .or_insert_with(|| IntentObject::new(id, "", by));
        if entry.statement.is_empty() && entry.history.len() == 1 {
            // Creation placeholder: make the first edit the real statement.
            entry.history.clear();
        }
        entry.edit(statement, by, reason);
        entry
    }

    pub fn render(&self) -> String {
        if self.intents.is_empty() {
            return "(no intents)\n".to_string();
        }
        self.intents.values().map(|i| i.render()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_are_provenance_stamped_and_survive_a_round_trip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let mut s = IntentStore::load(root);
        s.set("main", "ship the impossible computer", "opencode", "start");
        s.set(
            "main",
            "ship the impossible computer tonight",
            "gio",
            "deadline",
        );
        s.save(root).unwrap();

        let reloaded = IntentStore::load(root);
        let i = reloaded.get("main").unwrap();
        assert_eq!(i.statement, "ship the impossible computer tonight");
        assert_eq!(i.history.len(), 2);
        let last = i.last_edit().unwrap();
        assert_eq!(last.by, "gio");
        assert_eq!(last.from, "ship the impossible computer");
        assert_eq!(last.reason, "deadline");
    }

    #[test]
    fn an_edit_that_changes_nothing_is_still_recorded() {
        let mut s = IntentStore::default();
        s.set("x", "keep", "a", "create");
        s.set("x", "keep", "b", "reviewed, unchanged");
        let i = s.get("x").unwrap();
        assert_eq!(i.history.len(), 2, "a look is provenance too");
        assert_eq!(i.statement, "keep");
    }

    #[test]
    fn a_malformed_store_is_reported_not_fatal() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".physis")).unwrap();
        std::fs::write(IntentStore::path(tmp.path()), "{ not json").unwrap();
        let s = IntentStore::load(tmp.path());
        assert!(s.intents.is_empty());
    }

    #[test]
    fn render_names_the_statement_and_recent_edits() {
        let mut s = IntentStore::default();
        s.set("main", "do the thing", "opencode", "start");
        let text = s.render();
        assert!(text.contains("main"));
        assert!(text.contains("do the thing"));
        assert!(text.contains("opencode"));
    }
}
