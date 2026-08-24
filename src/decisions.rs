//! Phase 9.3 — decision mining from transcript episodes (clean-room
//! adaptation, docs/reference/repowise-adaptation.md P4).
//!
//! Pipeline: sessions::Event stream → [`Episode`] (one user prompt + its
//! assistant/tool traffic) → deterministic cue-based candidate extraction →
//! [`grounding`] gate. Embedding dedup against a prior-decision store is the
//! caller's job (needs an embedder); this module ships the pure mining +
//! exact-title dedup so both are testable without a model.

use crate::grounding::{apply_gate, Candidate, Verdict};
use crate::sessions::Event;

/// One conversational episode: a user turn plus everything the agent did
/// about it until the next user turn.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Episode {
    /// User prose that started the episode.
    pub prompt: String,
    /// Assistant prose turns.
    pub assistant: Vec<String>,
    /// Tool names invoked.
    pub tools: Vec<String>,
}

/// Fold an event stream into episodes.
pub fn fold_episodes(events: &[Event]) -> Vec<Episode> {
    let mut out: Vec<Episode> = Vec::new();
    for ev in events {
        match ev {
            Event::UserTurn { text } => {
                if let Some(last) = out.last_mut() {
                    if last.prompt.is_empty() && last.assistant.is_empty() && last.tools.is_empty()
                    {
                        last.prompt = text.clone();
                        continue;
                    }
                }
                out.push(Episode {
                    prompt: text.clone(),
                    ..Default::default()
                });
            }
            Event::AssistantTurn { text } => {
                if text.trim().is_empty() {
                    continue;
                }
                if let Some(last) = out.last_mut() {
                    last.assistant.push(text.clone());
                } else {
                    // Assistant speech before any user turn still belongs to an episode.
                    out.push(Episode {
                        prompt: String::new(),
                        assistant: vec![text.clone()],
                        tools: Vec::new(),
                    });
                }
            }
            Event::ToolCall { name } => {
                if let Some(last) = out.last_mut() {
                    last.tools.push(name.clone());
                }
            }
            Event::ToolResult { .. } => {} // outcomes are provenance, not decisions
        }
    }
    out
}

/// Cues whose sentence plausibly records a decision ("we chose X because Y").
const DECISION_CUES: [&str; 8] = [
    "we chose",
    "we'll use",
    "we will use",
    "we adopt",
    "we're switching",
    "decision:",
    "we decided",
    "we went with",
];

/// Split prose into sentences on `.`, `!`, `?` boundaries.
fn sentences(text: &str) -> impl Iterator<Item = &str> {
    text.split(['.', '!', '?'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Deterministic candidate extraction from one episode: any sentence
/// containing a decision cue becomes a candidate grounded in that episode's
/// own text (the source span IS the conversation — self-grounding by
/// construction unless the extractor invents).
pub fn mine_decisions(episodes: &[Episode]) -> Vec<Candidate> {
    let mut out = Vec::new();
    for ep in episodes {
        let source = {
            let mut parts = ep.assistant.clone();
            if !ep.prompt.is_empty() {
                parts.insert(0, ep.prompt.clone());
            }
            parts.join(". ")
        };
        for turn in &ep.assistant {
            for s in sentences(turn) {
                let lower = s.to_lowercase();
                if DECISION_CUES.iter().any(|cue| lower.contains(cue)) {
                    out.push(Candidate {
                        decision: s.to_owned(),
                        rationale: String::new(),
                        source_quote: s.to_owned(),
                        source_text: source.clone(),
                        verification: Verdict::Unverified,
                    });
                }
            }
        }
    }
    out
}

/// Normalize a decision title for exact-match dedup: lowercase alphanumeric run.
fn title_key(text: &str) -> String {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Exact-title dedup pass. Later paraphrase handling is 9.3b (embedding
/// cosine search over the shared node store — needs an embedder handle).
pub fn dedup_by_title(decisions: &[Candidate]) -> Vec<Candidate> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for c in decisions {
        if seen.insert(title_key(&c.decision)) {
            out.push(c.clone());
        }
    }
    out
}

/// Full deterministic pipeline: events → episodes → candidates → gate → dedup.
pub fn extract(events: &[Event]) -> Vec<Candidate> {
    let gated = apply_gate(mine_decisions(&fold_episodes(events))).0;
    dedup_by_title(&gated)
        .into_iter()
        .filter(|c| c.verification != Verdict::Unverified)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(t: &str) -> Event {
        Event::UserTurn { text: t.into() }
    }
    fn a(t: &str) -> Event {
        Event::AssistantTurn { text: t.into() }
    }

    #[test]
    fn fold_groups_events_into_prompt_scoped_episodes() {
        let eps = fold_episodes(&[
            u("fix flaky test"),
            a("We chose retry-once semantics."),
            Event::ToolCall { name: "Bash".into() },
            u("now the docs"),
            a("Docs updated."),
        ]);
        assert_eq!(eps.len(), 2);
        assert_eq!(eps[0].prompt, "fix flaky test");
        assert_eq!(eps[0].tools, vec!["Bash"]);
        assert_eq!(eps[1].prompt, "now the docs");
    }

    #[test]
    fn consecutive_user_turns_start_new_episodes() {
        let eps = fold_episodes(&[u("a"), u("b")]);
        assert_eq!(eps.len(), 2);
    }

    #[test]
    fn cue_sentences_become_candidates() {
        let eps = [Episode {
            prompt: "pick a store".into(),
            assistant: vec!["We chose sled because it is embeddable. The build is green."
                .into()],
            tools: vec![],
        }];
        let mined = mine_decisions(&eps);
        assert_eq!(mined.len(), 1);
        assert!(mined[0].decision.contains("sled"));
    }

    #[test]
    fn gate_passes_self_grounded_candidates() {
        let events = vec![
            u("pick a store"),
            a("We chose sled for persistence."),
        ];
        let got = extract(&events);
        assert_eq!(got.len(), 1);
        // rationale empty => only 2 of 3 fields grounded => Partial, kept.
        assert_eq!(got[0].verification, Verdict::Partial);
    }

    #[test]
    fn duplicate_titles_collapse() {
        let d1 = Candidate {
            decision: "Adopt sled for persistence".into(),
            rationale: String::new(),
            source_quote: "Adopt sled for persistence".into(),
            source_text: "Adopt sled for persistence!".into(),
            verification: Verdict::Grounded,
        };
        let mut d2 = d1.clone();
        d2.decision = "adopt SLED for persistence".into(); // same key after folding
        let deduped = dedup_by_title(&[d1, d2]);
        assert_eq!(deduped.len(), 1);
    }

    #[test]
    fn no_cues_means_no_decisions_not_a_crash() {
        let eps = [Episode {
            prompt: "hello".into(),
            assistant: vec!["Hi there, how can I help?".into()],
            tools: vec![],
        }];
        assert!(mine_decisions(&eps).is_empty());
        assert!(extract(&[]).is_empty());
    }
}
