//! Experiment 40 — chronos + becoming: rhythm and trajectory over ONTOLOGY
//! labels rather than induced clusters.
//!
//! Iteration 39 measured exactly where `becoming` breaks. The runs test is
//! sound — its unit tests separate `AAAAAABBBBBB` from `ABABBABAABAB`, which no
//! clusterer can do — but the step feeding it was not. Cluster separation
//! cannot tell "one sense used broadly" from "two senses": "test" occurs in
//! wildly varied contexts with one meaning and separated at 0.806, while "key"
//! carries genuinely distinct senses and separated at 0.087. Word sense
//! induction was the unsolved input, not the statistic.
//!
//! `chronos` does not have that problem, because it works on **labels**. So
//! this replaces the induced partition with a label that has a referent:
//!
//!   occurrence -> local window -> CellClassifier -> (domain, mode) cell
//!
//! The cell vocabulary is hand-authored and stable — the one thing this whole
//! research track measured to be reliable — and "same" or "different" is then
//! decided rather than measured.
//!
//! Two readings of the same label sequence, which is the point of the pairing:
//!
//!   chronos::detect_rhythms   is the term's occurrence RHYTHM regular?
//!                             `confidence` = regularity (1 - CV of intervals)
//!                             x evidence; `slipping` flags a broken beat.
//!   becoming::classify_labeled  are the two leading labels time-separated
//!                             (a becoming) or interleaved (a split)?
//!
//! Together: the tempo and the melody. A term can keep its rhythm while its
//! sense changes, or hold its sense while its rhythm slips, and the pair says
//! which.
//!
//! Ground truth is the same as Iteration 39's, taken from git rather than from
//! reading: "coverage" became a module name at a timestamped commit, "linkage"
//! on 06 Sep, "disposer" did not exist in this sense.
//!
//! Run:
//!   cargo run -p physis_pro --release --example experiment40_chronos_becoming -- <corpus.json>

use chrono::{DateTime, Utc};
use physis_core::becoming::{classify_labeled, Trajectory};
use physis_core::classify::CellClassifier;
use physis_core::ontology::OntologyLoader;
use physis_pro::machines::chronos::{detect_rhythms, slipping, TimedEvent};
use serde::Deserialize;

/// physis_pro and physis_core each declare their own `VectorEmbed` trait, so a
/// pro embedder cannot be handed to core's `CellClassifier` directly. This
/// forwards one to the other — the two traits agree on `embed` and `dimension`,
/// which is all the classifier uses.
struct CoreEmbed<'a>(&'a dyn physis_pro::embed::VectorEmbed);

impl physis_core::embed::VectorEmbed for CoreEmbed<'_> {
    fn embed(&self, text: &str) -> Vec<f32> {
        self.0.embed(text)
    }
    fn dimension(&self) -> usize {
        self.0.dimension()
    }
}

#[derive(Deserialize)]
struct Event {
    #[serde(default)]
    subject: String,
    #[serde(default)]
    evidence: Vec<String>,
    observed_at: String,
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "operational_corpus.json".to_string());
    println!("Experiment 40: chronos rhythm + becoming trajectory, over ontology labels\n");

    let Ok(raw) = std::fs::read_to_string(&path) else {
        println!("cannot read {path} — run populate_operational first.");
        return;
    };
    let Ok(mut events) = serde_json::from_str::<Vec<Event>>(&raw) else {
        println!("{path} is not a Vec<Event>.");
        return;
    };
    // Chronological order is the evidence; sort rather than trust the file.
    events.sort_by(|a, b| a.observed_at.cmp(&b.observed_at));
    let texts: Vec<String> = events
        .iter()
        .map(|e| format!("{} {}", e.subject, e.evidence.join(" ")).to_lowercase())
        .collect();
    let times: Vec<DateTime<Utc>> = events
        .iter()
        .map(|e| {
            DateTime::parse_from_rfc3339(&e.observed_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now())
        })
        .collect();
    println!("corpus: {} events", events.len());

    let config = physis_pro::config::PhysisConfig::default();
    let (embedder, which) = physis_pro::embed::select_embedder(&config);
    println!("embedder: {which}");
    let core_embed = CoreEmbed(embedder.as_ref());
    let ontology = OntologyLoader::load_all();
    let classifier = CellClassifier::build(&ontology, &core_embed);
    println!("classifier: {} populated cells\n", classifier.cell_count());

    const WINDOW: usize = 12;
    // A term's occurrences, each labelled by the ontology cell its LOCAL
    // context classifies into. The term itself is dropped from its own window:
    // it is in every occurrence and would only pull them all one way.
    let labelled = |term: &str| -> (Vec<String>, Vec<DateTime<Utc>>) {
        let (mut labels, mut when) = (Vec::new(), Vec::new());
        for (i, t) in texts.iter().enumerate() {
            let toks: Vec<&str> = t.split_whitespace().collect();
            for (k, w) in toks.iter().enumerate() {
                let clean: String = w.chars().filter(|c| c.is_alphanumeric()).collect();
                if clean != term {
                    continue;
                }
                let lo = k.saturating_sub(WINDOW);
                let hi = (k + WINDOW + 1).min(toks.len());
                let win: Vec<&str> = toks[lo..hi]
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| lo + j != k)
                    .map(|(_, s)| *s)
                    .collect();
                if win.len() < 6 {
                    continue;
                }
                let scored = classifier.classify_text(&win.join(" "), &core_embed);
                if let Some(top) = scored.first() {
                    labels.push(format!("{}/{}", top.domain, top.mode));
                    when.push(times[i]);
                }
            }
        }
        (labels, when)
    };

    let cases: [(&str, &[&str]); 3] = [
        ("BECOMING expected", &["coverage", "linkage", "disposer"]),
        ("SPLIT expected", &["key", "core"]),
        ("STABLE expected", &["commit", "test", "file"]),
    ];

    let now = *times.last().unwrap_or(&Utc::now());
    let (mut correct, mut scored_n) = (0usize, 0usize);
    println!(
        "  term          n   distinct   trajectory   runs z   | rhythm: period  conf   slipping"
    );
    for (label, terms) in cases {
        println!("\n  --- {label} ---");
        for t in terms {
            let (labels, when) = labelled(t);
            let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
            let v = classify_labeled(&refs);

            // chronos reads the SAME labels as a rhythm: one TimedEvent per
            // occurrence, labelled by its cell.
            let tev: Vec<TimedEvent> = labels
                .iter()
                .zip(&when)
                .map(|(l, w)| TimedEvent {
                    label: l.clone(),
                    at: *w,
                })
                .collect();
            let rhythms = detect_rhythms(&tev);
            let slip = slipping(&rhythms, now, 0.5).len();
            let lead = rhythms
                .iter()
                .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap());

            let expected = match label {
                "BECOMING expected" => Trajectory::Becoming,
                "SPLIT expected" => Trajectory::Split,
                _ => Trajectory::Stable,
            };
            let ok = v.trajectory == expected;
            if v.trajectory != Trajectory::TooFew {
                scored_n += 1;
                if ok {
                    correct += 1;
                }
            }
            let distinct = {
                let mut u: Vec<&str> = refs.clone();
                u.sort();
                u.dedup();
                u.len()
            };
            println!(
                "  {:<11} {:>4}   {:>6}   {:<11?} {:>+6.2}   | {:>6}  {:>5}   {:>3}/{:<3} {}",
                t,
                labels.len(),
                distinct,
                v.trajectory,
                v.runs_z,
                lead.map(|r| format!("{:.1}d", r.period_days))
                    .unwrap_or_else(|| "-".into()),
                lead.map(|r| format!("{:.2}", r.confidence))
                    .unwrap_or_else(|| "-".into()),
                slip,
                rhythms.len(),
                if v.trajectory == Trajectory::TooFew {
                    "(too few)"
                } else if ok {
                    "as expected"
                } else {
                    "MISMATCH"
                }
            );
        }
    }

    println!("\n=== Verdict ===\n");
    println!("  {correct}/{scored_n} terms match what git says happened.");
    println!("  (Iteration 39, same terms, induced clusters instead of labels: 1/8.)");
    if scored_n > 0 && correct * 2 > scored_n {
        println!("\n  Replacing the induced partition with an ontology label is what changed.");
        println!("  The runs test was never the problem; the sense partition was.");
    } else {
        println!("\n  Labels do not rescue it either. The failure is upstream of both the");
        println!("  statistic AND the partition — most likely the corpus, which is one");
        println!("  project's own documentation and may simply not contain the kind of");
        println!("  meaning-change being looked for.");
    }
    println!(
        "\n(chronos and becoming read the SAME label sequence two ways: chronos as tempo\n — is the beat regular, is it slipping — and becoming as melody — are the two\n leading senses time-separated or interleaved. A term can hold its rhythm while\n its sense changes, or keep its sense while its rhythm slips. What is still weak:\n n = 8 terms chosen by me, and cell labels are a coarse sense inventory — 70\n cells for all of English is a blunt instrument for word senses.)"
    );
}
