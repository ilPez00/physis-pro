// provenance: moved from physis-core examples/claim_identity.rs
// (core HEAD d65858b) by the 2026-09-14 Core/Product split: it runs over the
// act ledger corpus, which left Core with act_recall. The claim_identity
// mechanism itself stays in Core with its own tests.
//! C2 — does a structural identity beat the sentence at finding contradictions?
//!
//! Scores both identities over every pair of the twin ledger, where the eight
//! refutation/endorsement pairs are the known contradictions and everything
//! else is a negative. See `src/claim_identity.rs` for what the comparison can
//! and cannot say.
fn main() {
    let (embedder, kind) = physis_core::embed::select(384);
    let (claims, truth) = physis_pro::ported::act_recall::twin_pairs_corpus(embedder.as_ref());
    let run = physis_core::claim_identity::run(&claims, &truth, kind);
    print!("{}", run.render());
    let path = std::path::Path::new("benchmarks/results/claim-identity.json");
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    std::fs::write(path, serde_json::to_vec_pretty(&run).unwrap()).unwrap();
    println!("\nartifact -> {}", path.display());
}
