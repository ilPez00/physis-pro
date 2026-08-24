//! elenchus demo — evaluate an argument's claims (supported / contested /
//! unsupported).  cargo run --example elenchus_demo

use physis_pro::machines::elenchus::{
    contested_claims, evaluate, unsupported_claims, Kind, Statement,
};

fn st(id: &str, kind: Kind, text: &str, target: Option<&str>) -> Statement {
    Statement {
        id: id.into(),
        kind,
        text: text.into(),
        target: target.map(|s| s.into()),
    }
}

fn main() {
    let arg = vec![
        st("c1", Kind::Claim, "Remote work raises output", None),
        st(
            "w1",
            Kind::Warrant,
            "Fewer commute hours → more focus time",
            Some("c1"),
        ),
        st("c2", Kind::Claim, "Open offices boost collaboration", None),
        st(
            "w2",
            Kind::Warrant,
            "Proximity → more chance encounters",
            Some("c2"),
        ),
        st(
            "r2",
            Kind::Rebuttal,
            "Studies show open offices cut face-to-face 70%",
            Some("c2"),
        ),
        st(
            "c3",
            Kind::Claim,
            "Four-day weeks are universally better",
            None,
        ),
    ];

    println!(
        "elenchus — dialogue machine\nperceived {} statements\n",
        arg.len()
    );
    println!("CLAIM STANDING:");
    for (id, st) in evaluate(&arg) {
        let claim = arg.iter().find(|s| s.id == id).unwrap();
        println!("  {:<3} {:<12?}  {}", id, st, claim.text);
    }
    println!(
        "\nPRESS (unsupported — ask for a warrant): {:?}",
        unsupported_claims(&arg)
    );
    println!(
        "RECALIBRATE (contested by a rebuttal):   {:?}",
        contested_claims(&arg)
    );
}
