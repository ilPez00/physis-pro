//! plutos — the economic machine.
//!
//! Grounding: Aristotle's *chrematistics* — the distinction between use-value and
//! exchange-value, and value grasped as a *ratio* (a price is an interval, as in
//! `harmony`). The primitive is not a single transaction but a **value flow**: a
//! *recurring* directed exchange between two parties. A one-off spend is noise; a
//! flow that repeats is the elaborable unit. Composing flows gives each party a
//! balance (inflow − outflow); the dream surfaces the unnoticed recurring drain
//! and flags the party that is bleeding (net < 0).
//!
//! Pure + testable: `detect_flows` is engine-free; only `register_flows` touches
//! the core. Same shape as `polis` / the eros pair.

use std::collections::HashMap;

use crate::core::PhysisCore;
use crate::embed::VectorEmbed;

/// One directed transfer of value (from payer to payee), e.g. an invoice line or
/// an expense. `from`/`to` may be people, accounts, or categories.
#[derive(Debug, Clone)]
pub struct Transaction {
    pub from: String,
    pub to: String,
    pub amount: f32,
}

/// A recurring value flow between two parties — the economic primitive.
#[derive(Debug, Clone, PartialEq)]
pub struct Flow {
    pub from: String,
    pub to: String,
    /// Total value moved over all observed transactions.
    pub total: f32,
    /// Number of transactions (the recurrence evidence).
    pub count: usize,
    /// Mean transaction size.
    pub mean: f32,
    pub hash: String,
}

/// Net position of one party across all flows.
#[derive(Debug, Clone, PartialEq)]
pub struct Balance {
    pub party: String,
    pub inflow: f32,
    pub outflow: f32,
    /// inflow − outflow. Negative = bleeding value.
    pub net: f32,
}

/// Reduce transactions to recurring value flows. A `(from → to)` pair becomes a
/// flow only when it recurs at least `min_count` times; the one-off spend is
/// rejected as noise (chrematistic value is a *ratio* that repeats, not a single
/// exchange).
pub fn detect_flows(txns: &[Transaction], min_count: usize) -> Vec<Flow> {
    let min = min_count.max(1);
    let mut agg: HashMap<(String, String), (f32, usize)> = HashMap::new();
    for t in txns {
        if t.amount <= 0.0 {
            continue;
        }
        let e = agg.entry((t.from.clone(), t.to.clone())).or_insert((0.0, 0));
        e.0 += t.amount;
        e.1 += 1;
    }
    let mut out: Vec<Flow> = agg
        .into_iter()
        .filter(|(_, (_, count))| *count >= min)
        .map(|((from, to), (total, count))| {
            let mean = total / count as f32;
            let hash = format!("plutos_{from}_{to}");
            Flow { from, to, total, count, mean, hash }
        })
        .collect();
    out.sort_by(|a, b| b.total.partial_cmp(&a.total).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// The largest recurring outflows — the costs worth seeing. `min_total` filters
/// the trivial. This is the dream's "unnoticed recurring drain".
pub fn drains(flows: &[Flow], min_total: f32) -> Vec<&Flow> {
    flows.iter().filter(|f| f.total >= min_total).collect()
}

/// Per-party net position across all flows.
pub fn balances(flows: &[Flow]) -> Vec<Balance> {
    let mut io: HashMap<String, (f32, f32)> = HashMap::new();
    for f in flows {
        io.entry(f.from.clone()).or_insert((0.0, 0.0)).1 += f.total; // outflow
        io.entry(f.to.clone()).or_insert((0.0, 0.0)).0 += f.total; // inflow
    }
    let mut out: Vec<Balance> = io
        .into_iter()
        .map(|(party, (inflow, outflow))| Balance { party, inflow, outflow, net: inflow - outflow })
        .collect();
    out.sort_by(|a, b| a.net.partial_cmp(&b.net).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Parties whose net is negative — bleeding value (the projected breach).
pub fn breaches(balances: &[Balance]) -> Vec<&Balance> {
    balances.iter().filter(|b| b.net < 0.0).collect()
}

/// Adapter: register each flow as a bridged node and a weighted edge between the
/// two parties.
pub fn register_flows(
    core: &mut PhysisCore,
    embedder: &dyn VectorEmbed,
    flows: &[Flow],
) -> Vec<String> {
    let mut ids = Vec::new();
    for f in flows {
        let from = core.register_node_from_text(&format!("party: {}", f.from), embedder);
        let to = core.register_node_from_text(&format!("party: {}", f.to), embedder);
        core.add_edge_between(&from, &to, f.total, 1);
        let flow = core.register_node_from_text(
            &format!("flow: {} -> {} ({:.0} over {})", f.from, f.to, f.total, f.count),
            embedder,
        );
        core.bridge_node(&flow);
        ids.push(flow);
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(from: &str, to: &str, amount: f32) -> Transaction {
        Transaction { from: from.into(), to: to.into(), amount }
    }

    #[test]
    fn recurring_flow_detected_one_off_rejected() {
        let txns = vec![
            t("me", "rent", 1000.0), t("me", "rent", 1000.0), t("me", "rent", 1000.0), // recurring
            t("me", "gift", 50.0),                                                      // one-off
            t("salary", "me", 2500.0), t("salary", "me", 2500.0),                       // recurring inflow
        ];
        let flows = detect_flows(&txns, 2);
        assert!(flows.iter().any(|f| f.from == "me" && f.to == "rent" && f.count == 3 && f.total == 3000.0));
        assert!(flows.iter().any(|f| f.from == "salary" && f.to == "me" && f.count == 2));
        // the one-off gift is noise
        assert!(!flows.iter().any(|f| f.to == "gift"));
        // largest drain
        assert_eq!(drains(&flows, 2000.0).len(), 2); // rent (3000) + salary->me (5000)
    }

    #[test]
    fn balances_and_breach() {
        let txns = vec![
            t("salary", "me", 2000.0), t("salary", "me", 2000.0),  // me +4000
            t("me", "rent", 1500.0), t("me", "rent", 1500.0),       // me -3000
            t("me", "food", 800.0), t("me", "food", 800.0),         // me -1600
        ];
        let flows = detect_flows(&txns, 2);
        let bals = balances(&flows);
        let me = bals.iter().find(|b| b.party == "me").unwrap();
        assert_eq!(me.inflow, 4000.0);
        assert_eq!(me.outflow, 4600.0); // 3000 + 1600
        assert!((me.net - (-600.0)).abs() < 1e-3);
        // me is bleeding
        assert!(breaches(&bals).iter().any(|b| b.party == "me"));
        // rent/food only receive → not breaching
        assert!(!breaches(&bals).iter().any(|b| b.party == "rent"));
    }
}
