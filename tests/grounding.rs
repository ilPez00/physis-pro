//! Phase 9.2 — anti-hallucination grounding gate (clean-room adaptation,
//! see docs/reference/repowise-adaptation.md P3).
//!
//! Product guarantee: any produced field (`decision` / `rationale` /
//! `source_quote`) that does NOT token-match the verbatim `source_text` it
//! claims to come from is DROPPED; a candidate whose every field fails is
//! REJECTED outright. This is what stops a fluent-but-invented rationale from
//! being stored as institutional memory.

use physis_pro::grounding::{apply_gate, Candidate, Verdict};

fn grounded() -> Candidate {
    Candidate {
        decision: "Adopt sled for persistence".into(),
        rationale: "sled gives ACID transactions without a server".into(),
        source_quote: "sled gives ACID transactions without a server".into(),
        source_text: "We will adopt sled for persistence because sled gives ACID \
                      transactions without a server, and the deployment target \
                      cannot run Postgres."
            .into(),
        verification: Verdict::Unverified,
    }
}

#[test]
fn fully_grounded_candidate_passes_all_fields() {
    let (kept, rejected) = apply_gate(vec![grounded()]);
    assert_eq!(rejected, 0);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].verification, Verdict::Grounded);
}

#[test]
fn invented_rationale_is_dropped_candidate_survives_as_partial() {
    let mut c = grounded();
    c.rationale = "sled was recommended by the Rust performance book".into(); // not in source
    let (kept, rejected) = apply_gate(vec![c]);
    assert_eq!(rejected, 0, "decision+quote still grounded: evidence survives");
    assert_eq!(kept[0].verification, Verdict::Partial);
    assert_eq!(
        kept[0].rationale, "",
        "ungrounded FIELD is dropped, never stored"
    );
}

#[test]
fn every_field_invented_is_rejected() {
    let mut c = grounded();
    c.decision = "Switch everything to MongoDB".into();
    c.rationale = "MongoDB is web scale".into();
    c.source_quote = "it horizontally super duper scales".into();
    let (kept, rejected) = apply_gate(vec![c]);
    assert_eq!(rejected, 1, "no surviving evidence => candidate rejected");
    assert!(kept.is_empty());
}

#[test]
fn invented_quote_drops_field_but_survives_on_other_evidence() {
    let mut c = grounded();
    c.source_quote = "verbatim words that appear nowhere in the source text".into();
    let (kept, rejected) = apply_gate(vec![c]);
    assert_eq!(rejected, 0, "decision+rationale still grounded");
    assert_eq!(kept[0].verification, Verdict::Partial);
}

#[test]
fn token_match_ignores_punctuation_and_case() {
    let mut c = grounded();
    c.source_text = "SLED GIVES acid transactions, without a server!".into();
    // rationale tokens all present once punctuation/case folded
    let (kept, rejected) = apply_gate(vec![c]);
    assert_eq!((kept.len(), rejected), (1, 0));
}

#[test]
fn empty_source_text_rejects_everything() {
    let mut c = grounded();
    c.source_text = "".into();
    let (kept, rejected) = apply_gate(vec![c]);
    assert_eq!((kept.len(), rejected), (0, 1));
}

#[test]
fn batch_mixed_outcomes_are_independent() {
    let bad = Candidate {
        decision: "Use Redis cluster mode".into(),
        rationale: "redis cluster scales horizontally".into(),
        source_quote: "redis cluster scales horizontally".into(),
        source_text: "The cache layer is memcached behind a VIP.".into(), // nothing matches
        verification: Verdict::Unverified,
    };
    let (kept, rejected) = apply_gate(vec![grounded(), bad, grounded()]);
    assert_eq!((kept.len(), rejected), (2, 1));
}
