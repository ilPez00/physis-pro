//! polis demo — ties, reciprocity, strengthening vs fading.
//!   cargo run --example polis_demo

use physis_pro::machines::polis::{fading, strengthening, ties, Interaction};

fn i(from: &str, to: &str, w: f32) -> Interaction {
    Interaction {
        from: from.into(),
        to: to.into(),
        weight: w,
    }
}

fn main() {
    let interactions = vec![
        i("you", "ana", 9.0),
        i("ana", "you", 8.0), // warm, mutual
        i("you", "ben", 7.0),
        i("ben", "you", 1.0), // you carry it
        i("you", "cleo", 2.0),
        i("cleo", "you", 2.0), // light, balanced
        i("ana", "ben", 4.0),
        i("ben", "ana", 4.0),
    ];

    println!(
        "polis — social-graph machine\nperceived {} interactions\n",
        interactions.len()
    );
    let t = ties(&interactions);
    println!("TIES (strength · reciprocity):");
    for x in &t {
        println!(
            "  {:<4}–{:<4}  strength {:>4.1}  recip {:.2}",
            x.a, x.b, x.strength, x.reciprocity
        );
    }
    println!("\nSTRENGTHENING (strong + mutual):");
    for x in strengthening(&t, 8.0, 0.7) {
        println!("  ✓ {}–{}", x.a, x.b);
    }
    println!("\nFADING (one-sided — tend before it lapses):");
    for x in fading(&t, 0.5) {
        println!("  ⚠ {}–{}  (recip {:.2})", x.a, x.b, x.reciprocity);
    }
}
