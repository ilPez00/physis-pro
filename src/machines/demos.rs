//! Sample runs for the philosophical machines — one demonstrative input each.
//!
//! These lived as private helpers inside `bin/tui.rs`, which meant any second
//! front-end (the Studio) had to copy them. They are library code now: small,
//! deterministic, no I/O, and the same call renders the same block in any shell.
//! `bin/tui.rs` keeps its own copy for the moment; S6 of
//! `docs/plans/2026-09-18-studio-tui-demo.md` folds it onto this module.

use chrono::{Duration as ChronoDuration, Utc};

use crate::machines::{chronos, elenchus, harmony, lex, polis, soma, techne};

/// Run the named machine on its sample input. Unknown names return a message
/// rather than panicking — the caller is a keystroke, not a compiler.
pub fn run(name: &str) -> String {
    match name {
        "chronos" => chronos_demo(),
        "techne" => techne_demo(),
        "soma" => soma_demo(),
        "elenchus" => elenchus_demo(),
        "lex" => lex_demo(),
        "harmony" => harmony_demo(),
        "polis" => polis_demo(),
        _ => format!("unknown machine: {name}"),
    }
}

/// Every machine name [`run`] accepts, in display order.
pub fn names() -> Vec<&'static str> {
    vec![
        "chronos", "techne", "soma", "elenchus", "lex", "harmony", "polis",
    ]
}

fn chronos_demo() -> String {
    let now = Utc::now();
    let ev = |label: &str, days_ago: i64| chronos::TimedEvent {
        label: label.into(),
        at: now - ChronoDuration::days(days_ago),
    };
    let mut events: Vec<chronos::TimedEvent> = (0..7).map(|i| ev("gym", 2 + i * 2)).collect();
    for d in [32, 25, 18, 11] {
        events.push(ev("standup", d));
    }
    for d in [21, 9, 2] {
        events.push(ev("call mum", d));
    }
    let rhythms = chronos::detect_rhythms(&events);
    let mut s = format!("{} events → {} rhythms:\n", events.len(), rhythms.len());
    for r in &rhythms {
        s += &format!(
            "  {:<10} every {:>4.1}d  conf {:.2}  next {}\n",
            r.label,
            r.period_days,
            r.confidence,
            r.next_due.format("%Y-%m-%d")
        );
    }
    let slip = chronos::slipping(&rhythms, now, 0.5);
    if !slip.is_empty() {
        s += &format!(
            "slipping: {}",
            slip.iter()
                .map(|r| r.label.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    s
}

fn techne_demo() -> String {
    let f = |path: &str, lines: &[&str]| techne::SourceFile {
        path: path.into(),
        lines: lines.iter().map(|s| s.to_string()).collect(),
    };
    let files = vec![
        f("src/embed/mod.rs", &[]),
        f("src/classify.rs", &["use crate::embed::VectorEmbed;"]),
        f(
            "src/core.rs",
            &["use crate::trie::Trie;", "use crate::embed::VectorEmbed;"],
        ),
        f("src/trie.rs", &[]),
        f("src/bin/web_server.rs", &["use crate::core::PhysisCore;"]),
        f("src/spanner.rs", &["use crate::core::PhysisCore;"]),
    ];
    let mut s = String::from("capabilities:\n");
    for c in techne::classify_capabilities(&files) {
        s += &format!("  {:<14} {}\n", c.name, c.files.join(", "));
    }
    let edges = techne::dependency_edges(&files);
    s += &format!(
        "impact of `embed`: {}",
        techne::impact_of(&edges, "embed").join(", ")
    );
    s
}

fn soma_demo() -> String {
    let s = |day: u32, load: f32, recovery: f32| soma::Session {
        day,
        activity: "lift".into(),
        load,
        recovery,
    };
    let sessions = vec![
        s(1, 0.5, 0.9),
        s(2, 0.7, 0.8),
        s(3, 0.8, 0.6),
        s(4, 0.8, 0.5),
        s(5, 0.9, 0.4),
        s(6, 0.9, 0.35),
    ];
    let mut out = String::new();
    for x in &sessions {
        out += &format!(
            "  day {}  load {:.1} rec {:.1}  {:?}\n",
            x.day,
            x.load,
            x.recovery,
            soma::classify(x)
        );
    }
    let d = soma::drift(&sessions, 6);
    out += &format!(
        "drift {:+.2} → {}",
        d,
        if d > 0.5 {
            "overreaching risk"
        } else {
            "stable"
        }
    );
    out
}

fn elenchus_demo() -> String {
    let st =
        |id: &str, kind: elenchus::Kind, text: &str, target: Option<&str>| elenchus::Statement {
            id: id.into(),
            kind,
            text: text.into(),
            target: target.map(|s| s.into()),
        };
    let arg = vec![
        st(
            "c1",
            elenchus::Kind::Claim,
            "Remote work raises output",
            None,
        ),
        st(
            "w1",
            elenchus::Kind::Warrant,
            "fewer commute hours",
            Some("c1"),
        ),
        st(
            "c2",
            elenchus::Kind::Claim,
            "Open offices boost collaboration",
            None,
        ),
        st(
            "r2",
            elenchus::Kind::Rebuttal,
            "studies show -70% face time",
            Some("c2"),
        ),
        st(
            "c3",
            elenchus::Kind::Claim,
            "Four-day weeks always better",
            None,
        ),
    ];
    let mut s = String::new();
    for (id, standing) in elenchus::evaluate(&arg) {
        let t = arg.iter().find(|x| x.id == id).unwrap();
        s += &format!("  {:<3} {:<12?} {}\n", id, standing, t.text);
    }
    s += &format!(
        "press: {:?}\nrecalibrate: {:?}",
        elenchus::unsupported_claims(&arg),
        elenchus::contested_claims(&arg)
    );
    s
}

fn lex_demo() -> String {
    let c = |id: &str, kind: lex::Kind, requires: &[&str]| lex::Clause {
        id: id.into(),
        kind,
        text: id.into(),
        requires: requires.iter().map(|s| s.to_string()).collect(),
    };
    let contract = vec![
        c("deliver", lex::Kind::Obligation, &[]),
        c("pay", lex::Kind::Obligation, &["delivered"]),
        c("delivered", lex::Kind::Condition, &[]),
        c("refund", lex::Kind::Obligation, &["returned"]),
        c("warranty", lex::Kind::Right, &["registered"]),
    ];
    format!(
        "always due: {:?}\nbreach if `delivered` fails: {:?}\nunhandled: {:?}",
        lex::unconditional_obligations(&contract),
        lex::breach_paths(&contract, &["delivered".into()]),
        lex::dangling_conditions(&contract),
    )
}

fn harmony_demo() -> String {
    let melody = vec![60, 62, 64, 63, 65, 67, 69, 68, 67];
    let mut s = format!("intervals: {:?}\nmotifs:\n", harmony::intervals(&melody));
    for (seq, count) in harmony::motifs(&melody, 3) {
        s += &format!("  {:?} x{}\n", seq, count);
    }
    s += &format!("transpose +5: {:?}", harmony::transpose(&[60, 62, 64], 5));
    s
}

fn polis_demo() -> String {
    let i = |from: &str, to: &str, w: f32| polis::Interaction {
        from: from.into(),
        to: to.into(),
        weight: w,
    };
    let interactions = vec![
        i("you", "ana", 9.0),
        i("ana", "you", 8.0),
        i("you", "ben", 7.0),
        i("ben", "you", 1.0),
        i("ana", "ben", 4.0),
        i("ben", "ana", 4.0),
    ];
    let ties = polis::ties(&interactions);
    let mut s = String::from("ties:\n");
    for t in &ties {
        s += &format!(
            "  {:<4}-{:<4} strength {:>4.1} recip {:.2}\n",
            t.a, t.b, t.strength, t.reciprocity
        );
    }
    let fade = polis::fading(&ties, 0.5);
    if !fade.is_empty() {
        s += &format!(
            "fading: {}",
            fade.iter()
                .map(|t| format!("{}-{}", t.a, t.b))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every advertised name must produce something, and an unknown name must
    /// not panic: the caller is a keystroke, not a compile-time constant.
    #[test]
    fn every_machine_runs_and_unknown_names_are_reported() {
        for name in names() {
            let out = run(name);
            assert!(!out.trim().is_empty(), "{name} produced nothing");
            assert!(
                !out.contains("unknown machine"),
                "{name} not wired into run()"
            );
        }
        assert!(run("nope").contains("unknown machine"));
    }
}
