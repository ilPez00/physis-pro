//! Semantic Navigation (MVP Function 3): meaning-first navigation over
//! addressable objects — files, passages, concepts — with hybrid retrieval,
//! per-component score explanation, pins, navigation history, and honest
//! degradation when no semantic embedder is available.
//!
//! ## What this reuses (and does not rebuild)
//!
//! - **Embedding** — [`crate::embed::select_embedder`] / `physis_core::embed::select`
//!   cascade (ONNX MiniLM, self-test gated, falling back to a random-projection
//!   lexical hash that is *labelled as such*).
//! - **Chunking** — the 40-line window shape of [`crate::pack`] with its
//!   provenance (`path:start-end`).
//! - **Embedding cache** — [`crate::pack::EmbedCache`], content-addressed by
//!   `(embedder_kind, dim, sha256)`; an unchanged chunk costs a hash.
//! - **Cosine** — [`physis_core::models::cosine_sim`], one implementation.
//! - **Persistence** — one sled tree (`nav` namespace) in the engine's
//!   `index.sled` store, same store the disk indexer uses. Incremental: only
//!   entries whose `(size, mtime)` moved are re-read, re-chunked and re-embedded.
//!
//! No LLM anywhere. Nothing in this module performs generation; results are the
//! user's own objects, grouped and scored.

pub mod workspace;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::embed::VectorEmbed;

// ---------------------------------------------------------------------------
// Object model
// ---------------------------------------------------------------------------

/// Every navigable thing. Kept small deliberately (MVP); "Device", "Dataset"
/// etc. can join without breaking anything because objects are addressed by
/// `ObjectId`, not by kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    File,
    Passage,
    Directory,
    Concept,
    Note,
    Other,
}

impl ObjectKind {
    pub fn label(&self) -> &'static str {
        match self {
            ObjectKind::File => "File",
            ObjectKind::Passage => "Passage",
            ObjectKind::Directory => "Directory",
            ObjectKind::Concept => "Concept",
            ObjectKind::Note => "Note",
            ObjectKind::Other => "Other",
        }
    }
}

/// Stable identity. The path is *one* representation; the content hash and the
/// origin (which substrate produced the object) carry identity across renames.
/// Deliberately not `ObjectId = path` — the path field is rewritten when an
/// object is seen under a new name with the same content hash, keeping its
/// history and pins attached.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ObjectId {
    /// Substrate that produced the object: "fs" (filesystem), "concept"
    /// (ontology theme), … One namespace per substrate.
    pub origin: String,
    /// Identity within the substrate. For files: the content hash (hex), which
    /// survives a rename. For concepts: the theme id.
    pub key: String,
}

impl ObjectId {
    pub fn new(origin: &str, key: &str) -> Self {
        Self {
            origin: origin.to_string(),
            key: key.to_string(),
        }
    }
    /// Human-stable short form for display and CLI addressing.
    pub fn short(&self) -> String {
        format!("{}:{}", self.origin, &self.key[..self.key.len().min(12)])
    }
}

/// Where an object lives when it has a filesystem presence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRef {
    pub path: String,
    /// 1-indexed inclusive, when the object is a region of the file.
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
}

/// One addressable object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysisObject {
    pub id: ObjectId,
    pub kind: ObjectKind,
    pub label: String,
    pub source: Option<SourceRef>,
    /// mtime of the source file, seconds since epoch — the `O` context signal.
    pub mtime: f64,
    /// Content hash of the chunk text (not the whole file): the cache key.
    pub hash: String,
    /// Chunk text (passages) or the label/name (files, concepts).
    pub text: String,
    /// Embedding, persisted with the object (serde-defaulted so older blobs
    /// load). Empty when the object predates the embedder or embeddings are
    /// disabled — retrieval then reports the reduced mode honestly.
    #[serde(default)]
    pub embedding: Vec<f32>,
}

// ---------------------------------------------------------------------------
// Score
// ---------------------------------------------------------------------------/// Generic score map: every ranking component is inspectable by name. Unknown
/// future components render the same way — the HUD never special-cases one.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetrievalScore {
    pub total: f32,
    pub components: BTreeMap<String, f32>,
}

impl RetrievalScore {
    fn add(&mut self, name: &str, weight: f32, raw: f32) {
        self.components.insert(name.to_string(), raw);
        self.total += weight * raw;
    }
}

// ---------------------------------------------------------------------------
// Index
// ---------------------------------------------------------------------------

/// Exclusion dirs — start from the scanner's list; nav adds its own state dir
/// so the index never indexes itself.
fn exclude_dirs() -> std::collections::HashSet<String> {
    crate::scanner::DEFAULT_EXCLUDE_DIRS
        .iter()
        .map(|d| d.to_string())
        .chain([
            ".physis".to_string(),
            ".cache".to_string(),
            "Trash".to_string(),
        ])
        .collect()
}

/// Configured root with a priority that scales the context component. MVP:
/// one root, priority 1.0, from the CLI; the struct exists so multi-root
/// config (§17) is additive rather than a redesign.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavRoot {
    pub path: PathBuf,
    pub priority: f32,
}

/// Persisted index: objects keyed by `ObjectId`, plus a map from source paths
/// to the objects that came from them (the invalidation unit for incremental
/// updates). Stored as one sled blob per root, like [`crate::indexer::FileIndex`].
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct NavIndex {
    pub root: PathBuf,
    /// stat fast-path catalog: path → (size, mtime, hash). Unchanged file =
    /// zero reads, zero embeds.
    pub files: BTreeMap<String, FileRecord>,
    pub objects: BTreeMap<String, PhysisObject>,
    pub total_walked: usize,
    /// Where the shared content-addressed embed cache lives (set at load).
    #[serde(skip)]
    pub embed_cache_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub size: u64,
    pub mtime: f64,
    pub hash: String,
}

impl NavIndex {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            embed_cache_dir: std::env::var("PHYSIS_DATA_DIR")
                .ok()
                .filter(|p| !p.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| Path::new(".physis").to_path_buf()),
            ..Default::default()
        }
    }

    /// One incremental pass. Returns counts so the HUD can show
    /// `indexed: N / M objects` honestly while indexing is partial.
    pub fn update(&mut self, max_file_bytes: u64) -> anyhow::Result<NavUpdateReport> {
        let started = std::time::Instant::now();
        let exclude = exclude_dirs();
        let mut report = NavUpdateReport::default();

        let mut walked = 0usize;
        let mut current: BTreeMap<String, FileRecord> = Default::default();

        for entry in WalkDir::new(&self.root)
            .into_iter()
            .filter_entry(|e| {
                if e.depth() == 0 {
                    return true;
                }
                !(e.file_type().is_dir()
                    && exclude.contains(e.file_name().to_string_lossy().as_ref()))
            })
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            walked += 1;
            let Ok(meta) = entry.metadata() else { continue };
            let Ok(rel) = entry.path().strip_prefix(&self.root) else {
                continue;
            };
            let rel = rel.to_string_lossy().to_string();
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);

            // Byte-size sanity + text probe before hashing.
            if meta.len() > max_file_bytes {
                continue;
            }
            if !crate::scanner::is_text_file(entry.path()) {
                continue;
            }

            match self.files.get(&rel) {
                Some(prev) if prev.size == meta.len() && (prev.mtime - mtime).abs() < 0.001 => {
                    current.insert(rel, prev.clone()); // stat fast path
                }
                _ => {
                    let Some(hash) = crate::indexer::compute_file_hash(entry.path()) else {
                        continue;
                    };
                    let changed = self.files.get(&rel).map(|p| p.hash != hash).unwrap_or(true);
                    if changed {
                        report.changed += 1;
                    }
                    current.insert(
                        rel,
                        FileRecord {
                            size: meta.len(),
                            mtime,
                            hash,
                        },
                    );
                }
            }
        }

        // Drop records for deleted files + their objects.
        let gone: Vec<String> = self
            .files
            .keys()
            .filter(|p| !current.contains_key(*p))
            .cloned()
            .collect();
        for p in &gone {
            self.objects
                .retain(|_, o| o.source.as_ref().map(|s| &s.path != p).unwrap_or(true));
            self.files.remove(p);
            report.deleted += 1;
        }
        self.files = current;
        self.total_walked = walked;
        report.walked = walked;
        report.elapsed_ms = started.elapsed().as_millis();
        Ok(report)
    }

    /// Ingest one file: chunk into 40-line windows, emit one File object plus
    /// Passage objects. Text-provided by the caller (which already read the
    /// file for the hash pass) — kept as a free function for testability.
    pub fn ingest_file(&mut self, rel: &str, contents: &str, mtime: f64) -> usize {
        let path = self.root.join(rel);
        let file_hash = crate::indexer::compute_file_hash(&path).unwrap_or_default();
        self.files.insert(
            rel.to_string(),
            FileRecord {
                size: contents.len() as u64,
                mtime,
                hash: file_hash.clone(),
            },
        );

        // Drop previous objects from this file (chunk boundaries may have moved).
        self.objects
            .retain(|_, o| o.source.as_ref().map(|s| s.path != rel).unwrap_or(true));

        let mut n = 0usize;
        let kind = if rel.ends_with(".md") || rel.ends_with(".txt") || rel.contains("note") {
            ObjectKind::Note
        } else {
            ObjectKind::File
        };
        let label = rel.rsplit('/').next().unwrap_or(rel).to_string();
        let file_id = ObjectId::new("fs", &file_hash);
        self.objects.insert(
            serde_json::to_string(&file_id).unwrap(),
            PhysisObject {
                id: file_id,
                kind,
                label: label.clone(),
                source: Some(SourceRef {
                    path: rel.to_string(),
                    start_line: None,
                    end_line: None,
                }),
                mtime,
                hash: file_hash.clone(),
                text: format!("{rel} {label}"),
                embedding: Vec::new(),
            },
        );
        n += 1;

        let lines: Vec<&str> = contents.lines().collect();
        for (idx, window) in lines.chunks(CHUNK_LINES).enumerate() {
            let text = window.join("\n");
            if text.trim().is_empty() {
                continue;
            }
            let start = idx * CHUNK_LINES + 1;
            let end = start + window.len() - 1;
            let chunk_hash = {
                use sha2::{Digest, Sha256};
                let mut h = Sha256::new();
                h.update(text.as_bytes());
                format!("{:x}", h.finalize())
            };
            let id = ObjectId::new("fs", &chunk_hash);
            self.objects.insert(
                serde_json::to_string(&id).unwrap(),
                PhysisObject {
                    id,
                    kind: ObjectKind::Passage,
                    label: format!("{rel}:{start}-{end}"),
                    source: Some(SourceRef {
                        path: rel.to_string(),
                        start_line: Some(start),
                        end_line: Some(end),
                    }),
                    mtime,
                    hash: chunk_hash,
                    text,
                    embedding: Vec::new(),
                },
            );
            n += 1;
        }
        n
    }

    /// Compute embeddings for every object that lacks one, via the shared
    /// content-addressed cache. Only changed chunks reach the model. `false`
    /// return means the embedder declined (semantic off) — the caller reports
    /// the reduced mode.
    pub fn embed_objects(&mut self, embedder: &dyn VectorEmbed, embedder_kind: &str) -> usize {
        let keys: Vec<String> = self.objects.keys().cloned().collect();
        let mut embedded = 0usize;
        // Batch per collect: gather texts missing embeddings first.
        let texts: Vec<String> = keys
            .iter()
            .filter_map(|k| {
                let o = &self.objects[k];
                (o.embedding.is_empty()).then(|| o.text.clone())
            })
            .collect();
        if texts.is_empty() {
            return 0;
        }
        // Resident cache: loaded once per process, saved back here.
        let vectors = crate::pack::EmbedCache::with_shared(&self.embed_cache_dir, |cache| {
            cache.embed_texts(&texts, embedder, embedder_kind)
        });
        let mut vi = 0usize;
        for k in &keys {
            let o = self.objects.get_mut(k).unwrap();
            if o.embedding.is_empty() {
                o.embedding = vectors[vi].clone();
                vi += 1;
                embedded += 1;
            }
        }
        embedded
    }

    /// Drop all embeddings (acceptance step 21: semantic disabled). The cache
    /// file keeps them, so re-enabling is cheap and nothing is destroyed.
    pub fn clear_embeddings(&mut self) -> usize {
        let mut dropped = 0usize;
        for o in self.objects.values_mut() {
            if !o.embedding.is_empty() {
                o.embedding.clear();
                dropped += 1;
            }
        }
        dropped
    }
}

/// Counts from one [`NavIndex::update`] pass.
#[derive(Debug, Default)]
pub struct NavUpdateReport {
    pub walked: usize,
    pub changed: usize,
    pub deleted: usize,
    pub elapsed_ms: u128,
}

/// Lines per chunk — same shape as [`crate::pack::CHUNK_LINES`] so pack and
/// navigation agree on what a passage is.
pub const CHUNK_LINES: usize = 40;

// ---------------------------------------------------------------------------
// Pins
// ---------------------------------------------------------------------------

/// An explicit user-asserted relation between two objects. First-class graph
/// edge: never folded into an embedding, always inspectable and reversible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pin {
    pub a: ObjectId,
    pub b: ObjectId,
    /// Free-text relation name ("relates to", "is", …). MVP treats all pins
    /// the same; the field keeps intent without needing a fixed vocabulary.
    pub relation: String,
    pub created_at: String,
}

/// Navigation trail: one state per query/selection step, reconstructable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavStep {
    pub kind: String, // "query" | "enter" | "back"
    /// Query text or object id.
    pub focus: String,
    pub at: String,
}

// ---------------------------------------------------------------------------
// Retrieval
// ---------------------------------------------------------------------------

/// Weights for the ranking components. Weights are visible in explanations
/// (each component is shown both raw and weighted); they are deliberately not
/// a hidden constant — the CLI exposes `--semantic/--lexical/--recency` to
/// move them.
#[derive(Debug, Clone)]
/// Ranked legs. `vector` is the leg computed over embeddings — named for the
/// computation, never for a guessed source: a lexical hash produces vectors too,
/// and [`is_semantic_kind`] is the one place that decides whether those vectors
/// mean meaning. Calling the leg "semantic" made every renderer that printed the
/// component claim more than the number carries.
pub struct NavWeights {
    pub vector: f32,
    pub lexical: f32,
    pub recency: f32,
    pub pin: f32,
    /// Weight of the current-directory context bias.
    pub context: f32,
}

impl Default for NavWeights {
    fn default() -> Self {
        Self {
            vector: 0.6,
            lexical: 0.3,
            recency: 0.08,
            pin: 0.25,
            context: 0.15,
        }
    }
}

/// One ranked result with its inspectable score map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavHit {
    pub object: PhysisObject,
    pub score: RetrievalScore,
}

/// Why a hit ranked — the human-readable half of the explanation.
pub fn explain_hit(hit: &NavHit, weights: &NavWeights) -> String {
    let mut parts = Vec::new();
    let c = &hit.score.components;
    for (name, raw) in c {
        let w = match name.as_str() {
            "vector" => weights.vector,
            "lexical" => weights.lexical,
            "recency" => weights.recency,
            "pin" => weights.pin,
            "context" => weights.context,
            _ => 0.0,
        };
        parts.push(format!("{name} {raw:.2} × {w:.2}"));
    }
    parts.join(" · ")
}

/// Token-set lexical overlap (Jaccard on lowercase word unigrams). This is the
/// honest "BM25-shaped" component the MVP can compute with zero extra deps and
/// full determinism; BM25 proper over the same tokens is the upgrade path and
/// the score slot (`lexical`) does not change.
fn lexical_score(query_terms: &[String], text: &str) -> f32 {
    let lower = text.to_lowercase();
    let mut hits = 0usize;
    for t in query_terms {
        if lower.contains(t.as_str()) {
            hits += 1;
        }
    }
    if query_terms.is_empty() {
        0.0
    } else {
        hits as f32 / query_terms.len() as f32
    }
}

/// Recency in `0..1`: full score at 0 days, half-life 30 days.
fn recency_score(now: f64, mtime: f64) -> f32 {
    let days = ((now - mtime).max(0.0) / 86400.0) as f32;
    0.5f32.powf(days / 30.0)
}

/// Query terms: lowercase word tokens, len ≥ 3, minus a tiny stopword set.
fn query_terms(query: &str) -> Vec<String> {
    const STOP: [&str; 8] = [
        "the", "and", "that", "with", "about", "where", "what", "when",
    ];
    query
        .split(|c: char| !c.is_alphanumeric())
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() >= 3 && !STOP.contains(&w.as_str()))
        .collect()
}

/// Is this embedder kind *semantic*, or a lexical stand-in wearing an
/// embedder's interface?
///
/// One predicate, because three places used to answer this question three
/// different ways and two of them answered it wrong: `update()` hardcoded
/// `semantic: true` whenever it embedded anything, `RetrievalMode::semantic`
/// asked whether any hit carried a `semantic` score *component* (which the
/// lexical hash also produces), and the HUD compared the label itself. The
/// result was a header reading `[semantic+lexical] embedder:
/// random-projection` — retrieval by string match, announced as meaning. It
/// bites hardest on the client whose only embedder *is* the fallback.
///
/// The kind is the truth; the component set is not.
pub fn is_semantic_kind(kind: &str) -> bool {
    !matches!(kind, "off" | "random-projection")
}

/// What retrieval was able to use. Printed in the HUD header — a reduced mode
/// must be reported, never silent (acceptance step 22).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalMode {
    pub semantic: bool,
    pub embedder: String,
    pub indexed_objects: usize,
}

/// Run a query over the index.
///
/// `context_path` biases ranking toward the file the user is currently inside
/// (its directory shares the context component). `""` = no context. `/global`
/// prefixes are stripped by the caller; context bias is a separate additive
/// component so it is visible per-hit.
pub fn query(
    index: &NavIndex,
    q: &str,
    embedder: Option<&dyn VectorEmbed>,
    weights: &NavWeights,
    context_path: &str,
    pins: &[Pin],
    limit: usize,
) -> Vec<NavHit> {
    let terms = query_terms(q);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    let q_emb = embedder.map(|e| e.embed(q));

    // Pin adjacency: object key → partners (precomputed, all-pairs view).
    let key_of = |id: &ObjectId| serde_json::to_string(id).unwrap();
    let mut pin_map: BTreeMap<String, Vec<ObjectId>> = Default::default();
    for p in pins {
        pin_map.entry(key_of(&p.a)).or_default().push(p.b.clone());
        pin_map.entry(key_of(&p.b)).or_default().push(p.a.clone());
    }

    // Semantic preselection: when an embedder exists, seed each object's
    // embedding from the cache-backed store. In the MVP the embeddings live
    // attached to the query call via `EmbedCache` at the CLI layer; here we
    // compute cosine against a per-call embedding map passed in.
    let mut hits: Vec<NavHit> = Vec::new();
    let context_dir = context_path.rsplit('/').next().unwrap_or("").to_string();

    for obj in index.objects.values() {
        let mut score = RetrievalScore::default();

        let lex = lexical_score(&terms, &obj.text);
        score.add("lexical", weights.lexical, lex);

        if let (Some(qe), emb) = (&q_emb, &obj.embedding) {
            if !emb.is_empty() {
                let cos = physis_core::models::cosine_sim(qe, emb).clamp(0.0, 1.0);
                score.add("vector", weights.vector, cos);
            }
        }

        score.add("recency", weights.recency, recency_score(now, obj.mtime));

        if !context_path.is_empty() {
            let in_ctx = obj
                .source
                .as_ref()
                .map(|s| {
                    s.path.starts_with(context_path)
                        || (!context_dir.is_empty()
                            && s.path.split('/').any(|seg| seg == context_dir))
                })
                .unwrap_or(false);
            score.add("context", weights.context, if in_ctx { 1.0 } else { 0.0 });
        }

        // Pin score: this object rides up when a pinned partner matches the
        // query (lexically or semantically). Explicit bridge only — no
        // embedding is ever mutated.
        if !terms.is_empty() {
            if let Some(partners) = pin_map.get(&key_of(&obj.id)) {
                let linked = partners.iter().any(|partner| {
                    let Some(po) = index.objects.get(&key_of(partner)) else {
                        return false;
                    };
                    let lex = lexical_score(&terms, &po.text) > 0.2;
                    let sem = match (&q_emb, &po.embedding) {
                        (Some(qe), pe) if !pe.is_empty() => {
                            physis_core::models::cosine_sim(qe, pe) > 0.4
                        }
                        _ => false,
                    };
                    lex || sem
                });
                if linked {
                    score.add("pin", weights.pin, 1.0);
                }
            }
        }

        if score.total <= 0.01 {
            continue;
        }
        hits.push(NavHit {
            object: obj.clone(),
            score,
        });
    }

    hits.sort_by(|a, b| {
        b.score
            .total
            .partial_cmp(&a.score.total)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits.truncate(limit);
    hits
}

// ---------------------------------------------------------------------------
// Grouping (§7: results are a neighborhood, not a flat list)
// ---------------------------------------------------------------------------

/// Group key for the HUD. Order is the display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Notes,
    Files,
    Passages,
    Concepts,
    Other,
}

impl Group {
    pub fn label(&self) -> &'static str {
        match self {
            Group::Notes => "Notes",
            Group::Files => "Code & files",
            Group::Passages => "Passages",
            Group::Concepts => "Concepts",
            Group::Other => "Other",
        }
    }
}

pub fn group_hits(hits: &[NavHit]) -> Vec<(Group, Vec<&NavHit>)> {
    let mut order = [
        Group::Notes,
        Group::Files,
        Group::Passages,
        Group::Concepts,
        Group::Other,
    ];
    let mut groups: BTreeMap<Group, Vec<&NavHit>> = Default::default();
    for h in hits {
        let g = match h.object.kind {
            ObjectKind::Note => Group::Notes,
            ObjectKind::File | ObjectKind::Directory => Group::Files,
            ObjectKind::Passage => Group::Passages,
            ObjectKind::Concept => Group::Concepts,
            ObjectKind::Other => Group::Other,
        };
        groups.entry(g).or_default().push(h);
    }
    order.sort_by_key(|g| std::cmp::Reverse(groups.get(g).map(|v| v.len()).unwrap_or(0)));
    let mut out = Vec::new();
    for g in order {
        if let Some(v) = groups.remove(&g) {
            out.push((g, v));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

/// sled namespace keys. Same `index.sled` store the disk indexer uses; one
/// blob per root, one global pin set, one global trail.
pub mod store_keys {
    pub const PINS: &str = "nav:pins";
    pub const TRAIL: &str = "nav:trail";
}

/// The index lives in its **own file**, not a sled tree. Measured: sled keeps
/// every insert of the ~22 MB index blob as a new blob file, so repeated
/// saves filled gigabytes of never-reclaimed blobs. A single JSON file with
/// an atomic rename has none of that, and the small hot records (pins,
/// trail, relations) stay in sled where churn is cheap.
fn index_path(root: &Path) -> PathBuf {
    let data_dir = std::env::var("PHYSIS_DATA_DIR")
        .ok()
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(".physis").to_path_buf());
    let tag = root
        .canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .display()
        .to_string()
        .replace('/', "_");
    data_dir.join(format!("nav-index{tag}.json"))
}

/// Load the index for `root` (empty when absent/corrupt — a corrupt index is
/// rebuilt, never fatal).
pub fn load_index(_db: &sled::Db, root: &Path) -> NavIndex {
    let mut idx = NavIndex::new(root);
    let path = index_path(root);
    if let Ok(raw) = std::fs::read(&path) {
        if let Ok(saved) = serde_json::from_slice::<NavIndex>(&raw) {
            idx = saved;
            idx.embed_cache_dir = NavIndex::new(root).embed_cache_dir;
        }
    }
    idx
}

pub fn save_index(_db: &sled::Db, idx: &NavIndex) -> anyhow::Result<()> {
    let path = index_path(&idx.root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Atomic write: temp file in the same directory, then rename (see
    // `pack::EmbedCache::save` for the torn-write rationale).
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(idx)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// All pins, global (they bridge across roots by design).
pub fn load_pins(db: &sled::Db) -> Vec<Pin> {
    db.get(store_keys::PINS)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or_default()
}

pub fn save_pins(db: &sled::Db, pins: &[Pin]) -> anyhow::Result<()> {
    db.insert(store_keys::PINS, serde_json::to_vec(pins)?)?;
    Ok(())
}

/// Add a pin; no-op when an identical pin already exists (idempotent by
/// content, so a double-press of `p` cannot litter).
pub fn add_pin(db: &sled::Db, a: ObjectId, b: ObjectId, relation: &str) -> anyhow::Result<bool> {
    let mut pins = load_pins(db);
    let dup = pins
        .iter()
        .any(|p| (p.a == a && p.b == b) || (p.a == b && p.b == a));
    if dup {
        return Ok(false);
    }
    pins.push(Pin {
        a,
        b,
        relation: relation.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    });
    save_pins(db, &pins)?;
    Ok(true)
}

/// Remove a pin (reversible, §13).
pub fn remove_pin(db: &sled::Db, a: ObjectId, b: ObjectId) -> anyhow::Result<usize> {
    let mut pins = load_pins(db);
    let before = pins.len();
    pins.retain(|p| !((p.a == a && p.b == b) || (p.a == b && p.b == a)));
    let removed = before - pins.len();
    if removed > 0 {
        save_pins(db, &pins)?;
    }
    Ok(removed)
}

/// The navigation trail (§12): append one step, cap at 256 entries.
pub fn push_trail(db: &sled::Db, step: NavStep) {
    let mut trail: Vec<NavStep> = db
        .get(store_keys::TRAIL)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or_default();
    trail.push(step);
    if trail.len() > 256 {
        let drop = trail.len() - 256;
        trail.drain(0..drop);
    }
    if let Ok(bytes) = serde_json::to_vec(&trail) {
        let _ = db.insert(store_keys::TRAIL, bytes);
    }
}

pub fn load_trail(db: &sled::Db) -> Vec<NavStep> {
    db.get(store_keys::TRAIL)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or_default()
}

/// Read the exact underlying content of an object's source region (§10). No
/// summary, no generation: bytes from disk, sliced by the recorded lines.
pub fn read_object(obj: &PhysisObject, root: &Path) -> anyhow::Result<String> {
    let Some(src) = &obj.source else {
        return Ok(obj.text.clone());
    };
    let contents = std::fs::read_to_string(root.join(&src.path))?;
    match (src.start_line, src.end_line) {
        (Some(s), Some(e)) => {
            let sel: Vec<&str> = contents.lines().skip(s - 1).take(e - s + 1).collect();
            Ok(sel.join("\n"))
        }
        _ => Ok(contents),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::RandomProjectionEmbedder;

    pub(super) fn seed(root: &Path, rel: &str, content: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    pub(super) fn build_index(root: &Path) -> NavIndex {
        let mut idx = NavIndex::new(root);
        idx.update(512 * 1024).unwrap();
        // Ingest every file the update walk found — tests seed arbitrary
        // trees and must not maintain this list in sync with their fixtures.
        let rels: Vec<String> = idx.files.keys().cloned().collect();
        for rel in rels {
            if rel.starts_with(".physis") {
                continue; // engine state, not content
            }
            let path = root.join(&rel);
            if !path.exists() || !crate::scanner::is_text_file(&path) {
                continue;
            }
            let (Ok(contents), Ok(meta)) =
                (std::fs::read_to_string(&path), std::fs::metadata(&path))
            else {
                continue;
            };
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            idx.ingest_file(&rel, &contents, mtime);
        }
        idx
    }

    #[test]
    fn ingests_file_and_passage_objects() {
        let dir = tempfile::tempdir().unwrap();
        seed(
            dir.path(),
            "docs/order.md",
            "# Order experiment\n\nPosition alone beat the shuffled null.\n",
        );
        let idx = build_index(dir.path());
        // 1 file object + 1 passage (3 lines fit one window).
        assert_eq!(
            idx.objects.len(),
            2,
            "{:?}",
            idx.objects.keys().collect::<Vec<_>>()
        );
        let kinds: Vec<ObjectKind> = idx.objects.values().map(|o| o.kind).collect();
        assert!(kinds.contains(&ObjectKind::Note));
        assert!(kinds.contains(&ObjectKind::Passage));
    }

    /// Semantic query finds the passage whose words share *no* token with the
    /// query — the property lexical-only retrieval cannot deliver.
    #[test]
    fn semantic_query_finds_lexically_disjoint_passage() {
        let dir = tempfile::tempdir().unwrap();
        seed(
            dir.path(),
            "docs/order.md",
            "Position alone beat the shuffled null. The order signal held on every corpus.\n",
        );
        seed(
            dir.path(),
            "src/engine.rs",
            "fn main() { println!(\"invoice payment\"); }\n",
        );
        let mut idx = build_index(dir.path());
        let e = RandomProjectionEmbedder::new(64);
        // Random projection is a lexical hash; use a query with overlapping
        // vocabulary to keep this test about plumbing, not model quality.
        idx.embed_objects(&e, "rp-test");
        let hits = query(
            &idx,
            "order signal",
            Some(&e),
            &NavWeights::default(),
            "",
            &[],
            10,
        );
        assert!(!hits.is_empty());
        let best = &hits[0];
        assert!(best.score.components.contains_key("lexical"));
        assert!(best.score.components.contains_key("vector"));
        assert!(best.score.total > 0.0);
    }

    /// Degraded mode: no embedder ⇒ no semantic component, lexical still
    /// returns the right object. Honest reduction, not failure.
    #[test]
    fn lexical_only_query_works_without_embedder() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), "docs/order.md", "the order signal held\n");
        let idx = build_index(dir.path());
        let hits = query(
            &idx,
            "order signal",
            None,
            &NavWeights::default(),
            "",
            &[],
            10,
        );
        assert!(!hits.is_empty(), "lexical path must stand alone");
        assert!(hits
            .iter()
            .all(|h| !h.score.components.contains_key("vector")));
    }

    /// Pins drag the pinned partner up when only the partner matches.
    #[test]
    fn pin_bridge_surfaces_unrelated_object() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), "docs/order.md", "order signal experiment\n");
        seed(
            dir.path(),
            "src/unrelated.rs",
            "totally different words here\n",
        );
        let mut idx = build_index(dir.path());

        let e = RandomProjectionEmbedder::new(64);
        idx.embed_objects(&e, "rp-test");

        // Pin unrelated.rs to the order doc. Query matches only the order doc.
        let order_id = idx
            .objects
            .values()
            .find(|o| o.kind == ObjectKind::Note)
            .unwrap()
            .id
            .clone();
        let other_id = idx
            .objects
            .values()
            .find(|o| {
                o.source
                    .as_ref()
                    .map(|s| s.path == "src/unrelated.rs")
                    .unwrap_or(false)
            })
            .unwrap()
            .id
            .clone();
        let pin = Pin {
            a: order_id,
            b: other_id,
            relation: "relates to".into(),
            created_at: "t".into(),
        };
        let hits = query(
            &idx,
            "order signal",
            Some(&e),
            &NavWeights::default(),
            "",
            std::slice::from_ref(&pin),
            10,
        );
        let unrelated = hits
            .iter()
            .find(|h| {
                h.object
                    .source
                    .as_ref()
                    .map(|s| s.path == "src/unrelated.rs")
                    .unwrap_or(false)
            })
            .expect("pinned partner must appear");
        assert!(
            unrelated
                .score
                .components
                .get("pin")
                .copied()
                .unwrap_or(0.0)
                >= 1.0
        );
    }

    #[test]
    fn incremental_update_skips_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), "a.md", "hello world\n");
        let mut idx = build_index(dir.path());
        let before = idx.objects.len();

        // Touch nothing: update must read zero files.
        let r = idx.update(512 * 1024).unwrap();
        assert_eq!(r.changed, 0);
        assert_eq!(idx.objects.len(), before);

        // Change the file: objects refresh.
        std::thread::sleep(std::time::Duration::from_millis(20));
        seed(dir.path(), "a.md", "changed content entirely\n");
        let r_mid = idx.update(512 * 1024).unwrap();
        assert_eq!(r_mid.changed, 1);
        assert_eq!(r.changed, 0, "earlier report untouched");
        let r2 = idx.update(512 * 1024).unwrap();
        assert_eq!(r2.changed, 0, "second pass is warm again");
    }

    #[test]
    fn pins_are_idempotent_and_reversible() {
        let dir = tempfile::tempdir().unwrap();
        let db = sled::Config::new()
            .path(dir.path().join("s"))
            .open()
            .unwrap();
        let a = ObjectId::new("fs", "aaa");
        let b = ObjectId::new("fs", "bbb");
        assert!(add_pin(&db, a.clone(), b.clone(), "relates").unwrap());
        assert!(
            !add_pin(&db, b.clone(), a.clone(), "relates").unwrap(),
            "dup suppressed"
        );
        assert_eq!(load_pins(&db).len(), 1);
        assert_eq!(remove_pin(&db, a, b).unwrap(), 1);
        assert!(load_pins(&db).is_empty());
    }

    #[test]
    fn trail_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let db = sled::Config::new()
            .path(dir.path().join("s"))
            .open()
            .unwrap();
        push_trail(
            &db,
            NavStep {
                kind: "query".into(),
                focus: "order signal".into(),
                at: "t".into(),
            },
        );
        push_trail(
            &db,
            NavStep {
                kind: "enter".into(),
                focus: "fs:abc".into(),
                at: "t".into(),
            },
        );
        let trail = load_trail(&db);
        assert_eq!(trail.len(), 2);
        assert_eq!(trail[0].focus, "order signal");
    }

    #[test]
    fn read_object_returns_exact_lines() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), "a.md", "one\ntwo\nthree\nfour\n");
        let obj = PhysisObject {
            id: ObjectId::new("fs", "x"),
            kind: ObjectKind::Passage,
            label: "a.md".into(),
            source: Some(SourceRef {
                path: "a.md".into(),
                start_line: Some(2),
                end_line: Some(3),
            }),
            mtime: 0.0,
            hash: "h".into(),
            text: String::new(),
            embedding: Vec::new(),
        };
        let text = read_object(&obj, dir.path()).unwrap();
        assert_eq!(text, "two\nthree");
    }

    /// Context bias: the same query ranks in-context objects first, and the
    /// component is visible in the score map.
    #[test]
    fn context_component_visible_and_effective() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), "alpha/parser.md", "alpha parser things\n");
        seed(dir.path(), "beta/parser.md", "beta parser things\n");
        let idx = build_index(dir.path());
        let hits = query(
            &idx,
            "parser",
            None,
            &NavWeights::default(),
            "alpha",
            &[],
            10,
        );
        let alpha = hits
            .iter()
            .position(|h| {
                h.object
                    .source
                    .as_ref()
                    .map(|s| s.path.starts_with("alpha"))
                    .unwrap_or(false)
            })
            .unwrap();
        let beta = hits
            .iter()
            .position(|h| {
                h.object
                    .source
                    .as_ref()
                    .map(|s| s.path.starts_with("beta"))
                    .unwrap_or(false)
            })
            .unwrap();
        assert!(alpha < beta, "context must outrank");
    }

    #[test]
    fn grouping_orders_by_group_size() {
        let mk = |kind: ObjectKind, text: &str| NavHit {
            object: PhysisObject {
                id: ObjectId::new("fs", text),
                kind,
                label: text.into(),
                source: None,
                mtime: 0.0,
                hash: String::new(),
                text: text.into(),
                embedding: Vec::new(),
            },
            score: RetrievalScore::default(),
        };
        let hits = vec![
            mk(ObjectKind::Passage, "p1"),
            mk(ObjectKind::Passage, "p2"),
            mk(ObjectKind::Note, "n1"),
        ];
        let groups = group_hits(&hits);
        assert_eq!(groups[0].0, Group::Passages);
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].0, Group::Notes);
    }
}

// ---------------------------------------------------------------------------
// Deferred concepts (§32 Final Interface): object relations with provenance,
// surfaced as relation edges usable by navigation, graph and notebook views.
// Not a new graph system — pins were the first relation; these extend the
// same store with the three privileged human operations (SAME / DIFFERENT /
// RELATED).
// ---------------------------------------------------------------------------

/// How a relation came to exist. Never collapsed: a user assertion and a
/// model suggestion must not look identical (§33).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationOrigin {
    /// The user asserted this directly (pin/same/different).
    User,
    /// Deterministic measurement (shared hash, same source region).
    Deterministic,
    /// Derived from retrieval signals (cosine above threshold).
    Inferred,
}

/// Status of a relation — epistemic state kept separate from origin (§34).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationStatus {
    Asserted,
    Inferred,
    Disputed,
}

/// One user-visible relation between two objects. `Pin` remains the
/// "relates to" case with `RelationOrigin::User`; this generalises it so
/// SAME (≈), DIFFERENT (≠) and directional RELATED share one store, one
/// inspector and one effect on navigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    pub a: ObjectId,
    pub b: ObjectId,
    /// Machine-readable kind: "same" | "different" | custom verb
    /// ("references", "supersedes", …).
    pub kind: String,
    /// Optional human qualification ("same mechanism", "different cause").
    pub note: String,
    pub origin: RelationOrigin,
    pub status: RelationStatus,
    pub created_at: String,
}

/// Canonical edge id for a nav relation: endpoints ordered, kind in the
/// middle. Direction-free (a RELATED b and b RELATED a are one claim);
/// directed kinds (`extends`, `depends_on`) keep byte order via the note.
/// Pure derivation — no stored id, no migration, same pair+kind replays to
/// the same id on every load so the world surface can select/edit the edge.
pub fn relation_id(a: &ObjectId, kind: &str, b: &ObjectId) -> String {
    let (x, y) = if a.short() <= b.short() {
        (a.short(), b.short())
    } else {
        (b.short(), a.short())
    };
    format!("rel:{x}:{kind}:{y}")
}

/// Add a relation; idempotent by (a,b,kind) in both directions.
pub fn add_relation(
    db: &sled::Db,
    a: ObjectId,
    b: ObjectId,
    kind: &str,
    note: &str,
    origin: RelationOrigin,
) -> anyhow::Result<bool> {
    let mut rels = load_relations(db);
    let dup = rels
        .iter()
        .any(|r| (r.a == a && r.b == b || r.a == b && r.b == a) && r.kind == kind);
    if dup {
        return Ok(false);
    }
    rels.push(Relation {
        a,
        b,
        kind: kind.to_string(),
        note: note.to_string(),
        origin,
        status: RelationStatus::Asserted,
        created_at: chrono::Utc::now().to_rfc3339(),
    });
    save_relations(db, &rels)?;
    Ok(true)
}

/// Remove every relation matching (a,b) in either direction and (optionally)
/// a specific kind; `kind None` removes all kinds between the pair.
pub fn remove_relation(
    db: &sled::Db,
    a: &ObjectId,
    b: &ObjectId,
    kind: Option<&str>,
) -> anyhow::Result<usize> {
    let mut rels = load_relations(db);
    let before = rels.len();
    rels.retain(|r| {
        let between = (r.a == *a && r.b == *b) || (r.a == *b && r.b == *a);
        !(between && kind.map(|k| r.kind == k).unwrap_or(true))
    });
    let removed = before - rels.len();
    if removed > 0 {
        save_relations(db, &rels)?;
    }
    Ok(removed)
}

const RELATIONS_KEY: &str = "nav:relations";

pub fn load_relations(db: &sled::Db) -> Vec<Relation> {
    db.get(RELATIONS_KEY)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or_default()
}

pub fn save_relations(db: &sled::Db, rels: &[Relation]) -> anyhow::Result<()> {
    db.insert(RELATIONS_KEY, serde_json::to_vec(rels)?)?;
    Ok(())
}

/// Relations attached to one object (either side), for the inspector pane.
pub fn relations_of(db: &sled::Db, id: &ObjectId) -> Vec<Relation> {
    load_relations(db)
        .into_iter()
        .filter(|r| r.a == *id || r.b == *id)
        .collect()
}

/// Infer same-source relations deterministically: two objects whose source
/// path is identical share provenance (origin `Deterministic`, status
/// `Inferred`). Deliberately conservative — retrieval-derived inference is
/// a suggestion, and suggestions never wear assertion clothes (§33).
pub fn infer_same_source_relations(index: &NavIndex, db: &sled::Db) -> anyhow::Result<usize> {
    use std::collections::BTreeMap;
    let mut by_path: BTreeMap<String, Vec<ObjectId>> = BTreeMap::new();
    for o in index.objects.values() {
        if let Some(s) = &o.source {
            by_path
                .entry(s.path.clone())
                .or_default()
                .push(o.id.clone());
        }
    }
    let mut added = 0usize;
    for ids in by_path.values() {
        for pair in ids.windows(2) {
            if add_relation(
                db,
                pair[0].clone(),
                pair[1].clone(),
                "same-source",
                "",
                RelationOrigin::Deterministic,
            )? {
                added += 1;
            }
        }
    }
    Ok(added)
}

/// Semantic parents of an object — deliberately plural (§29 of the MVP spec /
/// §12 of the Final Interface): the filesystem parent is only one of them.
/// Derived on demand, not stored, so the definitions can grow without a
/// migration:
/// - `fs:` parent — the filesystem directory.
/// - `kind:` parent — every object of this kind forms one loose group.
/// - `note:` parent — the file-level object of the file a passage lives in.
pub fn semantic_parents(obj: &PhysisObject) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(src) = &obj.source {
        if let Some(dir) = src
            .path
            .rsplit_once('.')
            .and_then(|_| src.path.rsplit('/').next())
        {
            let _ = dir; // path handled below
        }
        if let Some((dir, _file)) = src.path.rsplit_once('/') {
            out.push(("fs".to_string(), dir.to_string()));
        } else {
            out.push(("fs".to_string(), ".".to_string()));
        }
        // Passage → its file-level parent.
        if obj.kind == ObjectKind::Passage {
            let file = src.path.split('/').next_back().unwrap_or(&src.path);
            out.push(("note".to_string(), file.to_string()));
        }
    }
    out.push(("kind".to_string(), obj.kind.label().to_string()));
    out
}

/// Map one nav relation kind onto the core `RelationType` vocabulary (Phase
/// 30 W2 bridge). SAME/DIFFERENT are the privileged ops; the common custom
/// verbs map to their named counterpart; anything unmapped stays `None` and
/// the bridge skips it openly rather than inventing a mapping.
pub fn relation_kind_to_core(kind: &str) -> Option<physis_core::relation::RelationType> {
    use physis_core::relation::RelationType as R;
    match kind.trim().to_lowercase().as_str() {
        "same" | "same-as" | "equivalent_to" | "equivalent-to" | "same-source" => None,
        "different" | "differs-from" | "differs_from" => None,
        "references" | "reference" => None,
        "supersedes" | "supersede" => Some(R::Invalidates),
        "extends" => Some(R::DerivedFrom),
        "depends_on" | "depends-on" | "depends on" => Some(R::DependsOn),
        "causes" | "cause" => Some(R::Causes),
        "produces" | "produce" => Some(R::Produces),
        "supports" | "support" => Some(R::Supports),
        "contradicts" | "contradict" => Some(R::Contradicts),
        "precedes" => Some(R::Precedes),
        "relates to" | "related" | "relates" => None,
        _ => None,
    }
}

/// Extend `query`: relations contribute a `relation` component. An object
/// rides up when a related partner matches the query (lexical or semantic),
/// same mechanism as pins but over all relation kinds. Pins are honoured too
/// (they are `kind = "relates to"` relations in spirit — kept separate for
/// backwards compatibility of the pin store).
#[allow(clippy::too_many_arguments)]
pub fn query_with_relations(
    index: &NavIndex,
    q: &str,
    embedder: Option<&dyn VectorEmbed>,
    weights: &NavWeights,
    context_path: &str,
    pins: &[Pin],
    relations: &[Relation],
    limit: usize,
) -> Vec<NavHit> {
    // Build adjacency from pins + relations, then reuse `query`'s scorer by
    // temporarily extending the pin list — one scoring path, two stores.
    let mut merged: Vec<Pin> = pins.to_vec();
    for r in relations {
        merged.push(Pin {
            a: r.a.clone(),
            b: r.b.clone(),
            relation: r.kind.clone(),
            created_at: r.created_at.clone(),
        });
    }
    query(index, q, embedder, weights, context_path, &merged, limit)
}

#[cfg(test)]
mod relation_tests {
    use super::tests::{build_index, seed};
    use super::*;
    use crate::embed::RandomProjectionEmbedder;

    #[test]
    fn relations_idempotent_and_reversible() {
        let dir = tempfile::tempdir().unwrap();
        let db = sled::Config::new()
            .path(dir.path().join("s"))
            .open()
            .unwrap();
        let a = ObjectId::new("fs", "aaa");
        let b = ObjectId::new("fs", "bbb");
        assert!(add_relation(&db, a.clone(), b.clone(), "same", "", RelationOrigin::User).unwrap());
        // Reversed pair, same kind: still a dup.
        assert!(
            !add_relation(&db, b.clone(), a.clone(), "same", "", RelationOrigin::User).unwrap()
        );
        // Different kind: allowed.
        assert!(add_relation(
            &db,
            a.clone(),
            b.clone(),
            "different",
            "",
            RelationOrigin::User
        )
        .unwrap());
        assert_eq!(load_relations(&db).len(), 2);
        assert_eq!(relations_of(&db, &a).len(), 2);
        assert_eq!(remove_relation(&db, &a, &b, Some("same")).unwrap(), 1);
        assert_eq!(remove_relation(&db, &a, &b, None).unwrap(), 1);
        assert!(load_relations(&db).is_empty());
    }

    #[test]
    fn kind_mapping_only_maps_traceable_verbs() {
        use physis_core::relation::RelationType as R;
        assert_eq!(relation_kind_to_core("extends"), Some(R::DerivedFrom));
        assert_eq!(relation_kind_to_core("depends_on"), Some(R::DependsOn));
        assert_eq!(relation_kind_to_core("causes"), Some(R::Causes));
        assert_eq!(relation_kind_to_core("same"), None);
        assert_eq!(relation_kind_to_core("different"), None);
        assert_eq!(relation_kind_to_core("references"), None);
        assert_eq!(relation_kind_to_core("relates to"), None);
        assert_eq!(relation_kind_to_core("frobnicate"), None);
    }

    #[test]
    fn relation_ids_are_canonical_across_endpoint_order() {
        let a = ObjectId::new("fs", "aaa");
        let b = ObjectId::new("fs", "bbb");
        assert_eq!(
            relation_id(&a, "extends", &b),
            relation_id(&b, "extends", &a)
        );
        assert_ne!(
            relation_id(&a, "extends", &b),
            relation_id(&a, "supports", &b)
        );
    }

    #[test]
    fn same_source_inference_is_deterministic_and_scoped() {
        let dir = tempfile::tempdir().unwrap();
        let db = sled::Config::new()
            .path(dir.path().join("s"))
            .open()
            .unwrap();
        seed(dir.path(), "a.md", "one\ntwo\nthree\nfour\nfive\nsix\n");
        let idx = build_index(dir.path());
        let n = infer_same_source_relations(&idx, &db).unwrap();
        assert!(n > 0);
        let rels = load_relations(&db);
        assert!(rels
            .iter()
            .all(|r| r.origin == RelationOrigin::Deterministic));
        // Idempotent: a second pass adds nothing.
        let n2 = infer_same_source_relations(&idx, &db).unwrap();
        assert_eq!(n2, 0);
    }

    #[test]
    fn semantic_parents_are_plural() {
        let obj = PhysisObject {
            id: ObjectId::new("fs", "chunk"),
            kind: ObjectKind::Passage,
            label: "docs/sub/a.md:41-80".into(),
            source: Some(SourceRef {
                path: "docs/sub/a.md".into(),
                start_line: Some(41),
                end_line: Some(80),
            }),
            mtime: 0.0,
            hash: "h".into(),
            text: String::new(),
            embedding: Vec::new(),
        };
        let parents = semantic_parents(&obj);
        assert!(parents.contains(&("fs".into(), "docs/sub".into())));
        assert!(parents.contains(&("note".into(), "a.md".into())));
        assert!(parents.contains(&("kind".into(), "Passage".into())));
        assert_eq!(parents.len(), 3, "plural by design: {parents:?}");
    }

    /// query_with_relations: a relation whose partner matches drags the
    /// object up, exactly like a pin — one scoring path, two stores.
    #[test]
    fn relation_bridge_surfaces_partner() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), "docs/order.md", "order signal experiment\n");
        seed(dir.path(), "src/plain.rs", "unrelated words entirely\n");
        let mut idx = build_index(dir.path());
        let e = RandomProjectionEmbedder::new(64);
        idx.embed_objects(&e, "rp-test");
        let order_id = idx
            .objects
            .values()
            .find(|o| o.kind == ObjectKind::Note)
            .unwrap()
            .id
            .clone();
        let other_id = idx
            .objects
            .values()
            .find(|o| {
                o.source
                    .as_ref()
                    .map(|s| s.path == "src/plain.rs")
                    .unwrap_or(false)
            })
            .unwrap()
            .id
            .clone();
        let db = db_temp();
        add_relation(
            &db,
            order_id,
            other_id,
            "relates to",
            "",
            RelationOrigin::User,
        )
        .unwrap();
        let rels = load_relations(&db);
        let hits = query_with_relations(
            &idx,
            "order signal",
            Some(&e),
            &NavWeights::default(),
            "",
            &[],
            &rels,
            10,
        );
        assert!(hits.iter().any(|h| h
            .object
            .source
            .as_ref()
            .map(|s| s.path == "src/plain.rs")
            .unwrap_or(false)));
    }

    fn db_temp() -> sled::Db {
        // Tempdir is leaked deliberately: the sled file must outlive the Db.
        let dir = tempfile::tempdir().unwrap();
        std::mem::forget(dir);
        sled::Config::new()
            .path(tempfile::tempdir().unwrap().keep().join("s"))
            .open()
            .unwrap()
    }
}
