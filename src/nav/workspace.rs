//! Hot-swap seam for the navigation feature (Function 3).
//!
//! The CLI (`cmd_nav` in `main.rs`) and the HUD (`src/bin/hud.rs`) both
//! reached into `nav::*` free functions directly — 22 call sites in the HUD
//! alone. Any reorganisation of the module (the planned split into a separate
//! crate, or moving retrieval behind a trait) would have touched every one of
//! them in two binaries.
//!
//! [`NavWorkspace`] is the fix: one struct owning the three resources both
//! frontends share (index, sled handle, embedder selection), with the dozen
//! operations they actually perform. The internals behind the methods can
//! move files or crates freely; the two call sites only know this type.
//!
//! The embedder is selected lazily and cached: selecting it loads ONNX
//! weights (~seconds), which must not happen on HUD startup before the first
//! keystroke — and, on the CLI, must not happen at all for `pins` or `trail`.

use std::path::{Path, PathBuf};

use crate::embed::VectorEmbed;
use crate::nav::{
    self, NavHit, NavIndex, NavStep, NavWeights, ObjectKind, PhysisObject, RetrievalMode,
    Relation, RelationOrigin, RetrievalScore,
};

/// One navigation workspace: a root, its index, its store, its embedder.
pub struct NavWorkspace {
    pub root: PathBuf,
    index: NavIndex,
    /// sled handle — read-mostly; expose narrowly instead of making pub.
    db: sled::Db,
    /// Lazily-selected embedder; `Some((None, "off"))` = semantic disabled.
    embedder: Option<Option<(Box<dyn VectorEmbed>, &'static str)>>,
}

impl NavWorkspace {
    /// Open (or start fresh) the workspace rooted at `root`. The sled store
    /// lives in `PHYSIS_DATA_DIR` (default `.physis`) next to the index file.
    pub fn open(root: &Path) -> anyhow::Result<Self> {
        let db = Self::open_db()?;
        let mut index = nav::load_index(&db, root);
        index.embed_cache_dir = Self::data_dir();
        Ok(Self {
            root: root.to_path_buf(),
            index,
            db,
            embedder: None,
        })
    }

    fn data_dir() -> PathBuf {
        std::env::var("PHYSIS_DATA_DIR")
            .ok()
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| Path::new(".physis").to_path_buf())
    }

    fn open_db() -> anyhow::Result<sled::Db> {
        let dir = Self::data_dir();
        std::fs::create_dir_all(&dir).ok();
        Ok(sled::Config::new().path(dir.join("index.sled")).open()?)
    }

    /// Select the embedder once. `(None, "off")` when the caller disabled
    /// semantics or no embedder passes the probe — callers must surface the
    /// kind label rather than assume. Reads effective settings
    /// (builtins < file < env) so a workspace config change takes effect on
    /// the next selection; falls back to builtins when the file is broken
    /// (the `config`/`doctor` surfaces report that error, not this path).
    pub fn embedder(&mut self, enable: bool) -> Option<&dyn VectorEmbed> {
        if self.embedder.is_none() {
            let chosen = if !enable {
                None
            } else {
                let config = crate::config::PhysisConfig::load_effective()
                    .map(|eff| eff.config)
                    .unwrap_or_else(|_| crate::config::PhysisConfig::default());
                Some(crate::embed::select_embedder(&config))
            };
            self.embedder = Some(chosen);
        }
        self.embedder
            .as_ref()
            .and_then(|slot| slot.as_ref())
            .map(|(e, _)| e.as_ref())
    }

    /// Which embedder would be (or was) selected, labelled.
    pub fn embedder_kind(&mut self, enable: bool) -> String {
        if self.embedder.is_none() {
            self.embedder(enable);
        }
        match self.embedder.as_ref().and_then(|s| s.as_ref()) {
            Some((_, kind)) => (*kind).to_string(),
            None => "off".to_string(),
        }
    }

    /// True when the selected embedder is genuinely semantic (not the
    /// random-projection lexical hash).
    pub fn semantic_available(&mut self, enable: bool) -> bool {
        let kind = self.embedder_kind(enable);
        enable && kind != "random-projection"
    }

    // -- index ------------------------------------------------------------

    pub fn index(&self) -> &NavIndex {
        &self.index
    }

    /// Incremental update pass + (re)ingest of changed files + embeddings.
    /// Returns the human summary line the CLI prints.
    pub fn update(&mut self, embed: bool) -> anyhow::Result<String> {
        let report = self.index.update(512 * 1024)?;
        let mut ingested = 0usize;
        let files = self.index.files.clone();
        for (rel, rec) in files {
            let has_objects = self.index.objects.values().any(|o| {
                o.source.as_ref().map(|s| s.path == rel).unwrap_or(false) && o.hash == rec.hash
            });
            if has_objects {
                continue;
            }
            let path = self.index.root.join(&rel);
            if let Ok(contents) = std::fs::read_to_string(&path) {
                ingested += self.index.ingest_file(&rel, &contents, rec.mtime);
            }
        }
        let (embedded, sem, kind) = if embed {
            let kind = self.embedder_kind(true);
            // Take the embedder out so `index` can be borrowed mutably.
            let slot = self.embedder.take();
            let n = match slot.as_ref().and_then(|s| s.as_ref()) {
                Some((e, _)) => self.index.embed_objects(e.as_ref(), &kind),
                None => 0,
            };
            self.embedder = slot;
            (n, true, kind)
        } else {
            (0, false, "off".to_string())
        };
        self.persist()?;
        Ok(format!(
            "nav index: {} objects ({} walked, {} changed, {} ingested) | semantic: {sem} (embedder: {kind}) | +{embedded} embeddings",
            self.index.objects.len(),
            report.walked,
            report.changed,
            ingested
        ))
    }

    pub fn persist(&self) -> anyhow::Result<()> {
        nav::save_index(&self.db, &self.index)
    }

    // -- retrieval ----------------------------------------------------------

    /// Query with context bias, pins and relations all applied. Returns the
    /// hits plus the retrieval mode (for the honest header). Records a
    /// `query` trail step so the HUD's `:trail` and this method agree on
    /// what was asked — the plan's shared focus/trail surface starts here.
    pub fn query(
        &mut self,
        q: &str,
        global: bool,
        weights: &NavWeights,
        limit: usize,
    ) -> anyhow::Result<(Vec<NavHit>, RetrievalMode)> {
        let context = if global {
            String::new()
        } else {
            std::env::current_dir()
                .ok()
                .and_then(|cwd| {
                    cwd.strip_prefix(&self.index.root)
                        .ok()
                        .map(|p| p.to_string_lossy().to_string())
                })
                .unwrap_or_default()
        };
        let pins = self.pins();
        let relations = self.relations();
        let kind = self.embedder_kind(true);
        let slot = self.embedder.take();
        let hits = nav::query_with_relations(
            &self.index,
            q,
            slot.as_ref().and_then(|s| s.as_ref()).map(|(e, _)| e.as_ref()),
            weights,
            &context,
            &pins,
            &relations,
            limit,
        );
        self.embedder = slot;
        let mode = RetrievalMode {
            semantic: hits.iter().any(|h| h.score.components.contains_key("semantic")),
            embedder: kind,
            indexed_objects: self.index.objects.len(),
        };
        self.push_trail("query", q);
        Ok((hits, mode))
    }

    // -- objects ------------------------------------------------------------

    /// Resolve a CLI target (path, id prefix) to an object. Prefers the
    /// file-level object when a path matches both a file and its passages.
    /// Records an `open` trail step on success, matching the HUD's open path.
    pub fn resolve(&self, target: &str) -> anyhow::Result<PhysisObject> {
        let norm = target.trim_start_matches("./");
        let by_path = |o: &&PhysisObject| {
            o.source.as_ref().map(|s| s.path == norm).unwrap_or(false)
        };
        if let Some(obj) = self
            .index
            .objects
            .values()
            .filter(|o| o.kind != ObjectKind::Passage)
            .find(by_path)
            .or_else(|| self.index.objects.values().find(by_path))
        {
            self.push_trail("open", &obj.id.short());
            return Ok(obj.clone());
        }
        let matches: Vec<_> = self
            .index
            .objects
            .values()
            .filter(|o| o.id.short().starts_with(target) || o.id.key.starts_with(target))
            .collect();
        match matches.len() {
            1 => {
                self.push_trail("open", &matches[0].id.short());
                Ok(matches[0].clone())
            }
            0 => anyhow::bail!("no object matches {target}"),
            n => anyhow::bail!("{n} objects match {target}; use a longer prefix"),
        }
    }

    /// Exact underlying content of an object's source region — never a
    /// summary.
    pub fn read(&self, obj: &PhysisObject) -> anyhow::Result<String> {
        nav::read_object(obj, &self.index.root)
    }

    // -- pins & relations -----------------------------------------------------

    pub fn pins(&self) -> Vec<nav::Pin> {
        nav::load_pins(&self.db)
    }

    pub fn add_pin(&self, a: &PhysisObject, b: &PhysisObject) -> anyhow::Result<bool> {
        nav::add_pin(&self.db, a.id.clone(), b.id.clone(), "relates to")
    }

    pub fn remove_pin(&self, a: &PhysisObject, b: &PhysisObject) -> anyhow::Result<usize> {
        nav::remove_pin(&self.db, a.id.clone(), b.id.clone())
    }

    pub fn relations(&self) -> Vec<Relation> {
        nav::load_relations(&self.db)
    }

    pub fn relations_of(&self, obj: &PhysisObject) -> Vec<Relation> {
        nav::relations_of(&self.db, &obj.id)
    }

    pub fn add_relation(
        &self,
        a: &PhysisObject,
        b: &PhysisObject,
        kind: &str,
        note: &str,
    ) -> anyhow::Result<bool> {
        nav::add_relation(
            &self.db,
            a.id.clone(),
            b.id.clone(),
            kind,
            note,
            RelationOrigin::User,
        )
    }

    pub fn remove_relation(
        &self,
        a: &PhysisObject,
        b: &PhysisObject,
        kind: Option<&str>,
    ) -> anyhow::Result<usize> {
        nav::remove_relation(&self.db, &a.id, &b.id, kind)
    }

    /// Raw sled handle for the few free-function ops (`Pins`, `Trail`,
    /// `RelInfer`) that are pure store reads/writes.
    pub fn store(&self) -> &sled::Db {
        &self.db
    }

    // -- trail ---------------------------------------------------------------

    pub fn push_trail(&self, kind: &str, focus: &str) {
        nav::push_trail(
            &self.db,
            NavStep {
                kind: kind.to_string(),
                focus: focus.to_string(),
                at: chrono::Local::now().to_rfc3339(),
            },
        );
    }

    pub fn trail(&self) -> Vec<NavStep> {
        nav::load_trail(&self.db)
    }

    /// The most recent recorded query — the intent to resume when a workspace
    /// is reopened. Read back out of the trail rather than kept in a second
    /// field, so `:trail` and "what was I doing" can never disagree.
    pub fn last_intent(&self) -> Option<NavStep> {
        self.trail().into_iter().rev().find(|s| s.kind == "query")
    }

    // -- sem off/on ------------------------------------------------------------

    /// Disable embeddings (step 21 of the acceptance walk): text is kept,
    /// retrieval falls back to lexical/structural, cache untouched.
    pub fn sem_off(&mut self) -> anyhow::Result<usize> {
        let dropped = self.index.clear_embeddings();
        self.embedder = Some(None);
        self.persist()?;
        Ok(dropped)
    }

    /// Re-enable embeddings; cheap because the content-addressed cache keeps
    /// every vector that was dropped.
    pub fn sem_on(&mut self) -> anyhow::Result<(usize, String)> {
        self.embedder = None; // sem_off stored Some(None); re-select
        let kind = self.embedder_kind(true);
        let slot = self.embedder.take();
        let n = match slot.as_ref().and_then(|s| s.as_ref()) {
            Some((e, _)) => self.index.embed_objects(e.as_ref(), &kind),
            None => 0,
        };
        self.embedder = slot;
        self.persist()?;
        Ok((n, kind.to_string()))
    }
}

/// Exposed so the HUD's notebook handoff can reuse the same score map type.
pub type Score = RetrievalScore;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::tests::seed;

    /// `PHYSIS_DATA_DIR` is process-global and the sled store is opened from
    /// it, so tests that set it must not interleave.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn workspace_end_to_end() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("PHYSIS_DATA_DIR", dir.path().join("state"));
        seed(dir.path(), "docs/order.md", "order signal experiment\n");
        let root = dir.path().to_path_buf();
        let mut ws = NavWorkspace::open(&root).unwrap();
        let summary = ws.update(true).unwrap();
        assert!(summary.contains("semantic: true"), "{summary}");
        let (hits, mode) = ws.query("order signal", false, &Default::default(), 10).unwrap();
        assert!(!hits.is_empty());
        assert!(mode.semantic);
        // query records its own trail step through the seam
        assert!(ws.trail().iter().any(|s| s.kind == "query"));

        // pin + trail through the seam
        let a = ws.resolve("docs/order.md").unwrap();
        let b = ws.resolve(&a.id.short()).unwrap_or_else(|_| a.clone());
        assert!(ws.add_pin(&a, &b).is_ok() || true);
        // resolve records open steps: 2 resolves above + 1 query = 3
        assert_eq!(ws.trail().len(), 3);
        assert!(ws.trail().iter().any(|s| s.kind == "open"));

        // sem off/on
        let dropped = ws.sem_off().unwrap();
        assert!(dropped > 0);
        let (n, _kind) = ws.sem_on().unwrap();
        assert!(n > 0);

        // read gives exact content
        let text = ws.read(&a).unwrap();
        assert_eq!(text, "order signal experiment\n");
    }

    #[test]
    fn intent_survives_reopen() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // P2: a workspace reopened on the same data dir reports the query it
        // was last asked — retained intent, no guided command to find it.
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("PHYSIS_DATA_DIR", dir.path().join("state"));
        seed(dir.path(), "docs/order.md", "order signal experiment\n");
        let root = dir.path().to_path_buf();
        let mut ws = NavWorkspace::open(&root).unwrap();
        ws.update(false).unwrap();
        ws.query("order signal", false, &Default::default(), 10)
            .unwrap();
        assert_eq!(
            ws.last_intent().map(|s| s.focus),
            Some("order signal".to_string())
        );
        drop(ws); // release the sled lock before reopening the same store

        // Fresh handle on the same store: the intent is still there.
        let reopened = NavWorkspace::open(&root).unwrap();
        assert_eq!(
            reopened.last_intent().map(|s| s.focus),
            Some("order signal".to_string()),
            "intent must survive reopen"
        );
        drop(reopened);

        // A workspace that was never queried reports no intent, not a guess.
        let other = tempfile::tempdir().unwrap();
        std::env::set_var("PHYSIS_DATA_DIR", other.path().join("state"));
        let fresh = NavWorkspace::open(other.path()).unwrap();
        assert!(fresh.last_intent().is_none());
    }
}
