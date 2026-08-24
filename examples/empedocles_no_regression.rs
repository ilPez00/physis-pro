//! Gate 2 for the Empedoclean dyad (docs/EMPEDOCLES_DYAD.md §5): the
//! no-regression check. Criticality (gate 1) proved the dynamics organise; this
//! proves that running them does not damage what the engine is for —
//! classification. Three separated classes are seeded as labelled nodes; held-out
//! points are classified by the nearest labelled node before and after the dyad
//! runs (decay + synthesis active, perception reinforcing each class every tick).
//! If post-dyad accuracy holds at the static baseline, decay-as-eviction and
//! synthesis are safe to wire into the live core.
//!
//! Run: `cargo run --release --example empedocles_no_regression`

use physis_pro::empedocles::{DyadConfig, NodeField};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn main() {
    let centroids = [[0.12f32, 0.12], [0.88, 0.12], [0.50, 0.88]];
    let mut rng = StdRng::seed_from_u64(7);
    let mut jitter = |c: &[f32; 2]| {
        vec![
            c[0] + (rng.random::<f32>() - 0.5) * 0.06,
            c[1] + (rng.random::<f32>() - 0.5) * 0.06,
        ]
    };
    let mut train: Vec<(Vec<f32>, u32)> = Vec::new();
    let mut test: Vec<(Vec<f32>, u32)> = Vec::new();
    for (cls, c) in centroids.iter().enumerate() {
        for _ in 0..12 {
            train.push((jitter(c), cls as u32));
        }
        for _ in 0..10 {
            test.push((jitter(c), cls as u32));
        }
    }
    let n_test = test.len();

    let accuracy = |field: &NodeField| -> f32 {
        let ok = test
            .iter()
            .filter(|(p, y)| field.classify_nearest_label(p) == Some(*y))
            .count();
        ok as f32 / n_test as f32
    };

    let base = NodeField::from_labeled(train.clone(), DyadConfig::default(), 1);
    let base_acc = accuracy(&base);

    println!("Empedoclean dyad — no-regression gate (3 classes, {n_test} held-out)\n");
    println!("static baseline (nearest labelled node): {base_acc:.3}\n");
    println!(
        "{:>6} {:>10} {:>7} {:>9} {:>9}  verdict",
        "decay", "synthesis", "alive", "dyad acc", "vs base"
    );

    for &(decay, synth) in &[
        (0.0, 0.0),
        (0.01, 0.02),
        (0.02, 0.05),
        (0.05, 0.05),
        (0.10, 0.05),
    ] {
        let cfg = DyadConfig {
            eros_radius: 0.30,
            eros_strength: 0.05,
            nemesis_radius: 0.05,
            nemesis_strength: 0.01,
            decay,
            death_charge: 0.10,
            synthesis_prob: synth,
            synthesis_min_charge: 0.8,
            mutation: 0.01,
            max_nodes: 1000,
            max_step: 0.05,
        };
        let mut field = NodeField::from_labeled(train.clone(), cfg, 2);
        for _ in 0..120 {
            for c in &centroids {
                field.reinforce(c, 0.5);
            }
            field.step();
        }
        let acc = accuracy(&field);
        let verdict = if acc + 1e-6 >= base_acc {
            "OK (no regression)"
        } else {
            "REGRESSION"
        };
        println!(
            "{decay:>6.2} {synth:>10.2} {:>7} {acc:>9.3} {:>+9.3}  {verdict}",
            field.len(),
            acc - base_acc
        );
    }

    println!("\nGate 2 passes iff every row holds at the static baseline: decay and");
    println!("synthesis prune and beget without losing the class structure — so");
    println!("decay-as-eviction is safe to wire into the live core first.");
}
