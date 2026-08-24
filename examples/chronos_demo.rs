//! chronos demo — run the temporal machine on a realistic event log and print
//! what it found: rhythms, what's due soon, what's slipping.
//!
//!   cargo run --example chronos_demo
//!
//! Output is captured into docs/machines/samples/chronos.md as a demonstration.

use chrono::{DateTime, Duration, Utc};
use physis_pro::machines::chronos::{detect_rhythms, slipping, upcoming, TimedEvent};

/// A fixed "today" so the demo is deterministic.
fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-06-06T09:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

/// Helper: an event `days_ago` before `now()`.
fn ago(label: &str, days_ago: i64) -> TimedEvent {
    TimedEvent {
        label: label.to_string(),
        at: now() - Duration::days(days_ago),
    }
}

fn main() {
    // A plausible month of life-log events.
    let events = vec![
        // gym — steady, ~every 2 days (regular)
        ago("gym", 28),
        ago("gym", 26),
        ago("gym", 24),
        ago("gym", 22),
        ago("gym", 20),
        ago("gym", 18),
        ago("gym", 16),
        ago("gym", 14),
        ago("gym", 12),
        ago("gym", 10),
        ago("gym", 8),
        ago("gym", 6),
        ago("gym", 4),
        ago("gym", 2),
        // team standup — weekly, but the last one was 11 days ago (slipping)
        ago("standup", 32),
        ago("standup", 25),
        ago("standup", 18),
        ago("standup", 11),
        // pay rent — monthly, due about now
        ago("pay rent", 62),
        ago("pay rent", 31),
        // call mum — irregular (low confidence)
        ago("call mum", 21),
        ago("call mum", 9),
        ago("call mum", 2),
        // one-offs — should NOT become rhythms
        ago("dentist", 13),
        ago("flight booked", 5),
    ];

    let rhythms = detect_rhythms(&events);
    let t = now();

    println!("chronos — temporal machine");
    println!(
        "perceived {} events · now = {}\n",
        events.len(),
        t.format("%Y-%m-%d")
    );

    println!("RHYTHMS (recurrence primitives, by confidence):");
    for r in &rhythms {
        println!(
            "  • {:<14} every {:>4.1}d  ×{:<2}  conf {:.2}  next {}",
            r.label,
            r.period_days,
            r.count,
            r.confidence,
            r.next_due.format("%Y-%m-%d"),
        );
    }

    println!("\nDUE in the next 3 days:");
    let up = upcoming(&rhythms, t, 3);
    if up.is_empty() {
        println!("  (nothing)");
    }
    for r in up {
        println!("  → {:<14} next {}", r.label, r.next_due.format("%Y-%m-%d"));
    }

    println!("\nSLIPPING (overdue past a half-period — surface before it breaks):");
    let slip = slipping(&rhythms, t, 0.5);
    if slip.is_empty() {
        println!("  (nothing)");
    }
    for r in slip {
        let overdue = (t - r.next_due).num_days();
        println!(
            "  ⚠ {:<14} {}d overdue (period {:.1}d)",
            r.label, overdue, r.period_days
        );
    }
}
