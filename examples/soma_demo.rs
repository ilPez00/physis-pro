//! soma demo — classify training sessions + detect drift toward overreaching.
//!   cargo run --example soma_demo

use physis_pro::machines::soma::{classify, drift, Session};

fn s(day: u32, load: f32, recovery: f32) -> Session {
    Session {
        day,
        activity: "training".into(),
        load,
        recovery,
    }
}

fn main() {
    let sessions = vec![
        s(1, 0.5, 0.9),
        s(2, 0.7, 0.8),
        s(3, 0.3, 0.9),
        s(4, 0.8, 0.6),
        s(5, 0.8, 0.5),
        s(6, 0.9, 0.4),
        s(7, 0.9, 0.35),
    ];

    println!(
        "soma — body machine\nperceived {} sessions\n",
        sessions.len()
    );
    println!("PER-SESSION (status → coherence):");
    for x in &sessions {
        let st = classify(x);
        println!(
            "  day {}  load {:.1} rec {:.1}  →  {:?} ({:+.0})",
            x.day,
            x.load,
            x.recovery,
            st,
            st.coherence()
        );
    }
    let d = drift(&sessions, 6);
    println!("\nDRIFT over last 6 sessions: {:+.2}", d);
    println!(
        "  {}",
        if d > 0.5 {
            "⚠ drifting toward overreaching — load up while recovery falls"
        } else if d < -0.2 {
            "recovering — load easing"
        } else {
            "stable"
        }
    );
}
