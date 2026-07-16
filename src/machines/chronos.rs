//! chronos — the temporal machine.
//!
//! Perceive a stream of timestamped events; reduce to **recurrence primitives**
//! (rhythms); elaborate by predicting the next instance; flag rhythms about to
//! break (the "dream" beat — surface a slip before it happens).
//!
//! Pure + testable: `detect_rhythms` takes events and returns rhythms; the
//! `register_*` adapter is the only part that touches the core, so the logic can
//! be tested without an engine. Primitive = a *rhythm* (a recurring label with a
//! period), not a single event — the rhythm is the elaborable unit.

use chrono::{DateTime, Duration, Utc};

use crate::core::PhysisCore;
use crate::embed::{VectorEmbed, RandomProjectionEmbedder};

/// One observed event: a label and when it happened.
#[derive(Debug, Clone)]
pub struct TimedEvent {
    pub label: String,
    pub at: DateTime<Utc>,
}

/// A detected recurrence: a label that repeats on an (estimated) period.
#[derive(Debug, Clone, PartialEq)]
pub struct Rhythm {
    pub label: String,
    /// Estimated period between occurrences, in days (median inter-arrival).
    pub period_days: f64,
    /// Number of occurrences observed.
    pub count: usize,
    /// Most recent occurrence.
    pub last: DateTime<Utc>,
    /// Predicted next occurrence (`last + period`).
    pub next_due: DateTime<Utc>,
    /// 0–1: regularity × evidence. Regular, frequent rhythms score higher.
    pub confidence: f32,
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    if n == 0 {
        0.0
    } else if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

/// Reduce events to rhythms. A label needs ≥ 3 occurrences (≥ 2 intervals) to
/// qualify. Confidence = regularity (1 − coefficient of variation of intervals)
/// scaled by evidence (occurrences, saturating at ~5).
pub fn detect_rhythms(events: &[TimedEvent]) -> Vec<Rhythm> {
    use std::collections::HashMap;
    let mut by_label: HashMap<&str, Vec<DateTime<Utc>>> = HashMap::new();
    for e in events {
        by_label.entry(&e.label).or_default().push(e.at);
    }

    let mut rhythms = Vec::new();
    for (label, mut times) in by_label {
        if times.len() < 3 {
            continue;
        }
        times.sort();
        let intervals: Vec<f64> = times
            .windows(2)
            .map(|w| (w[1] - w[0]).num_seconds() as f64 / 86_400.0)
            .filter(|d| *d > 0.0)
            .collect();
        if intervals.len() < 2 {
            continue;
        }
        let period = median(intervals.clone());
        if period <= 0.0 {
            continue;
        }
        let mean = intervals.iter().sum::<f64>() / intervals.len() as f64;
        let var = intervals.iter().map(|d| (d - mean).powi(2)).sum::<f64>()
            / intervals.len() as f64;
        let cv = if mean > 0.0 { var.sqrt() / mean } else { 1.0 };
        let regularity = (1.0 - cv).clamp(0.0, 1.0);
        let evidence = (times.len() as f64 / 5.0).min(1.0);
        let confidence = (regularity * evidence) as f32;
        let last = *times.last().unwrap();
        let next_due = last + Duration::seconds((period * 86_400.0) as i64);
        rhythms.push(Rhythm {
            label: label.to_string(),
            period_days: period,
            count: times.len(),
            last,
            next_due,
            confidence,
        });
    }
    rhythms.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    rhythms
}

/// Rhythms whose next occurrence falls within `[now, now + horizon_days]`.
pub fn upcoming(rhythms: &[Rhythm], now: DateTime<Utc>, horizon_days: i64) -> Vec<&Rhythm> {
    let horizon = now + Duration::days(horizon_days);
    rhythms.iter().filter(|r| r.next_due >= now && r.next_due <= horizon).collect()
}

/// Rhythms overdue past their period — the "about to break" / slipping signal.
/// `tolerance` is a grace fraction of the period (0.5 = half a period late).
pub fn slipping(rhythms: &[Rhythm], now: DateTime<Utc>, tolerance: f64) -> Vec<&Rhythm> {
    rhythms
        .iter()
        .filter(|r| {
            let grace = Duration::seconds((r.period_days * tolerance * 86_400.0) as i64);
            now > r.next_due + grace
        })
        .collect()
}

/// Adapter: register each rhythm as a labeled coherence node and bridge it into
/// the shared graph. Returns the coherence ids. The label encodes the rhythm so
/// it's recallable by content (e.g. "rhythm: gym every 3.5d").
pub fn register_rhythms(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    rhythms: &[Rhythm],
) -> Vec<String> {
    let mut ids = Vec::with_capacity(rhythms.len());
    for r in rhythms {
        let text = format!("rhythm: {} every {:.1}d", r.label, r.period_days);
        let id = core.register_node_from_text(&text, embedder);
        core.bridge_node(&id);
        ids.push(id);
    }
    ids
}

/// Engine-side reduce: detect rhythms, register them, and elaborate a
/// temporal-coherence score (regular rhythms = coherent; slipping
/// rhythms = incoherent). Python Machine.reduce() delegates here (Phase-8).
pub fn reduce(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    events: &[TimedEvent],
    now: DateTime<Utc>,
) -> f32 {
    let rhythms = detect_rhythms(events);
    let mut score: f32 = 0.0;
    for r in &rhythms {
        let text = format!("rhythm: {} every {:.1}d", r.label, r.period_days);
        core.register_node_from_text(&text, embedder);
        core.bridge_node(&text);
        score += r.confidence; // regular, frequent rhythms add coherence
    }
    if !rhythms.is_empty() {
        score /= rhythms.len() as f32;
    }
    // a rhythm that is slipping is a coherence loss
    let slip = slipping(&rhythms, now, 0.5);
    if !slip.is_empty() {
        score -= slip.len() as f32 * 0.5;
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(label: &str, day_offset: i64) -> TimedEvent {
        TimedEvent {
            label: label.to_string(),
            at: DateTime::<Utc>::from_timestamp(day_offset * 86_400, 0).unwrap(),
        }
    }

    #[test]
    fn reduce_scores_regular_rhythms_positive() {
        let events: Vec<TimedEvent> = (0..5).map(|i| ev("standup", i * 7)).collect();
        let now = DateTime::<Utc>::from_timestamp(11 * 86_400, 0).unwrap();
        let s = reduce(&mut PhysisCore::new(), &RandomProjectionEmbedder::new(64), &events, now);
        assert!(s > 0.5, "regular rhythms should score positive (got {})", s);
    }

    #[test]
    fn reduce_penalises_slipping_rhythms() {
        let events: Vec<TimedEvent> = (0..4).map(|i| ev("water plants", i * 3)).collect();
        let embedder = RandomProjectionEmbedder::new(64);
        // same rhythms, evaluated on-time (day 11) vs overdue (day 20)
        let on_time = DateTime::<Utc>::from_timestamp(11 * 86_400, 0).unwrap();
        let overdue = DateTime::<Utc>::from_timestamp(20 * 86_400, 0).unwrap();
        let s_ok = reduce(&mut PhysisCore::new(), &embedder, &events, on_time);
        let s_late = reduce(&mut PhysisCore::new(), &embedder, &events, overdue);
        assert!(s_late < s_ok, "overdue rhythm should score LOWER than on-time ({} vs {})", s_late, s_ok);
    }

    #[test]
    fn detects_a_weekly_rhythm_and_predicts_next() {
        // "standup" every 7 days, 5 times.
        let events: Vec<TimedEvent> = (0..5).map(|i| ev("standup", i * 7)).collect();
        let rhythms = detect_rhythms(&events);
        assert_eq!(rhythms.len(), 1);
        let r = &rhythms[0];
        assert_eq!(r.label, "standup");
        assert!((r.period_days - 7.0).abs() < 0.01);
        assert_eq!(r.count, 5);
        // perfectly regular → high confidence
        assert!(r.confidence > 0.9, "confidence {}", r.confidence);
        // next due = last (day 28) + 7 = day 35
        let expected = DateTime::<Utc>::from_timestamp(35 * 86_400, 0).unwrap();
        assert_eq!(r.next_due, expected);
    }

    #[test]
    fn ignores_labels_with_too_few_occurrences() {
        let events = vec![ev("once", 0), ev("twice", 1), ev("twice", 8)];
        assert!(detect_rhythms(&events).is_empty());
    }

    #[test]
    fn upcoming_and_slipping_partition_by_due_date() {
        let events: Vec<TimedEvent> = (0..4).map(|i| ev("water plants", i * 3)).collect();
        let rhythms = detect_rhythms(&events); // period ~3d, last day 9, next ~day 12
        // now = day 11 → upcoming within 3 days
        let now = DateTime::<Utc>::from_timestamp(11 * 86_400, 0).unwrap();
        assert_eq!(upcoming(&rhythms, now, 3).len(), 1);
        assert!(slipping(&rhythms, now, 0.5).is_empty());
        // now = day 20 → way overdue → slipping
        let later = DateTime::<Utc>::from_timestamp(20 * 86_400, 0).unwrap();
        assert!(upcoming(&rhythms, later, 3).is_empty());
        assert_eq!(slipping(&rhythms, later, 0.5).len(), 1);
    }
}
