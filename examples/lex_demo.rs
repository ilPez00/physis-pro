//! lex demo — obligations, breach paths, unhandled contingencies.
//!   cargo run --example lex_demo

use physis_pro::machines::lex::{breach_paths, dangling_conditions, unconditional_obligations, Clause, Kind};

fn c(id: &str, kind: Kind, text: &str, requires: &[&str]) -> Clause {
    Clause { id: id.into(), kind, text: text.into(), requires: requires.iter().map(|s| s.to_string()).collect() }
}

fn main() {
    let contract = vec![
        c("deliver", Kind::Obligation, "Seller delivers goods by date", &[]),
        c("pay", Kind::Obligation, "Buyer pays within 30 days of delivery", &["delivered"]),
        c("delivered", Kind::Condition, "Goods received and signed for", &[]),
        c("refund", Kind::Obligation, "Seller refunds if returned in 14 days", &["returned"]),
        c("warranty", Kind::Right, "Buyer may claim warranty", &["registered"]),
    ];

    println!("lex — contract machine\nperceived {} clauses\n", contract.len());
    println!("ALWAYS DUE (unconditional obligations): {:?}", unconditional_obligations(&contract));
    println!("\nBREACH PATH if `delivered` fails: {:?}", breach_paths(&contract, &["delivered".into()]));
    println!("UNHANDLED CONTINGENCIES (required but undefined): {:?}", dangling_conditions(&contract));
}
