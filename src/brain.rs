//! Phase 10 — the offline document brain ("brain multiplier" demo wedge).
//!
//! Pairs large documents with the engine's one embedding space and a small
//! statistical language model — an n-gram table built **from the corpus
//! itself** (the Qwen-3-style ngram-table floor: deterministic, model-free,
//! fully offline) — so a question over hundreds of pages is answered by the
//! real engine with verbatim citations even when no LLM endpoint exists.
//!
//! When a local small model IS reachable (Ollama floor: Gemma / LFM / Qwen),
//! the web layer routes the same retrieval through it and the reply's verbatim
//! quote is checked by the [`crate::grounding`] gate — the small model speaks,
//! the n-gram table and the grounding gate decide whether it is allowed to.
//!
//! Design honesty: the extractive path never invents a word — every answer is
//! a span of the source text, scored by meaning (embedding cosine) and
//! fluency (corpus n-gram log-probability). The face-off path never blends
//! contradictory documents away: disagreement is surfaced, not merged.

use crate::embed::VectorEmbed;
use crate::grounding;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

/// Cosine over one embedding space (same definition the content index uses).
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    crate::content::cosine(a, b)
}

// ── N-gram table — the corpus's own language-model floor ─────────────────────

/// Word-level n-gram counts (n = 1..3) built from the ingested corpus.
///
/// This is the demo's "small statistical AI": no weights, no network, a few
/// hundred KB of tables. Two uses, both visible on the page:
/// - **fluency floor** — mean log-probability of a candidate span under the
///   corpus, used to prefer ordinary-sounding sentences over fragment noise;
/// - **the honest AI claim** — the demo runs with zero inference weights when
///   no small model is present, and says so on screen.
#[derive(Debug, Clone, Default)]
pub struct NgramTable {
    uni: HashMap<String, u32>,
    bi: HashMap<(String, String), u32>,
    tri: HashMap<(String, String, String), u32>,
    total_words: u64,
}

impl NgramTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fold one document into the tables.
    pub fn push(&mut self, text: &str) {
        let toks = grounding_tokens(text);
        for (i, t) in toks.iter().enumerate() {
            *self.uni.entry(t.clone()).or_insert(0) += 1;
            self.total_words += 1;
            if i + 1 < toks.len() {
                *self
                    .bi
                    .entry((t.clone(), toks[i + 1].clone()))
                    .or_insert(0) += 1;
            }
            if i + 2 < toks.len() {
                *self
                    .tri
                    .entry((t.clone(), toks[i + 1].clone(), toks[i + 2].clone()))
                    .or_insert(0) += 1;
            }
        }
    }

    /// Mean trigram log-probability of a span under the corpus (add-one
    /// smoothed). Higher = more ordinary for this corpus's grammar.
    pub fn mean_logprob(&self, text: &str) -> f32 {
        let toks = grounding_tokens(text);
        if toks.len() < 3 || self.total_words == 0 {
            return -6.0; // the unobserved floor, not a penalty myth
        }
        let mut sum = 0.0f32;
        let mut n = 0u32;
        for w in toks.windows(3) {
            let c_tri = *self
                .tri
                .get(&(w[0].clone(), w[1].clone(), w[2].clone()))
                .unwrap_or(&0) as f32;
            let c_bi = *self
                .bi
                .get(&(w[0].clone(), w[1].clone()))
                .unwrap_or(&0) as f32;
            // add-one over the trigram continuation of the bigram context
            let p = (c_tri + 1.0) / (c_bi + self.uni.len() as f32);
            sum += p.ln();
            n += 1;
        }
        if n == 0 {
            -6.0
        } else {
            sum / n as f32
        }
    }

    /// Table size in distinct n-grams — the "AI is offline and explainable"
    /// number shown on the page.
    pub fn size(&self) -> usize {
        self.uni.len() + self.bi.len() + self.tri.len()
    }

    pub fn words(&self) -> u64 {
        self.total_words
    }
}

/// Same normalization the grounding gate uses — lowercase, alphanumeric runs.
/// Kept in sync deliberately: a quote the gate would accept is a quote this
/// module's scoring saw, so demo numbers and gate verdicts never disagree.
fn grounding_tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

fn token_match(needle: &str, haystack_tokens: &[String]) -> bool {
    let n = grounding_tokens(needle);
    !n.is_empty() && n.iter().all(|t| haystack_tokens.contains(t))
}

/// Fraction of `needle`'s tokens present in `hay` — the topical-overlap input
/// to the contradiction heuristic.
fn overlap(needle: &str, hay: &str) -> f32 {
    let n = grounding_tokens(needle);
    if n.is_empty() {
        return 0.0;
    }
    let h = grounding_tokens(hay);
    let present = n.iter().filter(|t| h.contains(t)).count();
    present as f32 / n.len() as f32
}

/// Negated-obligation markers: "X must not", "è vietato", "shall never".
/// One side affirming what the other forbids is the face-off's money shot.
const NEGATION_MARKERS: &[&str] = &[
    "not ", "no ", "non ", "mai ", "vietato", "divieto", "proibito", "must not", "shall not",
    "non può", "non deve",
];

fn negation_hits(text: &str) -> usize {
    let low = text.to_lowercase();
    NEGATION_MARKERS
        .iter()
        .filter(|m| low.contains(m.trim_end()))
        .count()
}

/// First number (with any unit-ish letters) in a text — differing values on
/// the same subject is the second contradiction tell.
fn leading_number(text: &str) -> Option<String> {
    let toks: Vec<&str> = text.split_whitespace().collect();
    toks.iter()
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()))
        .find(|t| t.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))
        .map(|t| t.chars().take_while(|c| c.is_alphanumeric()).collect())
}

// ── The book — chunked large documents in one embedding space ────────────────

/// One retrieval unit: ~a paragraph. Carries its verbatim sentences so an
/// extractive answer is always a real span of the source.
#[derive(Debug, Clone)]
pub struct BrainChunk {
    pub doc: usize,
    pub doc_name: String,
    pub heading: String,
    pub text: String,
    pub sentences: Vec<String>,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BrainDocInfo {
    pub name: String,
    pub chars: usize,
    pub chunks: usize,
}

/// The in-memory book over the demo's large documents.
#[derive(Debug, Default)]
pub struct BrainBook {
    pub docs: Vec<BrainDocInfo>,
    pub chunks: Vec<BrainChunk>,
    pub table: NgramTable,
    pub embedder_kind: String,
}

/// Target chunk size in chars — paragraph-shaped, small-model-context-sized.
const CHUNK_CHARS: usize = 900;

impl BrainBook {
    /// Build from `(name, text)` pairs, chunking on paragraphs and embedding
    /// every chunk in the engine's one embedding space. Callers run this
    /// inside `spawn_blocking` so the async runtime never blocks on it.
    pub fn build(
        docs: &[(String, String)],
        embedder: &dyn VectorEmbed,
        embedder_kind: &str,
    ) -> Self {
        let mut book = BrainBook {
            embedder_kind: embedder_kind.to_string(),
            ..Default::default()
        };
        for (name, body) in docs {
            let chars = body.chars().count();
            let mut doc_chunks: Vec<BrainChunk> = Vec::new();
            for piece in chunk_document(body) {
                let (heading, text) = split_heading(&piece);
                let sentences = split_sentences(&text);
                doc_chunks.push(BrainChunk {
                    doc: book.docs.len(),
                    doc_name: name.clone(),
                    heading,
                    text,
                    sentences,
                    embedding: Vec::new(),
                });
            }
            let n = doc_chunks.len();
            book.docs.push(BrainDocInfo {
                name: name.clone(),
                chars,
                chunks: n,
            });
            book.chunks.extend(doc_chunks);
        }
        let refs: Vec<&str> = book.chunks.iter().map(|c| c.text.as_str()).collect();
        let embs = embedder.embed_batch(&refs);
        for (c, e) in book.chunks.iter_mut().zip(embs) {
            c.embedding = e;
        }
        // The language-model floor is the corpus itself.
        for (_, body) in docs {
            book.table.push(body);
        }
        book
    }

    /// Rank chunks for a question.
    pub fn retrieve(&self, q_emb: &[f32], top_k: usize) -> Vec<&BrainChunk> {
        let mut scored: Vec<(f32, &BrainChunk)> = self
            .chunks
            .iter()
            .map(|c| {
                let s = if q_emb.is_empty() || c.embedding.is_empty() {
                    0.0
                } else {
                    cosine(q_emb, &c.embedding)
                };
                (s, c)
            })
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored
            .into_iter()
            .take(top_k.max(1))
            .map(|(_, c)| c)
            .collect()
    }

    /// Extractive grounded answer: pick the best *verbatim sentences* from the
    /// top chunks — meaning (cosine) + corpus fluency (n-gram log-prob).
    pub fn ask_extractive(&self, question: &str, q_emb: &[f32]) -> BrainReply {
        let top = self.retrieve(q_emb, 3);
        let q_tokens = grounding_tokens(question);
        let mut answers: Vec<BrainAnswer> = Vec::new();
        for (rank, c) in top.iter().enumerate() {
            let chunk_tokens = grounding_tokens(&c.text);
            let best = score_sentences(&c.sentences, &q_tokens, &chunk_tokens, &self.table);
            for (text, score) in best.into_iter().take(2) {
                answers.push(BrainAnswer {
                    text,
                    doc: c.doc_name.clone(),
                    section: c.heading.clone(),
                    score,
                    rank,
                });
            }
        }
        BrainReply {
            mode: "ngram-extractive",
            answers,
            citations: top
                .iter()
                .map(|c| Citation {
                    doc: c.doc_name.clone(),
                    section: c.heading.clone(),
                    snippet: c.text.chars().take(240).collect(),
                })
                .collect(),
        }
    }

    /// Document face-off: the strongest cross-document chunk pairs, classified
    /// agreement / contradiction. Contradictions are *reported*, never merged.
    pub fn faceoff(&self) -> FaceOffReport {
        let mut pairs: Vec<FaceOffPair> = Vec::new();
        for (i, a) in self.chunks.iter().enumerate() {
            for b in self.chunks.iter().skip(i + 1) {
                if a.doc == b.doc || a.embedding.is_empty() || b.embedding.is_empty() {
                    continue;
                }
                let sim = cosine(&a.embedding, &b.embedding);
                if sim < 0.42 {
                    continue; // unrelated — the demo only shows real tension
                }
                let ov = overlap(&a.text, &b.text);
                let na = negation_hits(&a.text);
                let nb = negation_hits(&b.text);
                let num_a = leading_number(&a.text);
                let num_b = leading_number(&b.text);
                let number_clash =
                    matches!((&num_a, &num_b), (Some(x), Some(y)) if x != y);
                let kind = if (ov >= 0.5 && na != nb) || (ov >= 0.5 && number_clash) {
                    PairKind::Contradiction
                } else {
                    PairKind::Agreement
                };
                pairs.push(FaceOffPair {
                    kind,
                    similarity: sim,
                    overlap: ov,
                    a: side_quote(a),
                    b: side_quote(b),
                    note: match kind {
                        PairKind::Contradiction if number_clash => format!(
                            "same subject, different values ({} vs {})",
                            num_a.unwrap_or_default(),
                            num_b.unwrap_or_default()
                        ),
                        PairKind::Contradiction => {
                            "one side forbids what the other requires".to_string()
                        }
                        PairKind::Agreement => "aligned provisions".to_string(),
                    },
                });
            }
        }
        // Keep the demo legible: contradictions first, then by similarity.
        pairs.sort_by(|x, y| {
            let kx = if x.kind == PairKind::Contradiction { 1 } else { 0 };
            let ky = if y.kind == PairKind::Contradiction { 1 } else { 0 };
            ky.cmp(&kx).then_with(|| {
                y.similarity
                    .partial_cmp(&x.similarity)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        pairs.truncate(6);
        let contradictions = pairs
            .iter()
            .filter(|p| p.kind == PairKind::Contradiction)
            .count();
        let agreements = pairs.len() - contradictions;
        FaceOffReport {
            contradictions,
            agreements,
            pairs,
        }
    }
}

/// A verbatim answer span scored from the source text itself.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BrainAnswer {
    pub text: String,
    pub doc: String,
    pub section: String,
    pub score: f32,
    pub rank: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Citation {
    pub doc: String,
    pub section: String,
    pub snippet: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BrainReply {
    pub mode: &'static str,
    pub answers: Vec<BrainAnswer>,
    pub citations: Vec<Citation>,
}

/// Sentence score: topical coverage + corpus fluency. Weights are visible
/// constants so the demo can say exactly how it scored.
fn score_sentences(
    sentences: &[String],
    q_tokens: &[String],
    chunk_tokens: &[String],
    table: &NgramTable,
) -> Vec<(String, f32)> {
    let mut scored: Vec<(String, f32)> = sentences
        .iter()
        .filter(|s| s.chars().count() > 40) // skip headings/fragments
        .map(|s| {
            let cov = if q_tokens.is_empty() {
                0.0
            } else {
                let s_tokens = grounding_tokens(s);
                let p = q_tokens.iter().filter(|t| s_tokens.contains(t)).count();
                p as f32 / q_tokens.len() as f32
            };
            let chunk_cov = if q_tokens.is_empty() {
                0.0
            } else {
                let p = q_tokens.iter().filter(|t| chunk_tokens.contains(t)).count();
                p as f32 / q_tokens.len() as f32
            };
            let flu = table.mean_logprob(s).max(-9.0); // floored so fluency can't dominate
            (s.clone(), (cov * 0.65 + chunk_cov * 0.35) * 0.9 + (flu + 9.0) * 0.01)
        })
        .filter(|(_, s)| *s > 0.0)
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PairKind {
    Agreement,
    Contradiction,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SideQuote {
    pub doc: String,
    pub section: String,
    pub quote: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FaceOffPair {
    pub kind: PairKind,
    pub similarity: f32,
    pub overlap: f32,
    pub a: SideQuote,
    pub b: SideQuote,
    pub note: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FaceOffReport {
    pub contradictions: usize,
    pub agreements: usize,
    pub pairs: Vec<FaceOffPair>,
}

fn side_quote(c: &BrainChunk) -> SideQuote {
    let quote = c
        .sentences
        .iter()
        .find(|s| s.chars().count() > 60)
        .cloned()
        .unwrap_or_else(|| c.text.chars().take(200).collect());
    SideQuote {
        doc: c.doc_name.clone(),
        section: c.heading.clone(),
        quote,
    }
}

// ── Chunking ─────────────────────────────────────────────────────────────────

fn chunk_document(body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for para in body.split("\n\n") {
        let para = para.trim();
        if para.is_empty() {
            continue;
        }
        if !cur.is_empty() && cur.chars().count() + para.chars().count() > CHUNK_CHARS {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push_str("\n\n");
        }
        cur.push_str(para);
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

fn split_heading(piece: &str) -> String {
    piece
        .lines()
        .next()
        .unwrap_or("")
        .trim_start_matches('#')
        .trim()
        .chars()
        .take(80)
        .collect()
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        cur.push(c);
        if matches!(c, '.' | '!' | '?' | ';') && cur.chars().count() > 25 {
            out.push(cur.trim().to_string());
            cur.clear();
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

// ── Web-layer static book (demo state; a restart re-seeds in ~1s) ────────────

static BOOK: OnceLock<RwLock<Option<Arc<BrainBook>>>> = OnceLock::new();

fn book_cell() -> &'static RwLock<Option<Arc<BrainBook>>> {
    BOOK.get_or_init(|| RwLock::new(None))
}

/// Replace the demo book (web seed handler).
pub fn store_book(book: BrainBook) -> Arc<BrainBook> {
    let arc = Arc::new(book);
    if let Ok(mut w) = book_cell().write() {
        *w = Some(arc.clone());
    }
    arc
}

pub fn current_book() -> Option<Arc<BrainBook>> {
    book_cell().read().ok().and_then(|b| b.clone())
}

/// LLM-path grounding: verify the model's claimed verbatim quote against the
/// retrieved chunks via the same token-match the grounding gate enforces.
pub fn ground_quote(quote: &str, chunks: &[&BrainChunk]) -> grounding::Verdict {
    if quote.trim().is_empty() {
        return grounding::Verdict::Unverified;
    }
    for c in chunks {
        let toks = grounding_tokens(&c.text);
        if token_match(quote, &toks) {
            return grounding::Verdict::Grounded;
        }
    }
    grounding::Verdict::Unverified
}
