//! What does an unrelated command actually score?
//!
//! Item B asked for a relevance floor and the sweep found no setting that
//! does anything. This prints the number the sweep could not: the cosine a
//! command with nothing to warn about gets from its best claim, beside the
//! cosine a command with a real refutation gets. A floor can only exist if
//! those two distributions separate.
fn main() {
    let (embedder, kind) = physis_core::embed::select(384);
    println!("embedder {kind}\n");
    let core = physis_pro::ported::act_recall::demo_ledger(embedder.as_ref());
    let show = |label: &str, cmds: &[&str]| {
        let mut all: Vec<f32> = Vec::new();
        for c in cmds {
            let b = physis_pro::ported::act::bearing_on(&core, c, embedder.as_ref(), 5);
            let lead = b.first().map(|x| x.relevance).unwrap_or(0.0);
            let fifth = b.get(4).map(|x| x.relevance).unwrap_or(0.0);
            all.push(lead);
            println!("  {lead:.3}  (5th {fifth:.3})  {c}");
        }
        all.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "  {label}: min {:.3}  median {:.3}  max {:.3}\n",
            all[0],
            all[all.len() / 2],
            all[all.len() - 1]
        );
    };
    println!("commands WITH a refutation in the ledger:");
    show("with", &physis_pro::ported::act_recall::pair_commands());
    println!("commands with NOTHING to warn about:");
    show(
        "without",
        &physis_pro::ported::act_recall::unrelated_commands(),
    );
}
