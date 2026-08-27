//! Phase 11 — corpus-level comprehension: community summaries + global query.
//!
//! Top-k retrieval structurally cannot answer "what are the recurring themes
//! across this corpus": no single node holds the answer, so no similarity
//! ranking can surface it. Community *summaries* can — rank and read the
//! summaries instead of the nodes and a question about the whole corpus costs
//! a bounded number of tokens no matter how large the graph is.
//!
//! The default path is deterministic and offline: keywords come from TF-IDF
//! over member labels, facets from majority vote, digests from a template —
//! never from a model. That keeps the air-gapped guarantee intact. When a
//! semantic provider is configured it may *rewrite* a digest, but membership,
//! facets and keywords are computed here and are invariant to whatever the
//! model does or fails to do (the two-pass coverage contract `wiki_gen`
//! already applies to outline naming, applied here to summaries).
//!
//! Read-only invariant, inherited from [`crate::communities`]: nothing in this
//! module mutates its inputs; every function returns fresh owned data.

use crate::communities::{self, EdgeList};
use crate::core::PhysisCore;
use crate::ledger::CausalLedger;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Distinctive terms kept per community.
pub const KEYWORDS_PER_COMMUNITY: usize = 6;

/// Member labels quoted in a digest as representatives of the community.
pub const REPRESENTATIVES_PER_COMMUNITY: usize = 3;

/// Shortest term that can become a keyword. Two-letter tokens are almost
/// always noise ("di", "il", "of", "to") and the stopword list cannot be
/// exhaustive across the two languages this corpus mixes.
const MIN_TERM_LEN: usize = 3;

/// English + Italian function words. The corpus mixes both (Italian shift
/// reports next to English agent logs), so one list covers both rather than
/// letting language detection decide what to strip.
const STOPWORDS: &[&str] = &[
    // English
    "and", "are", "but", "for", "from", "had", "has", "have", "her", "his", "not", "our", "out",
    "she", "that", "the", "their", "them", "then", "there", "these", "they", "this", "was", "were",
    "what", "when", "which", "who", "will", "with", "you", "your", "its", "into", "than", "each",
    // Italian
    "che", "chi", "con", "come", "dei", "del", "della", "delle", "degli", "dal", "dalla", "gli",
    "hanno", "non", "per", "più", "sono", "sul", "sulla", "una", "uno", "essere", "come", "questo",
    "questa", "quello", "nel", "nella", "alla", "allo", "dove", "quando",
];

/// What one member of a community contributes to its summary.
///
/// Everything past `label` is optional: a graph that has never been through
/// the datawork pipeline still summarizes, it just yields no facets.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MemberFacts {
    /// The node label — the only required field, and the text keywords come from.
    pub label: String,
    /// Semiotic cell pinned to this node: `(domain, mode)`.
    #[serde(default)]
    pub cell: Option<(String, String)>,
    /// Asserted outcome in `[-1, 1]`; `None` = nobody has judged this yet.
    #[serde(default)]
    pub verdict: Option<f32>,
    /// Space facet from the file address (`SpaceRef::place`).
    #[serde(default)]
    pub place: Option<String>,
    /// Time facet from the file address (`TimeRef::period`).
    #[serde(default)]
    pub period: Option<String>,
    /// Efficient-cause facet from the ledger (`Mechanism`).
    #[serde(default)]
    pub mechanism: Option<String>,
}

impl MemberFacts {
    /// A bare member with no facets — the minimum a caller must supply.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ..Default::default()
        }
    }
}

/// One community, summarized. This is the unit a global question is answered
/// over: small, self-describing, and cheap enough that hundreds fit a budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommunitySummary {
    pub id: u32,
    /// Short human title, derived from the dominant cell and top keywords.
    pub title: String,
    pub size: usize,
    /// Intra-cluster edge density, carried through from detection.
    pub cohesion: f32,
    /// Distinctive terms — TF-IDF against the other communities, so a term
    /// every community shares is never a keyword.
    pub keywords: Vec<String>,
    /// Majority `DOMAIN/MODE`, when the members carry cells at all.
    pub dominant_cell: Option<String>,
    pub dominant_place: Option<String>,
    pub dominant_period: Option<String>,
    pub dominant_mechanism: Option<String>,
    /// Member labels that best cover the community's own keywords.
    pub representatives: Vec<String>,
    /// Members carrying a negative verdict — "we already failed here".
    pub failures: usize,
    /// Members carrying a positive verdict.
    pub wins: usize,
    /// One-paragraph deterministic digest. This is the text that gets embedded
    /// and ranked for a global question.
    pub digest: String,
    /// Every member label, sorted. Coverage is guaranteed here: no summary
    /// step may drop a member.
    pub members: Vec<String>,
}

impl CommunitySummary {
    /// The text handed to the embedder and to the synthesizer. Title and
    /// keywords are repeated into it deliberately: the digest is the *only*
    /// string that gets embedded, so anything not in it cannot be matched.
    pub fn retrieval_text(&self) -> String {
        format!("{}. {}", self.title, self.digest)
    }
}

/// Lowercase alphanumeric terms of a label, stopwords and short tokens
/// dropped, deduplicated. Deduplication makes term frequency count *members*
/// mentioning a term rather than raw repetitions, so one verbose label cannot
/// dominate a community's keywords.
fn terms_of(label: &str) -> BTreeSet<String> {
    label
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .filter(|w| w.chars().count() >= MIN_TERM_LEN)
        .filter(|w| !STOPWORDS.contains(&w.as_str()))
        // A pure-digit token ("2026", "31") is never a theme.
        .filter(|w| !w.chars().all(|c| c.is_ascii_digit()))
        .collect()
}

/// Majority value with a deterministic tie-break: highest count wins, ties go
/// to the lexicographically smallest value. Returns `None` when no member
/// carried the facet at all — an absent facet is reported as absent rather
/// than guessed.
fn dominant<I: IntoIterator<Item = String>>(values: I) -> Option<String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for v in values {
        *counts.entry(v).or_default() += 1;
    }
    counts
        .into_iter()
        // BTreeMap iterates keys ascending, so max_by_key over count alone
        // would keep the LAST maximum. Compare on (count, Reverse(key)) so the
        // smallest key wins a tie.
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|(v, _)| v)
}

/// Summarize every community in one pass.
///
/// Takes all communities together because keyword distinctiveness is a
/// *corpus-level* property: a term is only interesting for a community if the
/// other communities do not also use it. Input is `(id, cohesion, members)`.
///
/// Output is sorted largest community first (ties on id), matching the
/// existing `/api/v1/communities` rollup so clients keep one ordering
/// convention.
pub fn summarize(communities: &[(u32, f32, Vec<MemberFacts>)]) -> Vec<CommunitySummary> {
    if communities.is_empty() {
        return Vec::new();
    }

    // Per-community term frequency (in members) and corpus-wide document
    // frequency (in communities).
    let mut per_community: Vec<BTreeMap<String, usize>> = Vec::with_capacity(communities.len());
    let mut doc_freq: BTreeMap<String, usize> = BTreeMap::new();
    for (_, _, members) in communities {
        let mut tf: BTreeMap<String, usize> = BTreeMap::new();
        for m in members {
            for t in terms_of(&m.label) {
                *tf.entry(t).or_default() += 1;
            }
        }
        for t in tf.keys() {
            *doc_freq.entry(t.clone()).or_default() += 1;
        }
        per_community.push(tf);
    }

    let n_communities = communities.len() as f32;
    let mut out: Vec<CommunitySummary> = Vec::with_capacity(communities.len());

    for (i, (id, cohesion, members)) in communities.iter().enumerate() {
        // TF-IDF. `ln(1 + N/df)` keeps a term that appears in every community
        // at a small but non-zero weight, so a single-community corpus still
        // produces keywords instead of an empty list.
        let mut scored: Vec<(String, f32)> = per_community[i]
            .iter()
            .map(|(term, &tf)| {
                let df = doc_freq.get(term).copied().unwrap_or(1) as f32;
                (term.clone(), tf as f32 * (1.0 + n_communities / df).ln())
            })
            .collect();
        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        let keywords: Vec<String> = scored
            .into_iter()
            .take(KEYWORDS_PER_COMMUNITY)
            .map(|(t, _)| t)
            .collect();

        let dominant_cell = dominant(
            members
                .iter()
                .filter_map(|m| m.cell.as_ref().map(|(d, md)| format!("{d}/{md}"))),
        );
        let dominant_place = dominant(members.iter().filter_map(|m| m.place.clone()));
        let dominant_period = dominant(members.iter().filter_map(|m| m.period.clone()));
        let dominant_mechanism = dominant(members.iter().filter_map(|m| m.mechanism.clone()));

        let failures = members
            .iter()
            .filter(|m| m.verdict.is_some_and(|v| v < 0.0))
            .count();
        let wins = members
            .iter()
            .filter(|m| m.verdict.is_some_and(|v| v > 0.0))
            .count();

        // Representative = the member whose own terms cover the most community
        // keywords. Ties break lexicographically so the choice is stable.
        let kw_set: BTreeSet<&str> = keywords.iter().map(|s| s.as_str()).collect();
        let mut ranked: Vec<(usize, &str)> = members
            .iter()
            .map(|m| {
                let hits = terms_of(&m.label)
                    .iter()
                    .filter(|t| kw_set.contains(t.as_str()))
                    .count();
                (hits, m.label.as_str())
            })
            .collect();
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        let representatives: Vec<String> = ranked
            .into_iter()
            .take(REPRESENTATIVES_PER_COMMUNITY)
            .map(|(_, l)| l.to_string())
            .collect();

        let mut member_labels: Vec<String> = members.iter().map(|m| m.label.clone()).collect();
        member_labels.sort();

        let title = build_title(*id, dominant_cell.as_deref(), &keywords);
        let digest = build_digest(
            members.len(),
            *cohesion,
            dominant_cell.as_deref(),
            dominant_place.as_deref(),
            dominant_period.as_deref(),
            dominant_mechanism.as_deref(),
            &keywords,
            &representatives,
            failures,
            wins,
        );

        out.push(CommunitySummary {
            id: *id,
            title,
            size: members.len(),
            cohesion: *cohesion,
            keywords,
            dominant_cell,
            dominant_place,
            dominant_period,
            dominant_mechanism,
            representatives,
            failures,
            wins,
            digest,
            members: member_labels,
        });
    }

    out.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.id.cmp(&b.id)));
    out
}

/// Gather member facts and label-space edges from a tenant's coherence graph and
/// its causal ledger — the input [`build_summaries`] needs.
///
/// Facets come from wherever the graph actually records them: the semiotic cell
/// from the node's pin, the verdict from its asserted outcome, and the mechanism
/// from the causal ledger when an event names this label (or node id) as its
/// object. Place and period live on `FileAddress`, which belongs to file nodes
/// rather than coherence nodes, so they stay `None` here — [`MemberFacts`] omits
/// absent facets from the digest rather than guessing them.
///
/// This is the graph-side half of the corpus pipeline, shared by the web
/// handlers and the `physis-pro themes` CLI so both summarize the same data
/// under the same rules. The read-only invariant holds: nothing here mutates the
/// core or the ledger.
pub fn gather_facts(
    core: &PhysisCore,
    ledger: &CausalLedger,
) -> (Vec<MemberFacts>, EdgeList) {
    // Latest mechanism per artifact. The ledger is append-only and ordered by
    // seq, so a later event simply overwrites an earlier one here.
    let mut mechanism_of: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for e in ledger.events() {
        mechanism_of.insert(e.object.clone(), e.mechanism.as_str().to_string());
    }

    let names: std::collections::HashMap<String, String> = core
        .nodes
        .values()
        .filter_map(|n| n.label.as_ref().map(|l| (n.id.clone(), l.clone())))
        .collect();

    let facts: Vec<MemberFacts> = core
        .nodes
        .values()
        .filter_map(|n| {
            let label = n.label.as_deref()?;
            if label.trim().is_empty() {
                return None;
            }
            Some(MemberFacts {
                label: label.to_string(),
                cell: n.cell_pin.clone(),
                verdict: n.asserted,
                place: None,
                period: None,
                mechanism: mechanism_of
                    .get(label)
                    .or_else(|| mechanism_of.get(&n.id))
                    .cloned(),
            })
        })
        .collect();

    let edges: EdgeList = core
        .resolved_edges()
        .into_iter()
        .filter_map(|(a, b, _w)| match (a, b) {
            (Some(a), Some(b)) if a != b => match (names.get(&a), names.get(&b)) {
                (Some(la), Some(lb)) if la != lb => Some((la.clone(), lb.clone())),
                _ => None,
            },
            _ => None,
        })
        .collect();

    (facts, edges)
}

/// Detect communities over `edges` and summarize them, carrying each member's
/// facets across.
///
/// Split from the summary math so the whole pipeline is unit-testable without an
/// HTTP stack or a persistence layer. Labeled nodes carrying no edge are
/// gathered into one singleton community at full cohesion — the same convention
/// the communities rollup uses, so a node is never invisible to corpus-level
/// questions just because nothing links to it.
pub fn build_summaries(
    facts: &[MemberFacts],
    edges: &EdgeList,
) -> Vec<CommunitySummary> {
    let report = communities::detect_communities(edges, 16);
    let by_label: std::collections::HashMap<&str, &MemberFacts> =
        facts.iter().map(|f| (f.label.as_str(), f)).collect();

    let mut input: Vec<(u32, f32, Vec<MemberFacts>)> = (0..report.n_communities())
        .map(|i| {
            let members: Vec<MemberFacts> = report
                .members(i as u32)
                .into_iter()
                .map(|label| {
                    by_label
                        .get(label)
                        .map(|f| (*f).clone())
                        // An edge endpoint with no node behind it still counts
                        // as a member: dropping it would break the coverage
                        // contract (`members` must list every member).
                        .unwrap_or_else(|| MemberFacts::new(label))
                })
                .collect();
            (i as u32, report.cohesion[i], members)
        })
        .collect();

    let connected: std::collections::HashSet<&str> =
        report.nodes.iter().map(|s| s.as_str()).collect();
    let singles: Vec<MemberFacts> = facts
        .iter()
        .filter(|f| !connected.contains(f.label.as_str()))
        .cloned()
        .collect();
    if !singles.is_empty() {
        input.push((input.len() as u32, 1.0, singles));
    }

    summarize(&input)
}

/// `DOMAIN/MODE — kw, kw, kw`, degrading to keywords alone, then to the id.
fn build_title(id: u32, cell: Option<&str>, keywords: &[String]) -> String {
    let head: String = keywords
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    match (cell, head.is_empty()) {
        (Some(c), false) => format!("{c} — {head}"),
        (Some(c), true) => c.to_string(),
        (None, false) => head,
        (None, true) => format!("Community {id}"),
    }
}

/// Deterministic digest. Every clause is omitted rather than filled with a
/// placeholder when its facet is absent, so the text never asserts more than
/// the graph knows.
#[allow(clippy::too_many_arguments)]
fn build_digest(
    size: usize,
    cohesion: f32,
    cell: Option<&str>,
    place: Option<&str>,
    period: Option<&str>,
    mechanism: Option<&str>,
    keywords: &[String],
    representatives: &[String],
    failures: usize,
    wins: usize,
) -> String {
    let mut s = format!(
        "{size} node{} at cohesion {cohesion:.2}.",
        if size == 1 { "" } else { "s" }
    );

    let mut where_parts: Vec<String> = Vec::new();
    if let Some(c) = cell {
        where_parts.push(format!("mostly {c}"));
    }
    if let Some(p) = place {
        where_parts.push(format!("at {p}"));
    }
    if let Some(p) = period {
        where_parts.push(format!("during {p}"));
    }
    if let Some(m) = mechanism {
        where_parts.push(format!("produced by {m}"));
    }
    if !where_parts.is_empty() {
        s.push(' ');
        s.push_str(&capitalize(&where_parts.join(", ")));
        s.push('.');
    }

    if !keywords.is_empty() {
        s.push_str(&format!(" Recurring terms: {}.", keywords.join(", ")));
    }

    if failures > 0 || wins > 0 {
        s.push_str(&format!(
            " Outcomes on record: {wins} positive, {failures} negative."
        ));
    }

    if !representatives.is_empty() {
        let quoted: Vec<String> = representatives.iter().map(|r| format!("\"{r}\"")).collect();
        s.push_str(&format!(" Representative entries: {}.", quoted.join("; ")));
    }

    s
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// One summary selected into a global answer's context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobalHit {
    /// 1-based citation number, matching the `[n]` markers in the prompt.
    pub n: usize,
    pub id: u32,
    pub title: String,
    pub score: f32,
    pub size: usize,
    pub tokens: usize,
}

/// Result of packing summaries under a hard token budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobalContext {
    pub hits: Vec<GlobalHit>,
    pub total_tokens: usize,
    pub budget: usize,
    /// True when at least one ranked summary did not fit.
    pub truncated: bool,
}

/// Greedily pack pre-ranked summaries under `budget`, mirroring
/// [`crate::rag::TokenFixedRetriever`]: the caller gets a context that is
/// guaranteed to fit regardless of corpus size.
///
/// `ranked` is `(index into summaries, score)` in descending score order —
/// scoring needs an embedder, so it stays out of this pure function.
/// A summary that does not fit is skipped, not truncated: half a digest is a
/// misleading citation. Packing continues so a small summary behind a large
/// one still makes it in.
pub fn pack_global(
    summaries: &[CommunitySummary],
    ranked: &[(usize, f32)],
    budget: usize,
    top_k: usize,
) -> GlobalContext {
    let mut hits = Vec::new();
    let mut total = 0usize;
    let mut truncated = false;

    for &(idx, score) in ranked {
        if hits.len() >= top_k {
            truncated = true;
            break;
        }
        let Some(s) = summaries.get(idx) else {
            continue;
        };
        let tokens = crate::rag::count_tokens(&s.retrieval_text());
        if total + tokens > budget {
            truncated = true;
            continue;
        }
        total += tokens;
        hits.push(GlobalHit {
            n: hits.len() + 1,
            id: s.id,
            title: s.title.clone(),
            score,
            size: s.size,
            tokens,
        });
    }

    GlobalContext {
        hits,
        total_tokens: total,
        budget,
        truncated,
    }
}

/// Grounding prompt for a global question. Pure, so the citation contract is
/// unit-testable without a live provider — same contract as
/// `web::search::ask_prompt`, but the numbered entries are whole communities
/// rather than single nodes, and the model is told so: a global answer is
/// about distribution and recurrence, not about one document.
pub fn global_prompt(
    question: &str,
    summaries: &[CommunitySummary],
    context: &GlobalContext,
) -> String {
    let mut p = String::from(
        "Answer the question using ONLY the numbered community summaries below. \
         Each entry describes a CLUSTER of related entries in a knowledge graph, \
         not a single document. Answer at that level: name the recurring themes, \
         say how large each is, and note where they concentrate. Cite communities \
         as [n]. If the summaries do not contain the answer, say so plainly \
         instead of guessing.\n\n",
    );
    for hit in &context.hits {
        if let Some(s) = summaries.iter().find(|s| s.id == hit.id) {
            p.push_str(&format!("[{}] {}\n{}\n\n", hit.n, s.title, s.digest));
        }
    }
    p.push_str("Question: ");
    p.push_str(question);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(label: &str) -> MemberFacts {
        MemberFacts::new(label)
    }

    fn celled(label: &str, domain: &str, mode: &str) -> MemberFacts {
        MemberFacts {
            label: label.to_string(),
            cell: Some((domain.to_string(), mode.to_string())),
            ..Default::default()
        }
    }

    #[test]
    fn keywords_are_distinctive_not_merely_frequent() {
        // "report" appears in BOTH communities, so it must lose to the terms
        // that appear in only one — that is the whole point of the IDF term.
        let communities = vec![
            (
                0u32,
                0.9f32,
                vec![
                    m("spindle vibration report"),
                    m("spindle bearing report"),
                    m("spindle temperature report"),
                ],
            ),
            (
                1u32,
                0.8f32,
                vec![
                    m("invoice mismatch report"),
                    m("invoice duplicate report"),
                    m("invoice vendor report"),
                ],
            ),
        ];
        let out = summarize(&communities);
        assert_eq!(out.len(), 2);
        let first = out.iter().find(|c| c.id == 0).unwrap();
        assert_eq!(
            first.keywords.first().map(String::as_str),
            Some("spindle"),
            "distinctive term must outrank the shared one: {:?}",
            first.keywords
        );
        assert!(
            first.keywords.iter().position(|k| k == "report")
                > first.keywords.iter().position(|k| k == "spindle"),
            "shared term must rank below distinctive ones: {:?}",
            first.keywords
        );
    }

    #[test]
    fn single_community_still_gets_keywords() {
        // df == N for every term here; ln(1 + 1/1) > 0 keeps them alive.
        let out = summarize(&[(0, 1.0, vec![m("spindle vibration"), m("spindle bearing")])]);
        assert!(!out[0].keywords.is_empty(), "{:?}", out[0]);
        assert_eq!(out[0].keywords[0], "spindle");
    }

    #[test]
    fn coverage_every_member_survives_into_the_summary() {
        let members: Vec<MemberFacts> = (0..25)
            .map(|i| m(&format!("node number {i} alpha")))
            .collect();
        let out = summarize(&[(0, 0.5, members.clone())]);
        assert_eq!(out[0].size, 25);
        assert_eq!(out[0].members.len(), 25);
        for mem in &members {
            assert!(out[0].members.contains(&mem.label), "dropped {}", mem.label);
        }
    }

    #[test]
    fn dominant_facets_are_majority_with_lexicographic_tiebreak() {
        let out = summarize(&[(
            0,
            0.5,
            vec![
                celled("a alpha", "FABRICATE", "MAINTAIN"),
                celled("b alpha", "FABRICATE", "MAINTAIN"),
                celled("c alpha", "ADMINISTER", "RECORD"),
            ],
        )]);
        assert_eq!(out[0].dominant_cell.as_deref(), Some("FABRICATE/MAINTAIN"));

        // Perfect tie → smallest label wins, deterministically.
        let tied = summarize(&[(
            0,
            0.5,
            vec![
                celled("a alpha", "FABRICATE", "MAINTAIN"),
                celled("b alpha", "ADMINISTER", "RECORD"),
            ],
        )]);
        assert_eq!(tied[0].dominant_cell.as_deref(), Some("ADMINISTER/RECORD"));
    }

    #[test]
    fn absent_facets_stay_absent_never_guessed() {
        let out = summarize(&[(0, 0.5, vec![m("bare label alpha")])]);
        assert!(out[0].dominant_cell.is_none());
        assert!(out[0].dominant_place.is_none());
        assert!(out[0].dominant_period.is_none());
        assert!(out[0].dominant_mechanism.is_none());
        assert!(!out[0].digest.contains("mostly"), "{}", out[0].digest);
    }

    #[test]
    fn verdicts_are_tallied_into_the_digest() {
        let mut bad = m("failed approach alpha");
        bad.verdict = Some(-1.0);
        let mut good = m("worked approach alpha");
        good.verdict = Some(1.0);
        let neutral = m("untried approach alpha");
        let out = summarize(&[(0, 0.5, vec![bad, good, neutral])]);
        assert_eq!(out[0].failures, 1);
        assert_eq!(out[0].wins, 1);
        assert!(
            out[0].digest.contains("1 positive, 1 negative"),
            "{}",
            out[0].digest
        );
    }

    #[test]
    fn summaries_sort_largest_first() {
        let out = summarize(&[
            (0, 0.5, vec![m("only one alpha")]),
            (1, 0.5, vec![m("a beta"), m("b beta"), m("c beta")]),
            (2, 0.5, vec![m("x gamma"), m("y gamma")]),
        ]);
        let sizes: Vec<usize> = out.iter().map(|c| c.size).collect();
        assert_eq!(sizes, vec![3, 2, 1]);
    }

    #[test]
    fn empty_input_is_clean() {
        assert!(summarize(&[]).is_empty());
        let ctx = pack_global(&[], &[], 100, 5);
        assert_eq!(ctx.hits.len(), 0);
        assert!(!ctx.truncated);
    }

    #[test]
    fn deterministic_across_calls() {
        let communities = vec![
            (
                0u32,
                0.7f32,
                vec![m("spindle vibration"), m("spindle heat")],
            ),
            (1u32, 0.6f32, vec![m("invoice vendor"), m("invoice total")]),
        ];
        assert_eq!(summarize(&communities), summarize(&communities));
    }

    #[test]
    fn packing_respects_the_budget_and_flags_truncation() {
        let out = summarize(&[
            (
                0,
                0.9,
                vec![m("spindle vibration alpha"), m("spindle bearing alpha")],
            ),
            (
                1,
                0.8,
                vec![m("invoice mismatch beta"), m("invoice vendor beta")],
            ),
        ]);
        let ranked = vec![(0usize, 0.9f32), (1usize, 0.8f32)];

        let generous = pack_global(&out, &ranked, 10_000, 10);
        assert_eq!(generous.hits.len(), 2);
        assert!(!generous.truncated);
        assert!(generous.total_tokens <= generous.budget);

        let tight = pack_global(&out, &ranked, 1, 10);
        assert!(tight.hits.is_empty(), "nothing fits a 1-token budget");
        assert!(tight.truncated);

        let capped = pack_global(&out, &ranked, 10_000, 1);
        assert_eq!(capped.hits.len(), 1);
        assert!(capped.truncated);
    }

    #[test]
    fn citation_numbers_are_dense_and_one_based() {
        let out = summarize(&[
            (5, 0.9, vec![m("spindle alpha")]),
            (9, 0.8, vec![m("invoice beta")]),
        ]);
        let ranked = vec![(0usize, 0.9f32), (1usize, 0.8f32)];
        let ctx = pack_global(&out, &ranked, 10_000, 10);
        let ns: Vec<usize> = ctx.hits.iter().map(|h| h.n).collect();
        assert_eq!(
            ns,
            vec![1, 2],
            "citation numbers must not inherit community ids"
        );
    }

    #[test]
    fn prompt_carries_every_packed_entry_and_the_question() {
        let out = summarize(&[
            (0, 0.9, vec![m("spindle vibration alpha")]),
            (1, 0.8, vec![m("invoice mismatch beta")]),
        ]);
        let ranked = vec![(0usize, 0.9f32), (1usize, 0.8f32)];
        let ctx = pack_global(&out, &ranked, 10_000, 10);
        let p = global_prompt("what recurs?", &out, &ctx);
        assert!(p.contains("[1]"));
        assert!(p.contains("[2]"));
        assert!(p.contains("what recurs?"));
        assert!(
            p.contains("CLUSTER"),
            "must tell the model these are clusters"
        );
    }

    #[test]
    fn retrieval_text_includes_title_and_digest() {
        let out = summarize(&[(0, 0.9, vec![m("spindle vibration alpha")])]);
        let t = out[0].retrieval_text();
        assert!(t.contains(&out[0].title));
        assert!(t.contains(&out[0].digest));
    }

    #[test]
    fn digits_and_stopwords_never_become_keywords() {
        let out = summarize(&[(
            0,
            0.5,
            vec![
                m("the report of 2026 with the spindle"),
                m("the spindle and 2026"),
            ],
        )]);
        assert!(!out[0].keywords.iter().any(|k| k == "2026"));
        assert!(!out[0].keywords.iter().any(|k| k == "the"));
        assert!(out[0].keywords.iter().any(|k| k == "spindle"));
    }

    #[test]
    fn input_is_not_mutated_read_only_invariant() {
        let communities = vec![(0u32, 0.7f32, vec![m("spindle vibration")])];
        let before = communities.clone();
        let _ = summarize(&communities);
        assert_eq!(communities, before);
    }
}
