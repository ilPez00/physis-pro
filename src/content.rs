//! Search by content — retrieval over what documents *say*.
//!
//! Every search surface in the product until now ranked **labels**: node
//! labels, goal labels, theme names. That answers "which of the things I
//! already named is closest to this phrase", which is not the question an
//! operator asks in front of a folder. They ask "where does it say the spindle
//! bearing was replaced", and the answer lives in a paragraph nobody labeled.
//!
//! This module indexes the corpus at the unit the author wrote it in — the
//! heading-scoped section from [`crate::doctree`] — and ranks with two channels
//! that fail in different places:
//!
//! * **lexical (BM25)** finds the exact part number, the error code, the
//!   surname. A vector model routinely misses these: `E-4412` and `E-4413`
//!   embed almost identically.
//! * **semantic (cosine)** finds the paraphrase — "cuscinetto grippato" for
//!   "seized bearing" — which no term match can reach.
//!
//! Fusing them is not decoration: an industrial corpus is half codes and half
//! prose, so either channel alone is wrong on half the corpus.
//!
//! The index is a value: build it, hold it, search it many times. Staleness is
//! decided by a [`IndexSignature`] the caller can cheaply recompute, so a demo
//! that scans once does not re-embed on every keystroke.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Characters of surrounding text returned around a match.
pub const DEFAULT_SNIPPET_CHARS: usize = 320;

/// Weight of the semantic channel in the fused score. The lexical channel takes
/// the remainder. Slightly semantic-leaning: exact-term hits already dominate
/// their own channel's normalization, so an even split over-rewarded documents
/// that merely repeat a common query word.
pub const SEMANTIC_WEIGHT: f32 = 0.55;

/// Which channels to score with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    /// BM25 + cosine, normalized per channel and fused.
    #[default]
    Hybrid,
    /// Cosine only — paraphrase search.
    Semantic,
    /// BM25 only — exact terms, works with no embedder at all.
    Lexical,
}

impl SearchMode {
    pub fn parse(s: &str) -> SearchMode {
        match s.trim().to_ascii_lowercase().as_str() {
            "semantic" => SearchMode::Semantic,
            "lexical" | "keyword" => SearchMode::Lexical,
            _ => SearchMode::Hybrid,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SearchMode::Hybrid => "hybrid",
            SearchMode::Semantic => "semantic",
            SearchMode::Lexical => "lexical",
        }
    }
}

/// One indexed unit: a heading-scoped section of one file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedSection {
    pub path: PathBuf,
    /// Document title (file stem or parsed title).
    pub title: String,
    /// File extension, lowercased. `""` when the file has none.
    pub kind: String,
    /// Heading chain, outermost first — the citation.
    pub breadcrumb: Vec<String>,
    pub level: u8,
    /// Section body as written (what gets snippeted).
    pub body: String,
    /// Filing axes from [`crate::address`], when the file carried them.
    pub place: Option<String>,
    pub period: Option<String>,
    /// Modification time, RFC3339. Sorting and recency filters read this.
    pub modified: Option<String>,
    /// Term frequencies over the section body + breadcrumb.
    pub terms: HashMap<String, u32>,
    /// Total term count (BM25 document length).
    pub length: u32,
    /// Same vector space as every other embedding in the process. Empty when
    /// the index was built without an embedder (lexical-only mode).
    pub embedding: Vec<f32>,
}

impl IndexedSection {
    /// "file.md › Spindle › Reassembly" — what a hit cites.
    ///
    /// The heading chain starts with the document title, which for a file-backed
    /// document is the file stem, so the naive form reads "policy.md › policy ›
    /// Escalation". That first crumb is dropped here and only here: it still
    /// belongs in the embedded text, where the filename is real signal.
    pub fn citation(&self) -> String {
        let name = self
            .path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| self.path.display().to_string());
        let stem = self
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let chain: Vec<&str> = self
            .breadcrumb
            .iter()
            .enumerate()
            .filter(|(i, c)| !(*i == 0 && c.trim().to_ascii_lowercase() == stem))
            .map(|(_, c)| c.as_str())
            .collect();
        if chain.is_empty() {
            name
        } else {
            format!("{name} › {}", chain.join(" › "))
        }
    }
}

/// Cheap staleness token. Two indexes over the same root with equal signatures
/// were built from the same bytes — recomputing it costs a directory walk of
/// **metadata only**, not a read of every file and certainly not an embedding
/// pass. That difference is what lets a search box re-check freshness per
/// keystroke instead of per minute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct IndexSignature {
    pub files: usize,
    pub bytes: u64,
    /// Newest document timestamp seen, as epoch seconds.
    pub newest: i64,
}

/// A built content index over one root directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentIndex {
    pub root: PathBuf,
    pub sections: Vec<IndexedSection>,
    pub files: usize,
    pub signature: IndexSignature,
    /// Document frequency per term across sections (BM25 idf input).
    df: HashMap<String, usize>,
    /// Mean section length in terms; never zero (guards the BM25 divisor).
    avg_len: f32,
    /// True when the corpus was larger than the section cap and got truncated.
    pub truncated: bool,
    /// Whether embeddings were computed (semantic channel available).
    pub semantic: bool,
}

/// Upper bound on indexed sections. A runaway scan should degrade to "the first
/// N sections, and it says so", never to an unbounded embedding job that looks
/// like a hang.
pub const MAX_SECTIONS: usize = 6000;

impl ContentIndex {
    /// Build from already-ingested documents plus their embeddings.
    ///
    /// Embeddings are passed in rather than computed here so the caller owns the
    /// cache and the threading: the web layer serves most of them from the
    /// content-addressed [`crate::pack::EmbedCache`] and batches the rest off
    /// the async runtime. Pass an empty vec for a lexical-only index.
    ///
    /// `addresses` is parallel to `docs` and may be shorter; missing entries
    /// simply carry no place/period.
    pub fn build(
        root: &Path,
        docs: &[crate::ingest::IngestedDoc],
        addresses: &[Option<crate::address::FileAddress>],
        embeddings: Vec<Vec<f32>>,
    ) -> ContentIndex {
        let units = section_units(root, docs, addresses);
        let truncated = units.len() > MAX_SECTIONS;
        let mut sections: Vec<IndexedSection> = units.into_iter().take(MAX_SECTIONS).collect();

        // Positional pairing with the embedding vector. A mismatch here would
        // silently attach one section's meaning to another's text, so short
        // input leaves the tail lexical-only rather than shifting.
        let semantic = !embeddings.is_empty();
        for (sec, emb) in sections.iter_mut().zip(embeddings) {
            sec.embedding = emb;
        }

        let mut df: HashMap<String, usize> = HashMap::new();
        let mut total_len: u64 = 0;
        for s in &sections {
            total_len += s.length as u64;
            for term in s.terms.keys() {
                *df.entry(term.clone()).or_insert(0) += 1;
            }
        }
        let avg_len = if sections.is_empty() {
            1.0
        } else {
            (total_len as f32 / sections.len() as f32).max(1.0)
        };

        ContentIndex {
            root: root.to_path_buf(),
            files: docs.len(),
            signature: scan_signature(root),
            sections,
            df,
            avg_len,
            truncated,
            semantic,
        }
    }

    /// The retrieval texts, in section order — what the caller must embed to
    /// give this index its semantic channel.
    pub fn retrieval_texts(
        root: &Path,
        docs: &[crate::ingest::IngestedDoc],
        addresses: &[Option<crate::address::FileAddress>],
    ) -> Vec<String> {
        section_units(root, docs, addresses)
            .into_iter()
            .take(MAX_SECTIONS)
            .map(|s| {
                // Identical to doctree's retrieval unit: breadcrumb, then body.
                // The embedder only ever sees this string, so context absent
                // from it cannot influence the match.
                let chain = s.breadcrumb.join(" > ");
                if s.body.trim().is_empty() {
                    chain
                } else {
                    format!("{chain}\n\n{}", s.body.trim())
                }
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.sections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    fn idf(&self, term: &str) -> f32 {
        let n = self.sections.len().max(1) as f32;
        let df = *self.df.get(term).unwrap_or(&0) as f32;
        // Standard BM25 idf, +1 inside the log so a term present in every
        // section scores 0 rather than going negative and *penalizing* a match.
        ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
    }

    /// Rank sections against a query. `q_emb` may be empty (no semantic channel).
    pub fn search(&self, query: &str, q_emb: &[f32], opts: &SearchOptions) -> ContentSearchResult {
        let terms = tokenize(query);
        let phrases = phrases(query);
        let mode = if q_emb.is_empty() || !self.semantic {
            // With no vectors there is only one channel, whatever was asked
            // for. Reporting "hybrid" here would be a quiet lie: the caller
            // would read paraphrase matching into a pure keyword result, and
            // hybrid's own weighting never ran. Semantic likewise cannot
            // return an order that merely *looks* ranked.
            SearchMode::Lexical
        } else {
            opts.mode
        };

        let mut scored: Vec<Candidate> = Vec::new();
        for (i, s) in self.sections.iter().enumerate() {
            if !opts.matches_filters(s) {
                continue;
            }
            if !phrases.is_empty() && !phrases.iter().all(|p| contains_ci(&s.body, p)) {
                continue;
            }

            let mut lexical = 0.0f32;
            for t in &terms {
                let tf = *s.terms.get(t).unwrap_or(&0) as f32;
                if tf == 0.0 {
                    continue;
                }
                // BM25 with the usual k1/b. The length norm matters here more
                // than in a web corpus: sections range from a one-line heading
                // to a 900-word procedure, and without it every long procedure
                // outranks the precise paragraph that answers the question.
                const K1: f32 = 1.2;
                const B: f32 = 0.75;
                let norm = 1.0 - B + B * (s.length as f32 / self.avg_len);
                lexical += self.idf(t) * (tf * (K1 + 1.0)) / (tf + K1 * norm);
            }

            let semantic = if q_emb.is_empty() || s.embedding.is_empty() {
                0.0
            } else {
                cosine(q_emb, &s.embedding)
            };

            let keep = match mode {
                SearchMode::Lexical => lexical > 0.0,
                SearchMode::Semantic => semantic > 0.0,
                // A phrase-only or filter-only query has no scoring terms at
                // all; those still deserve their matches back.
                SearchMode::Hybrid => {
                    lexical > 0.0 || semantic > 0.0 || (terms.is_empty() && !phrases.is_empty())
                }
            };
            if keep {
                scored.push(Candidate {
                    idx: i,
                    lexical,
                    semantic,
                });
            }
        }

        // Per-channel max normalization, computed over the candidate set rather
        // than a fixed range: BM25 has no upper bound, and cosine's usable band
        // is model-dependent. Normalizing makes the two comparable without
        // pretending either is a probability.
        let max_lex = scored.iter().fold(0.0f32, |m, c| m.max(c.lexical));
        let max_sem = scored.iter().fold(0.0f32, |m, c| m.max(c.semantic));

        let mut hits: Vec<ContentHit> = scored
            .into_iter()
            .map(|c| {
                let l = if max_lex > 0.0 {
                    c.lexical / max_lex
                } else {
                    0.0
                };
                let s = if max_sem > 0.0 {
                    c.semantic / max_sem
                } else {
                    0.0
                };
                let score = match mode {
                    SearchMode::Lexical => l,
                    SearchMode::Semantic => s,
                    SearchMode::Hybrid => SEMANTIC_WEIGHT * s + (1.0 - SEMANTIC_WEIGHT) * l,
                };
                (c, score)
            })
            .filter(|(_, score)| *score > 0.0)
            .map(|(c, score)| {
                let sec = &self.sections[c.idx];
                let (snippet, highlights) =
                    snippet(&sec.body, &terms, &phrases, opts.snippet_chars);
                ContentHit {
                    id: c.idx,
                    path: sec.path.display().to_string(),
                    title: sec.title.clone(),
                    kind: sec.kind.clone(),
                    citation: sec.citation(),
                    breadcrumb: sec.breadcrumb.clone(),
                    level: sec.level,
                    score,
                    lexical: c.lexical,
                    semantic: c.semantic,
                    snippet,
                    highlights,
                    place: sec.place.clone(),
                    period: sec.period.clone(),
                    modified: sec.modified.clone(),
                }
            })
            .collect();

        // Deterministic order: score first, then the citation, so two sections
        // that tie do not swap places between identical requests.
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.citation.cmp(&b.citation))
        });

        // Facets describe the whole match set, not the page: a count that
        // shrank when you scrolled would be a lie about the corpus.
        let facets = Facets::from_hits(&hits);
        let matched = hits.len();
        hits.truncate(opts.top_k.max(1));

        ContentSearchResult {
            mode,
            matched,
            hits,
            facets,
            indexed_sections: self.sections.len(),
            indexed_files: self.files,
            truncated_index: self.truncated,
        }
    }
}

struct Candidate {
    idx: usize,
    lexical: f32,
    semantic: f32,
}

/// Query-time knobs.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub mode: SearchMode,
    pub top_k: usize,
    pub snippet_chars: usize,
    pub place: Option<String>,
    pub period: Option<String>,
    pub kind: Option<String>,
    /// Substring the file path must contain (folder scoping).
    pub path_contains: Option<String>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        SearchOptions {
            mode: SearchMode::Hybrid,
            top_k: 20,
            snippet_chars: DEFAULT_SNIPPET_CHARS,
            place: None,
            period: None,
            kind: None,
            path_contains: None,
        }
    }
}

impl SearchOptions {
    fn matches_filters(&self, s: &IndexedSection) -> bool {
        if let Some(p) = &self.place {
            if s.place.as_deref() != Some(p.as_str()) {
                return false;
            }
        }
        if let Some(p) = &self.period {
            if s.period.as_deref() != Some(p.as_str()) {
                return false;
            }
        }
        if let Some(k) = &self.kind {
            if !s.kind.eq_ignore_ascii_case(k) {
                return false;
            }
        }
        if let Some(sub) = &self.path_contains {
            if !contains_ci(&s.path.display().to_string(), sub) {
                return false;
            }
        }
        true
    }
}

/// One ranked section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentHit {
    /// Index into the live [`ContentIndex`] — lets a caller fetch the full
    /// section without re-searching.
    pub id: usize,
    pub path: String,
    pub title: String,
    pub kind: String,
    pub citation: String,
    pub breadcrumb: Vec<String>,
    pub level: u8,
    pub score: f32,
    /// Raw channel scores, kept so the UI can show *why* something ranked.
    pub lexical: f32,
    pub semantic: f32,
    pub snippet: String,
    /// `[start, len]` character ranges within `snippet` to mark.
    pub highlights: Vec<[usize; 2]>,
    pub place: Option<String>,
    pub period: Option<String>,
    pub modified: Option<String>,
}

/// Counted values for the filter rail.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Facets {
    pub place: Vec<FacetCount>,
    pub period: Vec<FacetCount>,
    pub kind: Vec<FacetCount>,
    pub file: Vec<FacetCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacetCount {
    pub value: String,
    pub count: usize,
}

impl Facets {
    fn from_hits(hits: &[ContentHit]) -> Facets {
        let mut place: HashMap<String, usize> = HashMap::new();
        let mut period: HashMap<String, usize> = HashMap::new();
        let mut kind: HashMap<String, usize> = HashMap::new();
        let mut file: HashMap<String, usize> = HashMap::new();
        for h in hits {
            if let Some(p) = &h.place {
                *place.entry(p.clone()).or_insert(0) += 1;
            }
            if let Some(p) = &h.period {
                *period.entry(p.clone()).or_insert(0) += 1;
            }
            if !h.kind.is_empty() {
                *kind.entry(h.kind.clone()).or_insert(0) += 1;
            }
            *file.entry(h.path.clone()).or_insert(0) += 1;
        }
        Facets {
            place: rank(place, 20),
            period: rank(period, 20),
            kind: rank(kind, 20),
            file: rank(file, 40),
        }
    }
}

fn rank(map: HashMap<String, usize>, cap: usize) -> Vec<FacetCount> {
    let mut v: Vec<FacetCount> = map
        .into_iter()
        .map(|(value, count)| FacetCount { value, count })
        .collect();
    // Count desc, then value asc — a stable order the UI can diff against.
    v.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.value.cmp(&b.value)));
    v.truncate(cap);
    v
}

/// What one search returns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSearchResult {
    /// The mode actually used — may differ from the requested one when the
    /// index has no vectors.
    pub mode: SearchMode,
    /// Sections that matched before `top_k` truncation.
    pub matched: usize,
    pub hits: Vec<ContentHit>,
    pub facets: Facets,
    pub indexed_sections: usize,
    pub indexed_files: usize,
    pub truncated_index: bool,
}

// ── Index construction ────────────────────────────────────────────────────

fn section_units(
    root: &Path,
    docs: &[crate::ingest::IngestedDoc],
    addresses: &[Option<crate::address::FileAddress>],
) -> Vec<IndexedSection> {
    let mut out = Vec::new();
    for (i, d) in docs.iter().enumerate() {
        let addr = addresses.get(i).and_then(|a| a.as_ref());
        let kind = d
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let modified = Some(
            addr.map(|a| a.time.modified.to_rfc3339())
                .unwrap_or_else(|| d.timestamp.to_rfc3339()),
        );
        let place = Some(place_of(root, &d.path, addr));
        let period = addr.map(|a| a.time.period.clone()).or_else(|| {
            // No address (an index built straight off a scan) still deserves a
            // time facet; the month bucket is derivable from the same fact the
            // address would have used.
            Some(d.timestamp.format("%Y-%m").to_string())
        });
        for sec in crate::doctree::parse(&d.title, &d.body) {
            // The breadcrumb joins the term pool: a section titled "Bearing
            // preload" whose body never repeats the phrase must still be
            // findable by it.
            let mut terms: HashMap<String, u32> = HashMap::new();
            let mut length = 0u32;
            for t in tokenize(&sec.body)
                .into_iter()
                .chain(tokenize(&sec.breadcrumb.join(" ")))
            {
                *terms.entry(t).or_insert(0) += 1;
                length += 1;
            }
            out.push(IndexedSection {
                path: d.path.clone(),
                title: d.title.clone(),
                kind: kind.clone(),
                breadcrumb: sec.breadcrumb.clone(),
                level: sec.level,
                body: sec.body.clone(),
                place: place.clone(),
                period: period.clone(),
                modified: modified.clone(),
                terms,
                length,
                embedding: Vec::new(),
            });
        }
    }
    out
}

/// The place facet for one file.
///
/// The address's own `place` defaults to the scan root itself, which would make
/// the facet a single value covering everything — a filter that filters
/// nothing. So the root case falls back to the file's folder relative to the
/// root, which is the distinction an operator actually navigates by
/// (`line-2/maintenance` vs `office/invoices`). A named place from the space
/// ontology always wins, because that is a human decision and this is a guess.
fn place_of(root: &Path, path: &Path, addr: Option<&crate::address::FileAddress>) -> String {
    if let Some(a) = addr {
        if a.space.place != a.space.root {
            return a.space.place.clone();
        }
    }
    let parent = path.parent().unwrap_or(path);
    match parent.strip_prefix(root) {
        Ok(rel) if !rel.as_os_str().is_empty() => rel.display().to_string(),
        // Directly under the root, or outside it entirely (an importer's path).
        Ok(_) => "/".to_string(),
        Err(_) => parent
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string()),
    }
}

/// Metadata-only signature of everything under `root`.
///
/// No file is opened: size and mtime answer "did anything change" for every
/// realistic edit, and the one case they miss — a same-size, same-mtime
/// rewrite — is what the explicit refresh is for. An unreadable root yields
/// the zero signature, which compares unequal to any built index and therefore
/// forces a rebuild rather than serving stale results.
pub fn scan_signature(root: &Path) -> IndexSignature {
    let mut sig = IndexSignature::default();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(md) = entry.metadata() else { continue };
        sig.files += 1;
        sig.bytes += md.len();
        if let Ok(t) = md.modified() {
            if let Ok(d) = t.duration_since(std::time::UNIX_EPOCH) {
                sig.newest = sig.newest.max(d.as_secs() as i64);
            }
        }
    }
    sig
}

// ── Text utilities ────────────────────────────────────────────────────────

/// Terms that carry no discrimination in either language this corpus mixes.
/// Kept deliberately short: an aggressive stoplist eats real query terms
/// ("no fault found", "in process"), and BM25's idf already suppresses the
/// common ones. This list only removes words so frequent they distort the
/// length normalization.
const STOPWORDS: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "was", "were", "are", "has", "have",
    "not", "but", "you", "your", "its", "our", "all", "any", "can", "will", "into", "per", "che",
    "con", "del", "della", "delle", "dei", "degli", "una", "uno", "gli", "sono", "come", "nel",
    "nella", "per", "non", "alla", "allo", "dal", "dalla",
];

/// Lowercase alphanumeric terms, stopwords and 1-character noise removed.
///
/// Digits are kept attached to letters so `E4412`, `iso9001` and `24v` survive
/// as single terms — splitting them is how a parts search stops working.
pub fn tokenize(text: &str) -> Vec<String> {
    let stop: HashSet<&str> = STOPWORDS.iter().copied().collect();
    let mut out = Vec::new();
    let mut word = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            word.extend(c.to_lowercase());
        } else {
            push_term(&mut out, &mut word, &stop);
        }
    }
    push_term(&mut out, &mut word, &stop);
    out
}

fn push_term(out: &mut Vec<String>, word: &mut String, stop: &HashSet<&str>) {
    if word.chars().count() > 1 && !stop.contains(word.as_str()) {
        out.push(word.clone());
    }
    word.clear();
}

/// Quoted spans in the query, lowercased. `"seized bearing" spindle` yields
/// `["seized bearing"]` — those must appear verbatim.
pub fn phrases(query: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_quote = false;
    let mut cur = String::new();
    for c in query.chars() {
        match c {
            '"' => {
                if in_quote {
                    let t = cur.trim().to_lowercase();
                    if !t.is_empty() {
                        out.push(t);
                    }
                    cur.clear();
                }
                in_quote = !in_quote;
            }
            _ if in_quote => cur.push(c),
            _ => {}
        }
    }
    out
}

fn contains_ci(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na <= 0.0 || nb <= 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// The window of `body` that shows the most of what was asked for, plus the
/// ranges to mark inside it.
///
/// Offsets are in **characters**, not bytes: the front end indexes the string
/// the same way, and a byte offset lands mid-codepoint the first time an
/// accented word matches — which in this corpus is immediately.
pub fn snippet(
    body: &str,
    terms: &[String],
    phrases: &[String],
    width: usize,
) -> (String, Vec<[usize; 2]>) {
    let chars: Vec<char> = body.chars().collect();
    if chars.is_empty() {
        return (String::new(), Vec::new());
    }
    let lower: Vec<char> = body.to_lowercase().chars().collect();
    // to_lowercase can change char count (ß → ss, İ → i̇). When it does, the
    // two vectors no longer align and every offset would be wrong, so fall
    // back to a plain head snippet instead of marking the wrong characters.
    if lower.len() != chars.len() {
        let end = chars.len().min(width);
        let text: String = chars[..end].iter().collect();
        return (
            if end < chars.len() {
                format!("{text}…")
            } else {
                text
            },
            Vec::new(),
        );
    }

    // Match positions for every needle: whole-word for terms, verbatim for
    // phrases.
    let mut marks: Vec<(usize, usize)> = Vec::new();
    for p in phrases {
        marks.extend(find_all(&lower, &p.chars().collect::<Vec<_>>(), false));
    }
    for t in terms {
        marks.extend(find_all(&lower, &t.chars().collect::<Vec<_>>(), true));
    }
    // Longest span first at a shared start: a quoted phrase and its own words
    // both match at the same offset, and the renderer keeps the first of any
    // overlapping pair. Plain ascending order hands that to the single word, so
    // `"no fault found"` was marked as three separate terms — the phrase the
    // user asked for verbatim shown as if it were three coincidences.
    marks.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)));
    marks.dedup();

    if marks.is_empty() {
        let end = chars.len().min(width);
        let text: String = chars[..end].iter().collect();
        return (
            if end < chars.len() {
                format!("{text}…")
            } else {
                text
            },
            Vec::new(),
        );
    }

    // Pick the window covering the most marks; ties go to the earliest, which
    // for a document usually means the more definitional passage.
    let mut best_start = 0usize;
    let mut best_hits = 0usize;
    for (start, _) in &marks {
        let from = start.saturating_sub(width / 4);
        let to = from + width;
        let hits = marks.iter().filter(|(s, e)| *s >= from && *e <= to).count();
        if hits > best_hits {
            best_hits = hits;
            best_start = from;
        }
    }
    let start = best_start.min(chars.len().saturating_sub(1));
    let end = (start + width).min(chars.len());
    let text: String = chars[start..end].iter().collect();

    let highlights: Vec<[usize; 2]> = marks
        .iter()
        .filter(|(s, e)| *s >= start && *e <= end)
        .map(|(s, e)| [s - start, e - s])
        .collect();

    let mut out = text;
    if end < chars.len() {
        out.push('…');
    }
    if start > 0 {
        // Prefixing shifts every offset by one; do it after computing them and
        // adjust, rather than computing against a string that does not exist yet.
        let shifted = highlights.iter().map(|h| [h[0] + 1, h[1]]).collect();
        return (format!("…{out}"), shifted);
    }
    (out, highlights)
}

/// All occurrences of `needle` in `hay`. With `word_boundary`, a match must not
/// be flanked by alphanumerics — otherwise searching "oil" highlights half of
/// "boiler" and the snippet looks broken.
fn find_all(hay: &[char], needle: &[char], word_boundary: bool) -> Vec<(usize, usize)> {
    if needle.is_empty() || needle.len() > hay.len() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in 0..=(hay.len() - needle.len()) {
        if hay[i..i + needle.len()] != *needle {
            continue;
        }
        if word_boundary {
            let before_ok = i == 0 || !hay[i - 1].is_alphanumeric();
            let after = i + needle.len();
            let after_ok = after >= hay.len() || !hay[after].is_alphanumeric();
            if !(before_ok && after_ok) {
                continue;
            }
        }
        out.push((i, i + needle.len()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::{DocKind, DocMeta, IngestedDoc};

    fn doc(path: &str, title: &str, body: &str) -> IngestedDoc {
        IngestedDoc {
            path: PathBuf::from(path),
            kind: DocKind::Report,
            title: title.to_string(),
            body: body.to_string(),
            timestamp: chrono::Utc::now(),
            ontology_tags: Vec::new(),
            meta: DocMeta::default(),
        }
    }

    fn index(docs: Vec<IngestedDoc>) -> ContentIndex {
        ContentIndex::build(Path::new("/corpus"), &docs, &[], Vec::new())
    }

    #[test]
    fn tokenize_keeps_alphanumeric_codes_together() {
        // A parts search is the canonical lexical query; splitting "e4412"
        // into "e" + "4412" is how it stops working.
        let t = tokenize("Replaced part E4412 on the spindle (24V).");
        assert!(t.contains(&"e4412".to_string()));
        assert!(t.contains(&"24v".to_string()));
        assert!(t.contains(&"spindle".to_string()));
        assert!(!t.contains(&"the".to_string()), "stopword survived: {t:?}");
    }

    #[test]
    fn phrases_extracts_quoted_spans_only() {
        let p = phrases("\"seized bearing\" spindle \"no fault found\"");
        assert_eq!(p, vec!["seized bearing", "no fault found"]);
        assert!(phrases("no quotes here").is_empty());
    }

    #[test]
    fn a_citation_does_not_repeat_the_filename_as_its_first_heading() {
        // doctree roots the chain at the document title, which for a file is
        // the stem — "policy.md › policy › Escalation" reads as a bug.
        let idx = index(vec![doc(
            "/corpus/policy.md",
            "policy",
            "# Maintenance policy\n\n## Escalation\n\nRepeated E4412 escalates.\n",
        )]);
        let r = idx.search("escalates", &[], &SearchOptions::default());
        assert_eq!(r.hits.len(), 1, "{:?}", r.hits);
        assert_eq!(
            r.hits[0].citation,
            "policy.md › Maintenance policy › Escalation"
        );
    }

    #[test]
    fn a_vectorless_index_reports_lexical_whatever_was_asked_for() {
        // Reporting "hybrid" with no vectors would let the caller read
        // paraphrase matching into a pure keyword result.
        let idx = index(vec![doc("/corpus/a.md", "a", "# One\n\nbearing seized\n")]);
        for requested in [
            SearchMode::Hybrid,
            SearchMode::Semantic,
            SearchMode::Lexical,
        ] {
            let r = idx.search(
                "bearing",
                &[],
                &SearchOptions {
                    mode: requested,
                    ..Default::default()
                },
            );
            assert_eq!(r.mode, SearchMode::Lexical, "requested {requested:?}");
            assert!(!r.hits.is_empty(), "requested {requested:?}");
        }
    }

    #[test]
    fn breadcrumb_terms_are_searchable_even_when_the_body_never_repeats_them() {
        let idx = index(vec![doc(
            "/corpus/spindle.md",
            "spindle",
            "# Spindle Assembly\n\n## Bearing preload\n\nTorque to 42 Nm and recheck.\n",
        )]);
        let r = idx.search("preload", &[], &SearchOptions::default());
        assert_eq!(r.hits.len(), 1, "{:?}", r.hits);
        assert!(r.hits[0].citation.contains("Bearing preload"));
    }

    #[test]
    fn lexical_channel_ranks_the_section_that_actually_says_it() {
        let idx = index(vec![
            doc(
                "/corpus/a.md",
                "a",
                "# Overview\n\nGeneral maintenance notes for the plant.\n",
            ),
            doc(
                "/corpus/b.md",
                "b",
                "# Fault log\n\nError E4412 raised on line two; bearing replaced.\n",
            ),
        ]);
        let r = idx.search("E4412", &[], &SearchOptions::default());
        assert!(!r.hits.is_empty());
        assert!(r.hits[0].path.ends_with("b.md"), "{:?}", r.hits[0]);
        // No vectors were supplied, so a semantic request must degrade rather
        // than return an order that only looks ranked.
        let r2 = idx.search(
            "E4412",
            &[],
            &SearchOptions {
                mode: SearchMode::Semantic,
                ..Default::default()
            },
        );
        assert_eq!(r2.mode, SearchMode::Lexical);
    }

    #[test]
    fn quoted_phrase_excludes_sections_missing_it_verbatim() {
        let idx = index(vec![
            doc("/corpus/a.md", "a", "# One\n\nThe bearing was seized.\n"),
            doc(
                "/corpus/b.md",
                "b",
                "# Two\n\nA seized bearing was found on the spindle.\n",
            ),
        ]);
        let r = idx.search("\"seized bearing\"", &[], &SearchOptions::default());
        assert_eq!(r.hits.len(), 1, "{:?}", r.hits);
        assert!(r.hits[0].path.ends_with("b.md"));
    }

    #[test]
    fn a_quoted_phrase_marks_as_one_span_not_as_its_words() {
        // The phrase and each of its words match at the same offset. Whichever
        // is listed first wins in the renderer, so the phrase has to be.
        let (text, marks) = snippet(
            "Three parts scrapped. No fault found on the fourth.",
            &["no".into(), "fault".into(), "found".into()],
            &["no fault found".into()],
            120,
        );
        let first = marks.first().expect("a mark");
        let got: String = text.chars().skip(first[0]).take(first[1]).collect();
        assert_eq!(got, "No fault found", "{marks:?}");
    }

    #[test]
    fn snippet_marks_whole_words_only() {
        let (text, marks) = snippet(
            "The boiler ran hot; the oil level was low.",
            &["oil".to_string()],
            &[],
            80,
        );
        assert_eq!(marks.len(), 1, "boiler must not match 'oil': {text:?}");
        let start = marks[0][0];
        let got: String = text.chars().skip(start).take(marks[0][1]).collect();
        assert_eq!(got, "oil");
    }

    #[test]
    fn snippet_offsets_are_character_based_across_accents() {
        // Byte offsets land mid-codepoint the first time an accented word
        // precedes the match, which in an Italian corpus is immediately.
        let body = "Però il cuscinetto è grippato sul mandrino.";
        let (text, marks) = snippet(body, &["grippato".to_string()], &[], 200);
        assert_eq!(marks.len(), 1);
        let got: String = text.chars().skip(marks[0][0]).take(marks[0][1]).collect();
        assert_eq!(got, "grippato");
    }

    #[test]
    fn snippet_window_prefers_the_densest_region() {
        let mut body = "filler ".repeat(60);
        body.push_str("the pump seal failed and the pump was replaced");
        let (text, marks) = snippet(&body, &["pump".to_string()], &[], 60);
        assert!(text.starts_with('…'), "expected a shifted window: {text:?}");
        assert!(marks.len() >= 2, "{marks:?}");
        let got: String = text.chars().skip(marks[0][0]).take(marks[0][1]).collect();
        assert_eq!(got, "pump");
    }

    #[test]
    fn facets_count_the_whole_match_set_not_the_returned_page() {
        let docs: Vec<IngestedDoc> = (0..5)
            .map(|i| {
                doc(
                    &format!("/corpus/log{i}.md"),
                    "log",
                    "# Entry\n\nbearing replaced\n",
                )
            })
            .collect();
        let idx = index(docs);
        let r = idx.search(
            "bearing",
            &[],
            &SearchOptions {
                top_k: 2,
                ..Default::default()
            },
        );
        assert_eq!(r.hits.len(), 2);
        assert_eq!(r.matched, 5);
        assert_eq!(r.facets.file.len(), 5, "facets must describe all matches");
    }

    #[test]
    fn filters_scope_the_search() {
        let idx = index(vec![
            doc("/corpus/plant/a.md", "a", "# One\n\nbearing replaced\n"),
            doc("/corpus/office/b.md", "b", "# Two\n\nbearing replaced\n"),
        ]);
        let r = idx.search(
            "bearing",
            &[],
            &SearchOptions {
                path_contains: Some("plant".into()),
                ..Default::default()
            },
        );
        assert_eq!(r.hits.len(), 1);
        assert!(r.hits[0].path.contains("plant"));
    }

    #[test]
    fn hybrid_fuses_both_channels() {
        // Two sections: one wins on terms, the other on the vector. With a
        // semantic-leaning fusion the vector winner must come first, but the
        // term winner must still be returned — that is the whole point of
        // running both channels.
        let mut idx = index(vec![
            doc("/corpus/a.md", "a", "# A\n\nbearing bearing bearing\n"),
            doc("/corpus/b.md", "b", "# B\n\nunrelated prose\n"),
        ]);
        idx.semantic = true;
        for s in idx.sections.iter_mut() {
            s.embedding = if s.path.ends_with("b.md") {
                vec![1.0, 0.0]
            } else {
                vec![0.0, 1.0]
            };
        }
        let r = idx.search("bearing", &[1.0, 0.0], &SearchOptions::default());
        assert_eq!(r.hits.len(), 2, "{:?}", r.hits);
        assert!(r.hits[0].path.ends_with("b.md"), "{:?}", r.hits);
        assert!(r.hits[0].semantic > r.hits[1].semantic);
        assert!(r.hits[1].lexical > r.hits[0].lexical);
    }

    #[test]
    fn empty_index_returns_nothing_without_panicking() {
        let idx = index(Vec::new());
        let r = idx.search("anything", &[], &SearchOptions::default());
        assert!(r.hits.is_empty());
        assert_eq!(r.matched, 0);
        assert_eq!(r.indexed_sections, 0);
    }
}
