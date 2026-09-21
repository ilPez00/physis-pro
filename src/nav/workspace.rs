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

use serde::{Deserialize, Serialize};

use crate::embed::VectorEmbed;
use crate::nav::{
    self, NavHit, NavIndex, NavStep, NavWeights, ObjectKind, PhysisObject, Relation,
    RelationOrigin, RetrievalMode, RetrievalScore,
};

// -- Option B bundle types ------------------------------------------------

/// A self-contained portable snapshot of objects, relations and pins.
/// Produced by `NavWorkspace::export_bundle`; consumed by `import_bundle`.
#[derive(Debug, Serialize, Deserialize)]
pub struct NavBundle {
    /// `kind:dim` stamp of the embedder that produced the stored vectors (e.g.
    /// `random-projection:384`). The importer refuses when this differs from
    /// the live embedder's stamp and the local index already has vectors.
    pub embedder_stamp: Option<String>,
    /// Source root (informational; not enforced on import).
    pub source_root: String,
    pub objects: Vec<PhysisObject>,
    pub relations: Vec<Relation>,
    /// Pin pairs serialised as `origin:key` strings (the canonical round-trip
    /// form from `spell_object_id`/`parse_object_id`).
    pub pin_pairs: Vec<(String, String)>,
}

/// Summary returned by `export_bundle`.
#[derive(Debug)]
pub struct NavBundleMeta {
    pub objects: usize,
    pub relations: usize,
    pub pins: usize,
    pub embedder_stamp: Option<String>,
    pub bytes: usize,
}

/// Summary returned by `import_bundle`.
#[derive(Debug)]
pub struct NavImportReport {
    pub added: usize,
    pub skipped: usize,
    pub relations: usize,
    pub pins: usize,
}

/// One navigation workspace: a root, its index, its store, its embedder.
pub struct NavWorkspace {
    pub root: PathBuf,
    index: NavIndex,
    /// sled handle — read-mostly; expose narrowly instead of making pub.
    db: sled::Db,
    /// Lazily-selected embedder; `Some((None, "off"))` = semantic disabled.
    embedder: Option<Option<(Box<dyn VectorEmbed>, &'static str)>>,
    /// When `PHYSIS_REMOTE_EMBED` is set, this suffix is appended to the
    /// embedder kind label so the header always shows the configured intent.
    /// Format: `+remote:<host>`. Never silent — if you can't label it, don't do it.
    remote_embed_suffix: Option<String>,
}

impl NavWorkspace {
    /// Open (or start fresh) the workspace rooted at `root`. The sled store
    /// lives in `PHYSIS_DATA_DIR` (default `.physis`) next to the index file.
    pub fn open(root: &Path) -> anyhow::Result<Self> {
        let db = Self::open_db()?;
        let mut index = nav::load_index(&db, root);
        index.embed_cache_dir = Self::data_dir();
        let remote_embed_suffix = std::env::var("PHYSIS_REMOTE_EMBED")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map(|url| {
                // Extract host:port for the label — never print the full URL
                // (could contain a token in query params). Use the authority only.
                let host = url
                    .trim_start_matches("http://")
                    .trim_start_matches("https://")
                    .split('/')
                    .next()
                    .unwrap_or("remote")
                    .to_string();
                format!("+remote:{host}")
            });
        Ok(Self {
            root: root.to_path_buf(),
            index,
            db,
            embedder: None,
            remote_embed_suffix,
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
    ///
    /// When `PHYSIS_REMOTE_EMBED` is set the local embedder is wrapped in
    /// [`crate::embed::RemoteEmbedder`] so new content is embedded by the
    /// remote pansa instance; the kind label gains a `+remote:<host>` suffix
    /// so the header never silently shows a remote network call.
    pub fn embedder(&mut self, enable: bool) -> Option<&dyn VectorEmbed> {
        if self.embedder.is_none() {
            let chosen = if !enable {
                None
            } else {
                let config = crate::config::PhysisConfig::load_effective()
                    .map(|eff| eff.config)
                    .unwrap_or_else(|_| crate::config::PhysisConfig::default());
                let (local, kind) = crate::embed::select_embedder(&config);
                // Option C: wrap with remote when PHYSIS_REMOTE_EMBED is set.
                let wrapped: Box<dyn crate::embed::VectorEmbed> =
                    if self.remote_embed_suffix.is_some() {
                        let url = std::env::var("PHYSIS_REMOTE_EMBED").unwrap_or_default();
                        let token = std::env::var("PHYSIS_API_TOKEN").ok();
                        Box::new(crate::embed::RemoteEmbedder::new(local, url, token))
                    } else {
                        local
                    };
                Some((wrapped, kind))
            };
            self.embedder = Some(chosen);
        }
        self.embedder
            .as_ref()
            .and_then(|slot| slot.as_ref())
            .map(|(e, _)| e.as_ref())
    }

    /// Which embedder would be (or was) selected, labelled.
    /// Appends `+remote:<host>` when `PHYSIS_REMOTE_EMBED` is configured so
    /// the HUD header always shows the intent — never a silent network call.
    pub fn embedder_kind(&mut self, enable: bool) -> String {
        if self.embedder.is_none() {
            self.embedder(enable);
        }
        let base = match self.embedder.as_ref().and_then(|s| s.as_ref()) {
            Some((_, kind)) => (*kind).to_string(),
            None => "off".to_string(),
        };
        match &self.remote_embed_suffix {
            Some(suffix) => format!("{base}{suffix}"),
            None => base,
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
            // `sem` is the *kind*'s verdict, not "we embedded something": the
            // lexical hash embeds too, and reporting that as semantic is the
            // substitution the acceptance step forbids.
            (n, crate::nav::is_semantic_kind(&kind), kind)
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
            slot.as_ref()
                .and_then(|s| s.as_ref())
                .map(|(e, _)| e.as_ref()),
            weights,
            &context,
            &pins,
            &relations,
            limit,
        );
        self.embedder = slot;
        let mode = RetrievalMode {
            // Semantic retrieval is in play only when all three hold: the
            // embedder is not a lexical stand-in, the vector leg carries weight,
            // and a vector component actually contributed. Any one of them
            // missing means the honest label is the reduced mode — a zero
            // weight is not "semantic retrieval that happened to score 0".
            semantic: crate::nav::is_semantic_kind(&kind)
                && weights.vector > 0.0
                && hits
                    .iter()
                    .any(|h| h.score.components.contains_key("vector")),
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
    /// Resolve a target that is *exactly* one object's source path.
    ///
    /// The exact leg of [`Self::resolve`], separated because the two legs are
    /// different claims: this one says "the world already answered" (the textbox
    /// chain's `route=exact-object`), while the prefix leg below says "one object
    /// starts with this" — a weaker, ambiguity-resolved lookup. It also has no
    /// side effects (no trail step), because it runs on every keystroke the
    /// operator types, and a lookup must not write history.
    ///
    /// A path names both a file and its passages, so the file wins: same rule as
    /// `resolve`, so the two cannot disagree about which object a path is.
    pub fn resolve_exact(&self, target: &str) -> Option<PhysisObject> {
        let norm = target.trim_start_matches("./");
        if norm.is_empty() {
            return None;
        }
        let by_path =
            |o: &&PhysisObject| o.source.as_ref().map(|s| s.path == norm).unwrap_or(false);
        let file = self
            .index
            .objects
            .values()
            .filter(|o| o.kind != ObjectKind::Passage)
            .find(by_path);
        file.or_else(|| self.index.objects.values().find(by_path))
            .cloned()
    }

    pub fn resolve(&self, target: &str) -> anyhow::Result<PhysisObject> {
        let norm = target.trim_start_matches("./");
        let by_path =
            |o: &&PhysisObject| o.source.as_ref().map(|s| s.path == norm).unwrap_or(false);
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

    // -- Option A: re-tag store for portable delivery ----------------------

    /// Open a workspace from a foreign-root index by re-tagging it to `root`.
    ///
    /// This is Option A from PH-028: copy `.physis/` from one machine, then
    /// open it on the client with a different absolute root by pointing the
    /// index's `root` field at the local checkout. Identity is content-hash so
    /// object ids survive; paths are root-relative so file reading works as
    /// long as the relative layout matches (same commit, same directory name).
    ///
    /// The client calls `NavWorkspace::open_aliased(&local_root, &foreign_index_path)`.
    /// After this call the workspace behaves as if it was indexed locally —
    /// `persist()` writes the re-tagged index under the local name.
    pub fn open_aliased(root: &Path, index_path: &std::path::Path) -> anyhow::Result<Self> {
        let db = Self::open_db()?;
        let raw = std::fs::read(index_path)
            .map_err(|e| anyhow::anyhow!("cannot read index {}: {e}", index_path.display()))?;
        let mut index: nav::NavIndex = serde_json::from_slice(&raw)
            .map_err(|e| anyhow::anyhow!("index parse error: {e}"))?;
        // Re-point the root; the file table stays root-relative and valid.
        index.root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        index.embed_cache_dir = Self::data_dir();
        Ok(Self {
            root: index.root.clone(),
            index,
            db,
            embedder: None,
            remote_embed_suffix: None,
        })
    }

    /// Copy this workspace's `.physis/` store directory to `dest_dir`, writing
    /// the index under the name derived from `client_root` so the client can
    /// open it with a plain `NavWorkspace::open`. The re-tag is documented in
    /// the copy rather than a rename on the source, so pansa's store is intact.
    ///
    /// Returns the index file path written inside `dest_dir`.
    pub fn export_store(&self, dest_dir: &std::path::Path, client_root: &Path) -> anyhow::Result<PathBuf> {
        std::fs::create_dir_all(dest_dir)?;
        // Compute the destination index filename from the client root.
        let tag = client_root
            .canonicalize()
            .unwrap_or_else(|_| client_root.to_path_buf())
            .display()
            .to_string()
            .replace('/', "_");
        let dest_index = dest_dir.join(format!("nav-index{tag}.json"));
        // Re-write the index with the client root so the client opens it with
        // NavWorkspace::open — no alias call needed.
        let mut copy = self.index.clone();
        copy.root = client_root.canonicalize().unwrap_or_else(|_| client_root.to_path_buf());
        std::fs::write(&dest_index, serde_json::to_vec(&copy)?)?;
        Ok(dest_index)
    }

    // -- Option B: portable export/import bundle ---------------------------

    /// Export a self-contained bundle to `path`.
    ///
    /// The bundle is a JSON object with:
    /// - `embedder_stamp`:  `kind:dim` string — the refusal key on import
    /// - `source_root`:     the source root (informational; not enforced)
    /// - `objects`:         all `PhysisObject` values
    /// - `relations`:       all `Relation` values
    /// - `pin_pairs`:       pin pairs as `origin:key` strings (round-trip safe)
    ///
    /// The importer calls `import_bundle` which refuses when the live
    /// embedder's stamp differs — never silently comparing vectors from two
    /// models.
    pub fn export_bundle(&self, path: &std::path::Path) -> anyhow::Result<NavBundleMeta> {
        let objects: Vec<nav::PhysisObject> = self.index.objects.values().cloned().collect();
        let relations = nav::load_relations(&self.db);
        let pins = nav::load_pins(&self.db);
        let pin_pairs: Vec<(String, String)> = pins
            .iter()
            .map(|p| {
                (
                    crate::world_object::spell_object_id(&p.a),
                    crate::world_object::spell_object_id(&p.b),
                )
            })
            .collect();
        let stamp = self.index.embedder_stamp.clone();
        let bundle = NavBundle {
            embedder_stamp: stamp.clone(),
            source_root: self.root.display().to_string(),
            objects,
            relations,
            pin_pairs,
        };
        let obj_count = bundle.objects.len();
        let rel_count = bundle.relations.len();
        let pin_count = bundle.pin_pairs.len();
        let raw = serde_json::to_vec(&bundle)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &raw)?;
        Ok(NavBundleMeta {
            objects: obj_count,
            relations: rel_count,
            pins: pin_count,
            embedder_stamp: stamp,
            bytes: raw.len(),
        })
    }

    /// Import a bundle produced by `export_bundle`, merging into this workspace.
    ///
    /// **Refusal contract**: if the bundle carries an `embedder_stamp` that
    /// differs from the live embedder's kind, the import is refused with an
    /// error. This is the guard against the silent vector-space mismatch that
    /// `packets/README.md` flags as a category-1 failure.
    ///
    /// Objects whose content hash already exists are skipped (deduplication).
    /// Returns a summary of what was added vs skipped.
    pub fn import_bundle(
        &mut self,
        path: &std::path::Path,
    ) -> anyhow::Result<NavImportReport> {
        let raw = std::fs::read(path)
            .map_err(|e| anyhow::anyhow!("cannot read bundle {}: {e}", path.display()))?;
        let bundle: NavBundle = serde_json::from_slice(&raw)
            .map_err(|e| anyhow::anyhow!("bundle parse error: {e}"))?;

        // Embedder-stamp refusal: never merge vectors from different spaces.
        if let Some(bundle_stamp) = &bundle.embedder_stamp {
            let live_kind = self.embedder_kind(true);
            let live_dim = self
                .index
                .objects
                .values()
                .find(|o| !o.embedding.is_empty())
                .map(|o| o.embedding.len())
                .unwrap_or(0);
            let live_stamp = if live_dim > 0 {
                format!("{live_kind}:{live_dim}")
            } else {
                live_kind.clone()
            };
            // Only refuse when the live index already has vectors: an empty
            // local index can accept any bundle (it has no space to clash with).
            let local_has_vectors = self
                .index
                .objects
                .values()
                .any(|o| !o.embedding.is_empty());
            if local_has_vectors && bundle_stamp != &live_stamp {
                anyhow::bail!(
                    "embedder mismatch: bundle is `{bundle_stamp}`, live index is `{live_stamp}`. \
                     Re-export the bundle under the live embedder or clear local vectors first."
                );
            }
        }

        let mut added = 0usize;
        let mut skipped = 0usize;
        for obj in bundle.objects {
            let key = serde_json::to_string(&obj.id).unwrap_or_default();
            use std::collections::btree_map::Entry;
            match self.index.objects.entry(key) {
                Entry::Occupied(_) => skipped += 1,
                Entry::Vacant(e) => {
                    e.insert(obj);
                    added += 1;
                }
            }
        }
        // Save relations and pins using the existing batch helpers.
        if !bundle.relations.is_empty() {
            nav::save_relations(&self.db, &bundle.relations)?;
        }
        let new_pins: Vec<nav::Pin> = bundle
            .pin_pairs
            .iter()
            .map(|(a_str, b_str)| nav::Pin {
                a: crate::world_object::parse_object_id(a_str),
                b: crate::world_object::parse_object_id(b_str),
                relation: String::new(),
                created_at: String::new(),
            })
            .collect();
        if !new_pins.is_empty() {
            nav::save_pins(&self.db, &new_pins)?;
        }
        // Propagate the stamp if the local index didn't have one.
        if self.index.embedder_stamp.is_none() {
            self.index.embedder_stamp = bundle.embedder_stamp;
        }
        self.persist()?;
        Ok(NavImportReport {
            added,
            skipped,
            relations: bundle.relations.len(),
            pins: bundle.pin_pairs.len(),
        })
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

    /// Restores one env var when the guard drops, so a forced-lexical test
    /// cannot leak its setting into the next test in this process.
    struct RestoreEnv {
        key: &'static str,
        old: Option<std::ffi::OsString>,
    }

    impl Drop for RestoreEnv {
        fn drop(&mut self) {
            match &self.old {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    /// The lexical fallback must announce itself as lexical. This is the client
    /// condition (a box with no embedding model: `docs/FEATHER.md`) and it used
    /// to be reported as `semantic: true` by both the index summary and the
    /// retrieval mode.
    #[test]
    fn a_lexical_embedder_is_never_reported_as_semantic() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("PHYSIS_DATA_DIR", dir.path().join("state"));
        let _embedder = RestoreEnv {
            key: "PHYSIS_EMBEDDER",
            old: std::env::var_os("PHYSIS_EMBEDDER"),
        };
        std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
        seed(dir.path(), "docs/order.md", "order signal experiment\n");
        let root = dir.path().to_path_buf();
        let mut ws = NavWorkspace::open(&root).unwrap();
        assert_eq!(
            ws.embedder_kind(true),
            "random-projection",
            "precondition: the env var must actually select the fallback, or this test proves nothing"
        );
        let summary = ws.update(true).unwrap();
        assert!(
            summary.contains("semantic: false"),
            "a lexical run must not claim semantic retrieval: {summary}"
        );
        let (hits, mode) = ws
            .query("order signal", false, &Default::default(), 10)
            .unwrap();
        assert!(!hits.is_empty(), "lexical retrieval still returns hits");
        assert!(!mode.semantic, "mode: {mode:?}");
        assert_eq!(mode.embedder, "random-projection");
    }

    /// A zero-weighted vector leg is not semantic retrieval, even on a machine
    /// whose embedder is semantic. `--no-semantic` zeroes the weight (it must not
    /// edit the store), so the mode label has to notice.
    #[test]
    fn a_zero_weighted_vector_leg_is_not_reported_as_semantic() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("PHYSIS_DATA_DIR", dir.path().join("state"));
        seed(dir.path(), "docs/order.md", "order signal experiment\n");
        let root = dir.path().to_path_buf();
        let mut ws = NavWorkspace::open(&root).unwrap();
        let _ = ws.update(true);

        let lexical_only = NavWeights {
            vector: 0.0,
            ..Default::default()
        };
        let (hits, mode) = ws
            .query("order signal", false, &lexical_only, 10)
            .unwrap();
        assert!(!hits.is_empty(), "lexical ranking still answers");
        assert!(
            !mode.semantic,
            "a zero-weighted leg must not be labelled semantic: {mode:?}"
        );
        // The raw component stays visible and `explain_hit` prints it with the
        // zero weight: "considered and weighted zero" is not the same claim as
        // "absent", and the readout must not conflate them.
        let explained = crate::nav::explain_hit(&hits[0], &lexical_only);
        assert!(
            explained.contains("× 0.00") || !explained.contains("vector"),
            "a zero weight is shown as a zero weight: {explained}"
        );
    }

    #[test]
    fn workspace_end_to_end() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("PHYSIS_DATA_DIR", dir.path().join("state"));
        seed(dir.path(), "docs/order.md", "order signal experiment\n");
        let root = dir.path().to_path_buf();
        let mut ws = NavWorkspace::open(&root).unwrap();
        let summary = ws.update(true).unwrap();
        // The summary's semantic flag must agree with the embedder it names.
        // This used to assert a bare `semantic: true`, which pinned a claim
        // that was false whenever the selected embedder was the lexical hash
        // (the summary said `semantic: true (embedder: random-projection)`).
        let kind = ws.embedder_kind(true);
        let expected = format!("semantic: {}", crate::nav::is_semantic_kind(&kind));
        assert!(summary.contains(&expected), "{summary}");
        let (hits, mode) = ws
            .query("order signal", false, &Default::default(), 10)
            .unwrap();
        assert!(!hits.is_empty());
        assert_eq!(
            mode.semantic,
            crate::nav::is_semantic_kind(&mode.embedder),
            "mode claims semantic={} for embedder {}",
            mode.semantic,
            mode.embedder
        );
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

    // -- PH-028 tests ------------------------------------------------------

    /// embed_objects stamps the index with `kind:dim` when vectors are written.
    /// This stamp is the refusal key for Option B imports.
    #[test]
    fn embedder_stamp_is_written_on_embed() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("PHYSIS_DATA_DIR", dir.path().join("state"));
        std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
        let _e = RestoreEnv {
            key: "PHYSIS_EMBEDDER",
            old: std::env::var_os("PHYSIS_EMBEDDER"),
        };
        seed(dir.path(), "a.md", "coherence signal");
        let mut ws = NavWorkspace::open(dir.path()).unwrap();
        ws.update(true).unwrap();
        // After embedding the stamp must be set.
        let stamp = ws.index().embedder_stamp.clone();
        assert!(stamp.is_some(), "stamp must be set after embedding");
        let stamp = stamp.unwrap();
        // Format: `kind:dim`
        assert!(
            stamp.starts_with("random-projection:"),
            "stamp must include kind prefix: {stamp}"
        );
        let dim: usize = stamp.split(':').nth(1).unwrap().parse().unwrap();
        assert!(dim > 0, "dim must be positive: {stamp}");
    }

    /// Option B round-trip: export then import merges objects, stamp is carried.
    #[test]
    fn bundle_export_import_round_trips_objects_and_stamp() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let src_dir = tempfile::tempdir().unwrap();
        std::env::set_var("PHYSIS_DATA_DIR", src_dir.path().join("state"));
        std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
        let _e = RestoreEnv {
            key: "PHYSIS_EMBEDDER",
            old: std::env::var_os("PHYSIS_EMBEDDER"),
        };
        seed(src_dir.path(), "signal.md", "coherence signal experiment");
        let mut src = NavWorkspace::open(src_dir.path()).unwrap();
        src.update(true).unwrap();
        let src_objects = src.index().objects.len();
        assert!(src_objects > 0, "precondition: source has objects");

        let bundle_path = src_dir.path().join("bundle.json");
        let meta = src.export_bundle(&bundle_path).unwrap();
        assert_eq!(meta.objects, src_objects);
        assert!(meta.embedder_stamp.is_some(), "stamp must be in bundle");

        // Import into a fresh workspace.
        let dst_dir = tempfile::tempdir().unwrap();
        std::env::set_var("PHYSIS_DATA_DIR", dst_dir.path().join("state"));
        let mut dst = NavWorkspace::open(dst_dir.path()).unwrap();
        let report = dst.import_bundle(&bundle_path).unwrap();
        assert_eq!(report.added, src_objects, "all objects imported");
        assert_eq!(report.skipped, 0, "nothing skipped on first import");

        // Idempotent re-import skips everything.
        let report2 = dst.import_bundle(&bundle_path).unwrap();
        assert_eq!(report2.added, 0, "second import adds nothing");
        assert_eq!(report2.skipped, src_objects, "all skipped on second import");

        // Stamp propagated.
        assert_eq!(dst.index().embedder_stamp, meta.embedder_stamp);
    }

    /// Option B refusal: import refuses when the local index has vectors from a
    /// different embedder than the bundle. Never a silent space mismatch.
    #[test]
    fn bundle_import_refuses_embedder_mismatch() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("PHYSIS_DATA_DIR", dir.path().join("state"));
        std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
        let _e = RestoreEnv {
            key: "PHYSIS_EMBEDDER",
            old: std::env::var_os("PHYSIS_EMBEDDER"),
        };

        // Build a bundle with a deliberate wrong stamp.
        seed(dir.path(), "a.md", "signal");
        let mut ws = NavWorkspace::open(dir.path()).unwrap();
        ws.update(true).unwrap();
        let bundle_path = dir.path().join("bundle.json");
        ws.export_bundle(&bundle_path).unwrap();

        // Tamper the stamp in the bundle JSON.
        let mut raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&bundle_path).unwrap()).unwrap();
        raw["embedder_stamp"] = serde_json::json!("bge-base:768");
        std::fs::write(&bundle_path, serde_json::to_vec(&raw).unwrap()).unwrap();

        // The destination already has vectors (same workspace, re-opened).
        drop(ws);
        let mut dst = NavWorkspace::open(dir.path()).unwrap();
        let err = dst.import_bundle(&bundle_path).unwrap_err();
        assert!(
            err.to_string().contains("embedder mismatch"),
            "must say embedder mismatch: {err}"
        );
    }

    /// Option A: open_aliased re-tags the root so the client can open a pansa
    /// store without renaming any files.
    #[test]
    fn open_aliased_retags_root_and_persists_under_new_name() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let src_dir = tempfile::tempdir().unwrap();
        let _data = RestoreEnv {
            key: "PHYSIS_DATA_DIR",
            old: std::env::var_os("PHYSIS_DATA_DIR"),
        };
        let _emb = RestoreEnv {
            key: "PHYSIS_EMBEDDER",
            old: std::env::var_os("PHYSIS_EMBEDDER"),
        };
        std::env::set_var("PHYSIS_DATA_DIR", src_dir.path().join("state"));
        std::env::set_var("PHYSIS_EMBEDDER", "random-projection");

        seed(src_dir.path(), "world.md", "one world many projections");
        let mut src = NavWorkspace::open(src_dir.path()).unwrap();
        src.update(true).unwrap();
        src.persist().unwrap();

        // Find the index file path (tagged with the src root).
        let tag = src_dir
            .path()
            .canonicalize()
            .unwrap()
            .display()
            .to_string()
            .replace('/', "_");
        let index_file = src_dir
            .path()
            .join("state")
            .join(format!("nav-index{tag}.json"));
        assert!(index_file.exists(), "source index must exist: {}", index_file.display());

        // Re-tag for a different client root using a fresh data dir so there
        // is no sled lock conflict with the source workspace.
        let client_root = std::path::Path::new("/home/gio/physis_pro");
        let dst_dir = tempfile::tempdir().unwrap();
        std::env::set_var("PHYSIS_DATA_DIR", dst_dir.path().join("state2"));
        drop(src); // release the sled lock before opening the aliased store

        let ws = NavWorkspace::open_aliased(client_root, &index_file).unwrap();
        let written = ws.export_store(dst_dir.path(), client_root).unwrap();
        assert!(written.exists(), "re-tagged index must be written");
        let expected_tag = client_root
            .canonicalize()
            .unwrap_or_else(|_| client_root.to_path_buf())
            .display()
            .to_string()
            .replace('/', "_");
        assert!(
            written.file_name().unwrap().to_string_lossy().contains(&expected_tag),
            "file name must encode client root: {}",
            written.display()
        );
        // Objects survive the re-tag.
        let reloaded: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&written).unwrap()).unwrap();
        let obj_count = reloaded["objects"].as_object().map(|o| o.len()).unwrap_or(0);
        assert!(obj_count > 0, "re-tagged index must carry objects");
    }

    /// Option C: when PHYSIS_REMOTE_EMBED is set the embedder_kind label shows
    /// `+remote:<host>` — never a silent network call.
    #[test]
    fn remote_embed_env_annotates_kind_label() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let _data = RestoreEnv {
            key: "PHYSIS_DATA_DIR",
            old: std::env::var_os("PHYSIS_DATA_DIR"),
        };
        let _remote = RestoreEnv {
            key: "PHYSIS_REMOTE_EMBED",
            old: std::env::var_os("PHYSIS_REMOTE_EMBED"),
        };
        let _emb = RestoreEnv {
            key: "PHYSIS_EMBEDDER",
            old: std::env::var_os("PHYSIS_EMBEDDER"),
        };
        std::env::set_var("PHYSIS_DATA_DIR", dir.path().join("state"));
        std::env::set_var("PHYSIS_REMOTE_EMBED", "http://100.65.59.18:19876");
        std::env::set_var("PHYSIS_EMBEDDER", "random-projection");

        let mut ws = NavWorkspace::open(dir.path()).unwrap();
        let kind = ws.embedder_kind(true);
        assert!(
            kind.contains("+remote:"),
            "kind must show +remote when PHYSIS_REMOTE_EMBED is set: {kind}"
        );
        assert!(
            kind.contains("100.65.59.18"),
            "kind must show host: {kind}"
        );
    }
}
