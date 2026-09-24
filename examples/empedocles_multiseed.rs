//! Multi-seed criticality sweep for the Empedoclean dyad.
//!
//! The single-seed example (`empedocles_criticality`) cannot support the
//! paper's kill test: a power-law claim needs several seeds and a
//! distribution fit. This runs the same sweep over `SEEDS` seeds per
//! nemesis setting and emits JSON on stdout so the fit is done in Python
//! where the statistics library lives.
//!
//! Run: cargo run --release --example empedocles_multiseed -- 16

use physis_pro::empedocles::{regime, DyadConfig, NodeField};

const NODES: usize = 300;
const DIMS: usize = 2;
const TICKS: usize = 400;
const LINK_DIST: f32 = 0.15;
const EROS: f32 = 0.10;
const NEMESIS: [f32; 9] = [0.0, 0.001, 0.003, 0.006, 0.01, 0.02, 0.04, 0.08, 0.16];

fn main() {
    let seeds: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(16);

    println!("[");
    let mut first_run = true;
    for &nemesis in &NEMESIS {
        let cfg = DyadConfig {
            eros_radius: 0.30,
            eros_strength: EROS,
            nemesis_radius: 0.20,
            nemesis_strength: nemesis,
            decay: 0.0,
            death_charge: 0.0,
            synthesis_prob: 0.0,
            max_step: 0.03,
            ..Default::default()
        };
        for seed in 0..seeds {
            let mut field = NodeField::seed_uniform(NODES, DIMS, cfg.clone(), seed as u64);
            for _ in 0..TICKS {
                field.step();
            }
            let sizes = field.cluster_sizes(LINK_DIST);
            let (r, largest) = regime(&sizes, field.len());
            let json_sizes: Vec<String> = sizes.iter().map(|s| s.to_string()).collect();
            if !first_run {
                println!(",");
            }
            first_run = false;
            print!(
                "  {{\"nemesis\": {nemesis}, \"seed\": {seed}, \"nodes\": {}, \
                 \"regime\": \"{r:?}\", \"largest\": {largest:.6}, \
                 \"sizes\": [{}]}}",
                field.len(),
                json_sizes.join(",")
            );
        }
    }
    println!("\n]");
}
