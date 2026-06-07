//! harmony demo — intervals, motifs, variations.
//!   cargo run --example harmony_demo

use physis_pro::machines::harmony::{intervals, invert, motifs, transpose};

fn main() {
    // A little melody (MIDI pitches) with a repeating "up2 up2 down1" shape.
    let melody = vec![60, 62, 64, 63, 65, 67, 69, 68, 67];

    println!("harmony — music machine\nperceived {} notes\n", melody.len());
    println!("INTERVALS (key-invariant shape): {:?}", intervals(&melody));

    println!("\nMOTIFS (recurring interval runs, len 3):");
    for (seq, count) in motifs(&melody, 3) {
        println!("  {:?}  ×{}", seq, count);
    }

    println!("\nVARIATIONS of the opening motif [60,62,64]:");
    println!("  transpose +5: {:?}", transpose(&[60, 62, 64], 5));
    println!("  invert:       {:?}", invert(&[60, 62, 64]));
}
