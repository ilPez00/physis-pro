// provenance: moved from physis-core src/nlq.rs (core HEAD d65858b)
// as part of the Core/Product split, 2026-09-14. One implementation; no fork.
//! Natural-language front door to the world layer — and the measurement that
//! says a cue parser is not enough to be one.
//!
//! ## The gap this addresses
//!
//! `physis-world` answers `locate`, `neighbors`, `history`, `changed`,
//! `compare`, `trace`, `why` and `retrieve`, and every answer prints the chain
//! it came from. But the caller has to already know which verb their question
//! is. That is the one place in this substrate where a person still has to
//! think in the schema.
//!
//! This module maps a sentence onto one of those verbs and a term. It does
//! **not** answer the question and it does not run anything. It returns a
//! [`Proposal`] — the verb it thinks was meant, the term it extracted, the cues
//! that fired, and the runner-up — for a person or a caller to accept. That
//! split is deliberate: a wrong parse that proposes costs a keystroke, a wrong
//! parse that executes costs a `copy` into the wrong file.
//!
//! ## What was measured, and what it refuted
//!
//! Two question sets, both scored as exact intent match against the nine verbs:
//!
//! | set | this parser | majority-intent null |
//! |---|---|---|
//! | `SET` — 48 questions written alongside the cue table | **1.000** (48/48) | 0.146 (7/48) |
//! | `HELD_OUT` — 16 questions phrased to avoid the cue vocabulary, written after the table was frozen | **0.062** (1/16) | 0.125 (2/16) |
//!
//! **On unseen phrasing the parser loses to a null that ignores the words.**
//! Fifteen of the sixteen held-out questions fired no cue at all and fell
//! through to `Retrieve`. The 1.000 on `SET` measures nothing but the fact that
//! the same hand wrote the questions and the cues; it is a self-consistency
//! check and it is reported here only so that it cannot be quoted as a result.
//!
//! So the hypothesis *"a deterministic cue table is a sufficient NL front door
//! for the world layer"* is **refuted**, by its own benchmark, at a null it was
//! supposed to beat easily.
//!
//! ## What survives the refutation
//!
//! Not the cue table — the **contract around it**. [`Proposal`] is what any
//! parser in this slot has to produce: a named verb, an extracted term, the
//! evidence for the choice, the runner-up that was nearly picked, and a
//! `fallback` flag that is true exactly when nothing was understood. A model
//! put in this slot is held to the same shape, which is what keeps it a
//! replaceable interpreter instead of the memory. The measurement above is the
//! floor it has to clear, and clearing 0.062 on `HELD_OUT` is not an
//! achievement — it is the minimum entry condition.
//!
//! The cue table stays in the tree as that floor, and as the thing an
//! LLM-backed parser gets compared against on the same two sets. It is not
//! wired into any binary, because a 0.062 parser must not be in a user's path.
//!
//! ## Why there is no model in here
//!
//! An LLM would parse these better — that is now measured, not assumed. It
//! would also make the parse unauditable unless it is made to fill in
//! [`Proposal::evidence`]. The layer boundary this project keeps is that the
//! model stays **outside** as a replaceable interpreter, so the next step is a
//! `ModelProvider`-backed parser scored on these same sets, not a bigger cue
//! table. Growing the table to cover `HELD_OUT` would be fitting the test.

use serde::{Deserialize, Serialize};

/// A world-layer verb — one of the commands `physis-world` already answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Intent {
    /// `inspect` — what does this world contain at all.
    Inspect,
    /// `locate <term>` — where does a thing sit.
    Locate,
    /// `neighbors <term>` — what is structurally near it.
    Neighbors,
    /// `history <term>` — every position mentioning it, in log order.
    History,
    /// `changed <term>` — positions whose derived transition is not persistence.
    Changed,
    /// `compare <a> <b>` — shared links and neighbourhood.
    Compare,
    /// `trace <position>` — the chain behind one position.
    Trace,
    /// `why <position>` — what a position follows from.
    Why,
    /// `retrieve <query...>` — linked arm and blind arm, side by side.
    Retrieve,
}

impl Intent {
    /// The `physis-world` subcommand this intent runs as.
    pub fn verb(self) -> &'static str {
        match self {
            Intent::Inspect => "inspect",
            Intent::Locate => "locate",
            Intent::Neighbors => "neighbors",
            Intent::History => "history",
            Intent::Changed => "changed",
            Intent::Compare => "compare",
            Intent::Trace => "trace",
            Intent::Why => "why",
            Intent::Retrieve => "retrieve",
        }
    }

    /// Every intent, in the order the scorer breaks ties.
    ///
    /// Ties break toward the *narrower* verb: `Retrieve` is last because it
    /// reads the whole world and is the honest fallback, so it must never win
    /// a tie against a verb that names a specific thing.
    pub fn all() -> &'static [Intent] {
        &[
            Intent::Compare,
            Intent::Changed,
            Intent::History,
            Intent::Neighbors,
            Intent::Trace,
            Intent::Why,
            Intent::Locate,
            Intent::Inspect,
            Intent::Retrieve,
        ]
    }
}

/// One cue that fired, and what it argued for. This is the parse's provenance:
/// every point in [`Proposal::score`] is attributable to one of these.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    /// The literal phrase found in the question.
    pub phrase: String,
    /// What it argued for.
    pub intent: Intent,
    /// How much it argued. Multi-word cues weigh more than single words
    /// because "what changed" is a far stronger signal than "changed".
    pub weight: i32,
}

/// What the parser thinks was asked — offered, never executed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Proposal {
    /// The question as given, verbatim.
    pub question: String,
    /// Best-scoring intent.
    pub intent: Intent,
    /// The extracted argument: a term, a path, or a log position.
    pub term: String,
    /// Second-best intent and its score — printed so an operator sees the
    /// choice that was nearly made rather than only the one that was.
    pub runner_up: Option<(Intent, i32)>,
    /// Winning score.
    pub score: i32,
    /// Cues that fired, for any intent. The parse's provenance.
    pub evidence: Vec<Cue>,
    /// True when nothing fired and [`Intent::Retrieve`] was chosen by default
    /// rather than by evidence. A caller **must** surface this; it is the
    /// difference between "you asked for search" and "I could not tell".
    pub fallback: bool,
}

impl Proposal {
    /// The command line this proposal would run. Rendering it is the point:
    /// the operator approves a concrete command, not an intention.
    pub fn command(&self) -> String {
        match self.intent {
            Intent::Inspect => "physis-world inspect".to_string(),
            _ => format!("physis-world {} {}", self.intent.verb(), self.term),
        }
    }

    /// Margin over the runner-up. Zero means the two intents tied and the
    /// tie-break in [`Intent::all`] decided it — the caller should ask.
    pub fn margin(&self) -> i32 {
        self.score - self.runner_up.map(|(_, s)| s).unwrap_or(0)
    }
}

/// Cue table. Ordered longest-phrase-first within an intent so that a
/// multi-word cue is matched before its own substrings.
const CUES: &[(&str, Intent, i32)] = &[
    // Compare — must be checked early; "difference between A and B" also
    // contains "between", which nothing else claims.
    ("difference between", Intent::Compare, 3),
    ("compare", Intent::Compare, 3),
    ("versus", Intent::Compare, 3),
    (" vs ", Intent::Compare, 3),
    ("differ from", Intent::Compare, 3),
    // Changed — the transition verbs.
    ("what changed", Intent::Changed, 4),
    ("changed since", Intent::Changed, 4),
    ("changed", Intent::Changed, 2),
    ("modified", Intent::Changed, 2),
    ("moved", Intent::Changed, 2),
    ("disappeared", Intent::Changed, 3),
    ("new since", Intent::Changed, 3),
    // History — the temporal verbs.
    ("history of", Intent::History, 4),
    ("over time", Intent::History, 3),
    ("when was", Intent::History, 3),
    ("when did", Intent::History, 3),
    ("what was i doing", Intent::History, 4),
    ("timeline", Intent::History, 3),
    ("every time", Intent::History, 3),
    ("history", Intent::History, 2),
    // Neighbors — structural proximity.
    ("near", Intent::Neighbors, 3),
    ("close to", Intent::Neighbors, 3),
    ("related to", Intent::Neighbors, 3),
    ("similar to", Intent::Neighbors, 3),
    ("neighbours", Intent::Neighbors, 3),
    ("neighbors", Intent::Neighbors, 3),
    ("goes with", Intent::Neighbors, 2),
    // Trace / Why — both take a position, and "why" is the stronger claim.
    ("trace", Intent::Trace, 3),
    ("chain behind", Intent::Trace, 4),
    ("provenance of", Intent::Trace, 3),
    ("where did this come from", Intent::Trace, 4),
    ("why", Intent::Why, 3),
    ("how come", Intent::Why, 3),
    ("follows from", Intent::Why, 3),
    ("follow from", Intent::Why, 3),
    // Locate — position lookup.
    ("where is", Intent::Locate, 4),
    ("where does", Intent::Locate, 3),
    ("locate", Intent::Locate, 3),
    ("which position", Intent::Locate, 3),
    ("where", Intent::Locate, 1),
    // Inspect — no term at all.
    ("what is in this world", Intent::Inspect, 4),
    ("what does this world contain", Intent::Inspect, 4),
    ("how many observations", Intent::Inspect, 4),
    ("summarise the world", Intent::Inspect, 4),
    ("inspect", Intent::Inspect, 3),
    ("overview", Intent::Inspect, 2),
    // Retrieve — explicit search language only; it is also the fallback.
    ("find", Intent::Retrieve, 2),
    ("search for", Intent::Retrieve, 3),
    ("look up", Intent::Retrieve, 2),
    ("show me everything about", Intent::Retrieve, 4),
    ("anything about", Intent::Retrieve, 3),
];

/// Words that carry no argument. Kept deliberately short: over-stripping is
/// how a term extractor turns `why did the build break` into `build break`
/// and then into nothing.
const STOP: &[&str] = &[
    "a", "an", "the", "is", "are", "was", "were", "do", "does", "did", "i",
    "me", "my", "we", "you", "it", "this", "that", "these", "those", "of",
    "to", "in", "on", "at", "for", "with", "and", "or", "but", "what",
    "which", "who", "whom", "how", "show", "tell", "give", "please", "all",
    "any", "about", "from", "between", "since", "then", "now", "here",
    "there", "thing", "things", "stuff", "everything", "something",
];

/// Parse one question into a proposal. Never runs anything.
///
/// Scoring is a sum over cues that appear as substrings of the lowercased
/// question. Substring rather than token match because the cues are phrases
/// (`"what was i doing"`), and a tokeniser that split them would need the
/// phrase table back to put them together again.
pub fn parse(question: &str) -> Proposal {
    let q = normalise(question);
    let mut evidence: Vec<Cue> = Vec::new();
    let mut scores: Vec<(Intent, i32)> = Intent::all().iter().map(|&i| (i, 0)).collect();

    for &(phrase, cue_intent, weight) in CUES {
        if q.contains(phrase) {
            evidence.push(Cue { phrase: phrase.to_string(), intent: cue_intent, weight });
            if let Some(slot) = scores.iter_mut().find(|(i, _)| *i == cue_intent) {
                slot.1 += weight;
            }
        }
    }

    // A bare integer argument is decisive for the two position-taking verbs:
    // `trace 41` and `why 41` are the only commands whose term is a number.
    let has_position = extract_position(&q).is_some();
    if has_position {
        for (intent, s) in scores.iter_mut() {
            if matches!(intent, Intent::Trace | Intent::Why) && *s > 0 {
                *s += 2;
            }
        }
    }

    // `Intent::all()` is in tie-break order, so the winner is the *first*
    // maximum. `max_by_key` keeps the last one, which silently handed
    // `"locate.rs versus retrieve.rs"` to `Locate` over `Compare` at 3–3, so
    // the fold below keeps the first instead.
    let first_max = |xs: &[(Intent, i32)]| -> (Intent, i32) {
        xs.iter().copied().fold((Intent::Retrieve, -1), |best, cur| {
            if cur.1 > best.1 { cur } else { best }
        })
    };
    let (intent, score) = first_max(&scores);

    let others: Vec<(Intent, i32)> =
        scores.iter().copied().filter(|(i, _)| *i != intent).collect();
    let runner_up = Some(first_max(&others)).filter(|(_, s)| *s > 0);

    let fallback = score == 0;
    let intent = if fallback { Intent::Retrieve } else { intent };

    let term = match intent {
        Intent::Inspect => String::new(),
        Intent::Trace | Intent::Why => {
            extract_position(&q).map(|p| p.to_string()).unwrap_or_else(|| extract_term(&q))
        }
        _ => extract_term(&q),
    };

    Proposal {
        question: question.to_string(),
        intent,
        term,
        runner_up,
        score,
        evidence,
        fallback,
    }
}

/// Lowercase, and pad with spaces so a cue like `" vs "` can rely on word
/// boundaries at the ends of the string too.
fn normalise(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push(' ');
    for c in s.chars() {
        if c.is_alphanumeric() || c == '.' || c == '/' || c == '_' || c == '-' {
            out.extend(c.to_lowercase());
        } else {
            out.push(' ');
        }
    }
    out.push(' ');
    out
}

/// A standalone integer, which in this world is a log position.
fn extract_position(q: &str) -> Option<u64> {
    q.split_whitespace().find_map(|t| t.parse::<u64>().ok())
}

/// The argument. Preference order, most specific first:
///
/// 1. a path-like token (contains `/` or a dot with an extension) — these are
///    what `watch_fs` records as subjects, so they resolve exactly;
/// 2. any token that no cue phrase claimed and that is not a stop word.
///
/// Cue words are removed *before* stop words so that `"where is the build"`
/// keeps `build` rather than being eaten by the `where` cue's own tokens.
fn extract_term(q: &str) -> String {
    let cue_words: Vec<&str> = CUES
        .iter()
        .filter(|(phrase, _, _)| q.contains(phrase))
        .flat_map(|(phrase, _, _)| phrase.split_whitespace())
        .collect();

    let toks: Vec<&str> = q
        .split_whitespace()
        .filter(|t| !cue_words.contains(t))
        .filter(|t| !STOP.contains(t))
        .collect();

    if let Some(path) = toks.iter().find(|t| t.contains('/') || t.contains('.')) {
        return (*path).to_string();
    }
    toks.join(" ")
}

/// The null this parser is scored against: answer every question with the
/// most frequent intent, ignoring the words entirely.
///
/// It is handed the majority intent of the very set it is scored on, which is
/// more than the parser is told. A parser that cannot beat this is reading the
/// question for nothing.
pub fn null_majority(question: &str, majority: Intent) -> Proposal {
    Proposal {
        question: question.to_string(),
        intent: majority,
        term: extract_term(&normalise(question)),
        runner_up: None,
        score: 0,
        evidence: Vec::new(),
        fallback: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Question set. Sources, so the provenance of the *benchmark* is also on
    /// the record: `B` = phrasings from the brief this module was built from,
    /// `H` = the `physis-world --help` text restated as a question, `N` =
    /// natural variants written for this test.
    ///
    /// None of these came from a log of what a person actually typed. That is
    /// the benchmark's central weakness and it is stated in the module docs.
    const SET: &[(&str, Intent)] = &[
        // Locate
        ("where is observe.rs", Intent::Locate),
        ("where does the licence gate live", Intent::Locate),
        ("locate the ontology file", Intent::Locate),
        ("which position holds Cargo.toml", Intent::Locate),
        ("where is physis_resources.txt", Intent::Locate),
        // History  (B: "What was I doing when this broke?")
        ("what was i doing when the build broke", Intent::History),
        ("history of direction.rs", Intent::History),
        ("when did i last touch act.rs", Intent::History),
        ("show me the timeline for the grid", Intent::History),
        ("every time i ran cargo test", Intent::History),
        ("how has propose.rs moved over time", Intent::History),
        ("when was the licence added", Intent::History),
        // Changed  (B: "Show me what changed since yesterday.")
        ("what changed since yesterday", Intent::Changed),
        ("what changed in physis-core", Intent::Changed),
        ("which files were modified today", Intent::Changed),
        ("show me what moved", Intent::Changed),
        ("anything that disappeared from the log", Intent::Changed),
        ("what is new since the last scan", Intent::Changed),
        // Neighbors  (B: "Find everything related to this concept.")
        ("what is related to coherence", Intent::Neighbors),
        ("what sits near embed.rs", Intent::Neighbors),
        ("things similar to hypothesis.rs", Intent::Neighbors),
        ("neighbours of store.rs", Intent::Neighbors),
        ("what is close to the ontology grid", Intent::Neighbors),
        ("what goes with provenance.rs", Intent::Neighbors),
        // Compare
        ("compare observe.rs and act.rs", Intent::Compare),
        ("difference between physis-core and physis-pro", Intent::Compare),
        ("locate.rs versus retrieve.rs", Intent::Compare),
        ("how does direction.rs differ from transform.rs", Intent::Compare),
        // Trace
        ("trace position 41", Intent::Trace),
        ("show the chain behind 128", Intent::Trace),
        ("what is the provenance of 900", Intent::Trace),
        ("where did this come from 12", Intent::Trace),
        // Why  (B: "Why did I stop using this approach?")
        ("why is position 77 here", Intent::Why),
        ("why did i stop using the cosine scorer", Intent::Why),
        ("how come 310 is in the log", Intent::Why),
        ("what does 44 follow from", Intent::Why),
        // Inspect
        ("what is in this world", Intent::Inspect),
        ("how many observations are there", Intent::Inspect),
        ("give me an overview", Intent::Inspect),
        ("inspect the log", Intent::Inspect),
        ("what does this world contain", Intent::Inspect),
        // Retrieve  (B: "Find the sources supporting this proposition.")
        ("find the sources supporting the null control", Intent::Retrieve),
        ("search for the ed25519 gate", Intent::Retrieve),
        ("look up the benchmark results", Intent::Retrieve),
        ("show me everything about the semiotic grid", Intent::Retrieve),
        ("anything about token fixed rag", Intent::Retrieve),
        ("find the version where i solved the filing problem", Intent::Retrieve),
        ("cargo test failures", Intent::Retrieve),
    ];

    /// The questions this parser gets wrong, kept in `SET` and named here.
    /// Repo rule: a failure comment is updated, never deleted. If a change
    /// makes one of these pass, take it out of this list and say why.
    const FAILING: &[&str] = &[
        // "broke" is not a transition cue, and "what was i doing" wins — which
        // is arguably right. Listed because the intended verb is debatable,
        // not because the parse is stupid.
        "",
    ];

    fn majority() -> Intent {
        let mut best = (Intent::Retrieve, 0usize);
        for &i in Intent::all() {
            let c = SET.iter().filter(|(_, g)| *g == i).count();
            if c > best.1 {
                best = (i, c);
            }
        }
        best.0
    }

    /// Held-out set: the same nine intents, phrased so as to **avoid the cue
    /// vocabulary on purpose**. Written after the cue table was frozen and
    /// never fed back into it — when one of these misses, the miss is
    /// recorded in `HELD_OUT_MISSES`, the cue table is not edited.
    ///
    /// This is the only number in this module that can move. `SET` scores
    /// 1.000 because it and the cues were written together; that is a
    /// self-consistency check, not a result.
    const HELD_OUT: &[(&str, Intent)] = &[
        ("point me at the ontology file", Intent::Locate),
        ("in which observation does Cargo.toml appear", Intent::Locate),
        ("what did the log look like before i touched act.rs", Intent::History),
        ("walk me through act.rs from the beginning", Intent::History),
        ("which files stopped being the same", Intent::Changed),
        ("anything not stable in physis-core", Intent::Changed),
        ("what clusters with embed.rs", Intent::Neighbors),
        ("what else looks like hypothesis.rs", Intent::Neighbors),
        ("put observe.rs and act.rs side by side", Intent::Compare),
        ("is direction.rs anything like transform.rs", Intent::Compare),
        ("unpack observation 41 for me", Intent::Trace),
        ("what produced 128", Intent::Trace),
        ("on what grounds is 77 in the log", Intent::Why),
        ("justify position 310", Intent::Why),
        ("how big is the log", Intent::Inspect),
        ("pull up the benchmark results", Intent::Retrieve),
    ];

    /// Held-out questions the parser gets wrong. Repo rule: a failure comment
    /// is updated, never deleted. Removing a line here requires saying which
    /// change made it pass and why that change was not written *for* it.
    const HELD_OUT_MISSES: &[&str] = &[
        // All but one. Fifteen of these fire no cue whatsoever and fall
        // through to `Retrieve`; the exception is "pull up the benchmark
        // results", which is right by accident because `Retrieve` is also the
        // fallback. Not patched: covering these by adding their words to the
        // cue table would be fitting the test that refuted the table.
        "point me at the ontology file",
        "in which observation does Cargo.toml appear",
        "what did the log look like before i touched act.rs",
        "walk me through act.rs from the beginning",
        "which files stopped being the same",
        "anything not stable in physis-core",
        "what clusters with embed.rs",
        "what else looks like hypothesis.rs",
        "put observe.rs and act.rs side by side",
        "is direction.rs anything like transform.rs",
        "unpack observation 41 for me",
        "what produced 128",
        "on what grounds is 77 in the log",
        "justify position 310",
        "how big is the log",
    ];

    #[test]
    fn held_out_is_the_number_that_can_move() {
        let maj = majority();
        let hit = HELD_OUT.iter().filter(|(q, g)| parse(q).intent == *g).count();
        let null_hit = HELD_OUT
            .iter()
            .filter(|(q, g)| null_majority(q, maj).intent == *g)
            .count();
        let acc = hit as f64 / HELD_OUT.len() as f64;
        eprintln!(
            "HELD-OUT intent accuracy {hit}/{} = {acc:.3}   majority null {null_hit}/{}",
            HELD_OUT.len(),
            HELD_OUT.len()
        );
        for (q, g) in HELD_OUT {
            let p = parse(q);
            if p.intent != *g {
                eprintln!("  HELD-OUT MISS {:?} != {:?}  «{q}»  score {}", p.intent, g, p.score);
            }
        }
        // The recorded refutation, asserted in both directions so that it
        // cannot drift unnoticed: 1/16 for the parser, 2/16 for a null that
        // ignores the words. If either moves, the module docs are wrong and
        // this test says so before anyone quotes them.
        assert_eq!(hit, 1, "held-out accuracy moved; update the table in the module docs");
        assert_eq!(null_hit, 2, "the null moved; the held-out set or `majority()` changed");
        assert!(
            hit < null_hit,
            "the cue parser is recorded as LOSING to the null here — if it now wins, \
             say which change did it and whether that change was written for this set"
        );
        let declared: Vec<&str> = HELD_OUT
            .iter()
            .filter(|(q, g)| parse(q).intent != *g)
            .map(|(q, _)| *q)
            .collect();
        assert_eq!(declared, HELD_OUT_MISSES, "the declared failure list is out of date");
    }

    #[test]
    fn parser_beats_both_nulls() {
        let maj = majority();
        let hit = SET.iter().filter(|(q, g)| parse(q).intent == *g).count();
        let null_hit = SET
            .iter()
            .filter(|(q, g)| null_majority(q, maj).intent == *g)
            .count();

        let acc = hit as f64 / SET.len() as f64;
        let null_acc = null_hit as f64 / SET.len() as f64;
        eprintln!(
            "intent accuracy {hit}/{} = {acc:.3}   majority null ({:?}) {null_hit}/{} = {null_acc:.3}",
            SET.len(),
            maj,
            SET.len()
        );
        for (q, g) in SET {
            let p = parse(q);
            if p.intent != *g {
                eprintln!("  MISS {:?} != {:?}  «{q}»  score {}", p.intent, g, p.score);
            }
        }
        // This set cannot fail honestly — it was written beside the cues — so
        // it is asserted exactly, as a tripwire on the docs rather than as
        // evidence of anything. `held_out_is_the_number_that_can_move` is the
        // test that carries the real claim.
        assert_eq!(hit, SET.len(), "self-consistency set moved; the docs quote 1.000");
        assert!(acc > null_acc, "even the self-consistent set must beat its null");
    }

    #[test]
    fn nothing_is_executed_and_fallback_is_visible() {
        let p = parse("qwertyuiop");
        assert_eq!(p.intent, Intent::Retrieve);
        assert!(p.fallback, "an unparseable question must be flagged, not silently searched");
        assert_eq!(p.score, 0);
    }

    #[test]
    fn evidence_accounts_for_every_point() {
        for (q, _) in SET {
            let p = parse(q);
            let from_cues: i32 = p
                .evidence
                .iter()
                .filter(|c| c.intent == p.intent)
                .map(|c| c.weight)
                .sum();
            // The only points not from cues are the +2 position bonus.
            let unexplained = p.score - from_cues;
            assert!(
                unexplained == 0 || unexplained == 2,
                "«{q}» has {unexplained} points no cue accounts for"
            );
        }
    }

    #[test]
    fn position_taking_verbs_extract_the_number() {
        assert_eq!(parse("trace position 41").term, "41");
        assert_eq!(parse("why is position 77 here").term, "77");
    }

    #[test]
    fn paths_win_over_bare_words() {
        assert_eq!(parse("where is observe.rs").term, "observe.rs");
        assert_eq!(parse("neighbours of store.rs").term, "store.rs");
    }

    #[test]
    fn command_is_runnable_text_not_an_intention() {
        let p = parse("what changed since yesterday");
        assert!(p.command().starts_with("physis-world changed"));
        assert_eq!(parse("what is in this world").command(), "physis-world inspect");
    }

    #[test]
    fn failing_cases_are_declared() {
        for f in FAILING.iter().filter(|f| !f.is_empty()) {
            assert!(SET.iter().any(|(q, _)| q == f), "declared failure «{f}» is not in the set");
        }
    }
}
