//! The falsifier for the Empedoclean dyad (docs/EMPEDOCLES_DYAD.md §3).
//!
//! Sweeps the Eros(attraction)/Nemesis(repulsion) ratio and prints, per setting,
//! the regime and cluster-size distribution after the field settles. The thesis
//! predicts: extremes give one-giant-cluster (supercritical, Love won) and
//! all-singletons (subcritical, Strife won); a MIDDLE band gives a heavy-tailed
//! (power-law-ish) cluster-size spread — self-organised criticality. If no such
//! band appears, the framing is poetry and we drop it.
//!
//! Run: `cargo run --release --example empedocles_criticality`

use physis_pro::empedocles::{regime, DyadConfig, NodeField};

fn main() {
    println!("Empedoclean dyad — criticality sweep");
    println!("Strife (repulsion) controls node spacing, so sweep IT against a fixed");
    println!("Love (attraction). nemesis=0 → Love collapses the field; nemesis high →");
    println!("Strife scatters it. The critical percolation band, if real, is between.\n");
    println!(
        "{:>6} {:>8} {:>6} {:>13} {:>8}  cluster sizes (desc, top 10)",
        "eros", "nemesis", "nodes", "regime", "largest"
    );

    // Cluster link distance is the interaction scale: nodes that can attract
    // (within eros_radius) and end up that close count as one cluster. Below
    // this the measurement only sees collapsed pairs (the earlier artifact).
    let link_dist = 0.15f32;
    let eros = 0.10f32;
    for &nemesis in &[0.0, 0.001, 0.003, 0.006, 0.01, 0.02, 0.04, 0.08, 0.16] {
        let cfg = DyadConfig {
            eros_radius: 0.30,
            eros_strength: eros,
            nemesis_radius: 0.20,
            nemesis_strength: nemesis,
            // Decay + synthesis off: isolate the attraction/repulsion balance,
            // which is the criticality variable.
            decay: 0.0,
            death_charge: 0.0,
            synthesis_prob: 0.0,
            max_step: 0.03,
            ..Default::default()
        };
        let mut field = NodeField::seed_uniform(300, 2, cfg, 42);
        for _ in 0..400 {
            field.step();
        }
        let sizes = field.cluster_sizes(link_dist);
        let (r, largest) = regime(&sizes, field.len());
        let head: Vec<String> = sizes.iter().take(10).map(|s| s.to_string()).collect();
        println!(
            "{eros:>6.2} {nemesis:>8.3} {:>6} {:>13?} {largest:>8.2}  {}",
            field.len(),
            r,
            head.join(" ")
        );
    }

    println!(
        "\nRead: 'largest' near 1.0 = Love collapsed the field (supercritical); \
         all 1s = Strife scattered it (subcritical); a graded spread of cluster \
         sizes in a middle row = the critical band the thesis predicts."
    );
}
