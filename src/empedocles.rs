//! The Empedoclean dyad as a dynamical node field (see `docs/EMPEDOCLES_DYAD.md`).
//!
//! Two coupled forces over a population of charged points:
//!   - **Eros** = attraction + generative synthesis (Love combines and begets).
//!   - **Nemesis** = repulsion + decay (Strife separates; mortality prunes).
//!
//! This is a RESEARCH module: pure, deterministic-seeded, and wired to nothing
//! on the production classify / web path (PREMORTEM P2). Its purpose is the
//! falsifiable experiment in `examples/empedocles_criticality.rs`: sweep the
//! attraction/repulsion ratio and check whether a middle band yields a
//! power-law cluster-size distribution — the signature of self-organised
//! criticality. If it does, the dyad does real organising work; if not, the
//! framing is poetry and we drop it.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

/// One charged point in the field. Position lives in the same kind of vector
/// space as a coherence-node embedding, so the field could later run on real
/// nodes; the experiment seeds it synthetically.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldNode {
    pub pos: Vec<f32>,
    /// Vitality. Reinforcement raises it; decay lowers it; death removes the node.
    pub charge: f32,
    /// Ticks since birth — diagnostics only.
    pub age: u64,
}

/// Force parameters. The attraction/repulsion ratio is the criticality tunable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DyadConfig {
    /// Eros: neighbours within this distance attract.
    pub eros_radius: f32,
    /// Eros: attraction step size.
    pub eros_strength: f32,
    /// Nemesis: neighbours inside this (smaller) distance repel.
    pub nemesis_radius: f32,
    /// Nemesis: repulsion step size.
    pub nemesis_strength: f32,
    /// Nemesis: fractional charge lost per tick (`charge *= 1 - decay`).
    pub decay: f32,
    /// Nemesis: nodes with charge below this die.
    pub death_charge: f32,
    /// Eros: a synthesis offspring is attempted between two high-charge nodes in
    /// the attraction band with this per-pair probability.
    pub synthesis_prob: f32,
    /// Eros: both parents must exceed this charge to beget.
    pub synthesis_min_charge: f32,
    /// Eros: gaussian mutation scale applied to an offspring's position.
    pub mutation: f32,
    /// Hard population cap (begetting stops here).
    pub max_nodes: usize,
    /// Velocity clamp: max displacement per node per tick. Summed forces over a
    /// dense neighbourhood can otherwise explode (overshoot → oscillation, never
    /// settling); a bounded step keeps integration stable, as in any particle
    /// sim. Set high to disable.
    pub max_step: f32,
}

impl Default for DyadConfig {
    fn default() -> Self {
        Self {
            eros_radius: 0.30,
            eros_strength: 0.10,
            nemesis_radius: 0.10,
            nemesis_strength: 0.05,
            decay: 0.01,
            death_charge: 0.05,
            synthesis_prob: 0.02,
            synthesis_min_charge: 0.8,
            mutation: 0.02,
            max_nodes: 2000,
            max_step: 0.05,
        }
    }
}

fn dist(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f32>().sqrt()
}

/// A population of nodes evolving under the dyad. Deterministic given its seed.
pub struct NodeField {
    pub nodes: Vec<FieldNode>,
    cfg: DyadConfig,
    rng: StdRng,
    dim: usize,
}

impl NodeField {
    /// Seed `n` nodes uniformly in the `dim`-dimensional unit cube, full charge.
    pub fn seed_uniform(n: usize, dim: usize, cfg: DyadConfig, seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let nodes = (0..n)
            .map(|_| FieldNode {
                pos: (0..dim).map(|_| rng.gen::<f32>()).collect(),
                charge: 1.0,
                age: 0,
            })
            .collect();
        Self { nodes, cfg, rng, dim }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Mean charge across the field — a coarse health/energy reading.
    pub fn mean_charge(&self) -> f32 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        self.nodes.iter().map(|n| n.charge).sum::<f32>() / self.nodes.len() as f32
    }

    /// Advance the field one tick: attraction + repulsion → move; decay → cull;
    /// synthesis → beget. Returns `(births, deaths)` for diagnostics.
    // Numeric kernel: forces cross-index nodes i and j of the same buffer, so
    // range indexing is clearer than (and not replaceable by) iterator zips.
    #[allow(clippy::needless_range_loop)]
    pub fn step(&mut self) -> (usize, usize) {
        let n = self.nodes.len();
        let mut delta = vec![vec![0.0f32; self.dim]; n];

        // Eros attraction + Nemesis repulsion. O(n^2): fine for the experiment's
        // few-thousand nodes; a grid index would scale it if ever needed.
        for i in 0..n {
            for j in (i + 1)..n {
                let d = dist(&self.nodes[i].pos, &self.nodes[j].pos);
                if d < 1e-6 {
                    continue;
                }
                if d < self.cfg.eros_radius {
                    // Pull i and j toward each other, weighted by the other's charge.
                    let wi = self.cfg.eros_strength * self.nodes[j].charge;
                    let wj = self.cfg.eros_strength * self.nodes[i].charge;
                    for k in 0..self.dim {
                        let dir = (self.nodes[j].pos[k] - self.nodes[i].pos[k]) / d;
                        delta[i][k] += dir * wi;
                        delta[j][k] -= dir * wj;
                    }
                }
                if d < self.cfg.nemesis_radius {
                    // Push apart, inverse-square — collapse-prevention.
                    let f = self.cfg.nemesis_strength / (d * d);
                    for k in 0..self.dim {
                        let dir = (self.nodes[j].pos[k] - self.nodes[i].pos[k]) / d;
                        delta[i][k] -= dir * f;
                        delta[j][k] += dir * f;
                    }
                }
            }
        }
        let max_step = self.cfg.max_step;
        for (node, dl) in self.nodes.iter_mut().zip(&delta) {
            // Velocity clamp: scale the displacement down if it exceeds max_step.
            let mag = dl.iter().map(|x| x * x).sum::<f32>().sqrt();
            let scale = if mag > max_step && mag > 1e-9 { max_step / mag } else { 1.0 };
            for k in 0..self.dim {
                node.pos[k] += dl[k] * scale;
            }
            node.age += 1;
            node.charge *= 1.0 - self.cfg.decay;
        }

        // Eros synthesis: beget offspring between high-charge attraction-band
        // pairs. Collect first (can't push while iterating), then append.
        let mut offspring: Vec<FieldNode> = Vec::new();
        if self.nodes.len() < self.cfg.max_nodes {
            for i in 0..self.nodes.len() {
                for j in (i + 1)..self.nodes.len() {
                    if self.nodes[i].charge < self.cfg.synthesis_min_charge
                        || self.nodes[j].charge < self.cfg.synthesis_min_charge
                    {
                        continue;
                    }
                    let d = dist(&self.nodes[i].pos, &self.nodes[j].pos);
                    // In the attraction band but not collapsed: distinct enough
                    // to produce a novel synthesis, close enough to be drawn.
                    if d >= self.cfg.nemesis_radius
                        && d < self.cfg.eros_radius
                        && self.rng.gen::<f32>() < self.cfg.synthesis_prob
                    {
                        let pos = (0..self.dim)
                            .map(|k| {
                                let mid = 0.5 * (self.nodes[i].pos[k] + self.nodes[j].pos[k]);
                                mid + (self.rng.gen::<f32>() - 0.5) * self.cfg.mutation
                            })
                            .collect();
                        let charge = 0.5 * (self.nodes[i].charge + self.nodes[j].charge);
                        offspring.push(FieldNode { pos, charge, age: 0 });
                        if self.nodes.len() + offspring.len() >= self.cfg.max_nodes {
                            break;
                        }
                    }
                }
                if self.nodes.len() + offspring.len() >= self.cfg.max_nodes {
                    break;
                }
            }
        }
        let births = offspring.len();
        self.nodes.extend(offspring);

        // Nemesis mortality: cull the faded.
        let before = self.nodes.len();
        let death = self.cfg.death_charge;
        self.nodes.retain(|n| n.charge >= death);
        let deaths = before - self.nodes.len();

        (births, deaths)
    }

    /// Reinforce the nearest node to `pos` (perception feeding the field): boost
    /// its charge and draw it slightly toward the input. No-op on an empty field.
    #[allow(clippy::needless_range_loop)]
    pub fn reinforce(&mut self, pos: &[f32], boost: f32) {
        let Some((idx, _)) = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (i, dist(&n.pos, pos)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        else {
            return;
        };
        let node = &mut self.nodes[idx];
        node.charge = (node.charge + boost).min(1.0);
        for k in 0..self.dim.min(pos.len()) {
            node.pos[k] += 0.1 * (pos[k] - node.pos[k]);
        }
    }

    /// Connected-component clustering at `link_dist`: two nodes share a cluster
    /// if within `link_dist`. Returns cluster sizes, descending — the input to
    /// the criticality (power-law) check.
    pub fn cluster_sizes(&self, link_dist: f32) -> Vec<usize> {
        let n = self.nodes.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(parent: &mut [usize], x: usize) -> usize {
            let mut r = x;
            while parent[r] != r {
                r = parent[r];
            }
            let mut c = x;
            while parent[c] != c {
                let next = parent[c];
                parent[c] = r;
                c = next;
            }
            r
        }
        for i in 0..n {
            for j in (i + 1)..n {
                if dist(&self.nodes[i].pos, &self.nodes[j].pos) <= link_dist {
                    let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                    if ri != rj {
                        parent[ri] = rj;
                    }
                }
            }
        }
        let mut counts: std::collections::HashMap<usize, usize> = Default::default();
        for i in 0..n {
            let r = find(&mut parent, i);
            *counts.entry(r).or_default() += 1;
        }
        let mut sizes: Vec<usize> = counts.into_values().collect();
        sizes.sort_unstable_by(|a, b| b.cmp(a));
        sizes
    }
}

/// Coarse regime classifier for a cluster-size distribution. Not a rigorous
/// power-law fit — a cheap signal for the sweep: one dominant cluster =
/// supercritical (Love won); all singletons = subcritical (Strife won);
/// a heavy spread of sizes = the critical band worth a real fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Regime {
    Subcritical,
    Critical,
    Supercritical,
}

/// Classify `(regime, largest_fraction)` from cluster sizes over `total` nodes.
pub fn regime(sizes: &[usize], total: usize) -> (Regime, f32) {
    if total == 0 || sizes.is_empty() {
        return (Regime::Subcritical, 0.0);
    }
    let largest = sizes[0] as f32 / total as f32;
    let singleton_frac =
        sizes.iter().filter(|&&s| s == 1).count() as f32 / sizes.len() as f32;
    let r = if largest > 0.6 {
        Regime::Supercritical
    } else if singleton_frac > 0.9 {
        Regime::Subcritical
    } else {
        Regime::Critical
    };
    (r, largest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pure_attraction_collapses_supercritical() {
        // Eros across the whole space, no Nemesis (no repulsion, no decay) →
        // everything coalesces into one cluster. Velocity-clamped so it settles
        // rather than oscillating.
        let cfg = DyadConfig {
            eros_radius: 2.0,
            eros_strength: 0.10,
            nemesis_radius: 0.0,
            nemesis_strength: 0.0,
            decay: 0.0,
            death_charge: 0.0,
            synthesis_prob: 0.0,
            max_step: 0.05,
            ..Default::default()
        };
        let mut f = NodeField::seed_uniform(80, 2, cfg, 1);
        for _ in 0..400 {
            f.step();
        }
        let sizes = f.cluster_sizes(0.1);
        let (r, largest) = regime(&sizes, f.len());
        assert_eq!(r, Regime::Supercritical, "largest fraction {largest}");
    }

    #[test]
    fn test_pure_repulsion_scatters_subcritical() {
        // Strong Nemesis repulsion, no Eros attraction → everything separates.
        let cfg = DyadConfig {
            eros_radius: 0.0,
            eros_strength: 0.0,
            nemesis_radius: 2.0,
            nemesis_strength: 0.05,
            decay: 0.0,
            death_charge: 0.0,
            synthesis_prob: 0.0,
            ..Default::default()
        };
        let mut f = NodeField::seed_uniform(80, 2, cfg, 2);
        for _ in 0..60 {
            f.step();
        }
        let sizes = f.cluster_sizes(0.05);
        let (r, _) = regime(&sizes, f.len());
        assert_eq!(r, Regime::Subcritical);
    }

    #[test]
    fn test_decay_kills_unreinforced_nodes() {
        // Nemesis mortality: with decay and no reinforcement the field empties.
        let cfg = DyadConfig {
            eros_strength: 0.0,
            nemesis_strength: 0.0,
            decay: 0.5,
            death_charge: 0.1,
            synthesis_prob: 0.0,
            ..Default::default()
        };
        let mut f = NodeField::seed_uniform(50, 2, cfg, 3);
        let mut total_deaths = 0;
        for _ in 0..20 {
            total_deaths += f.step().1;
        }
        assert!(f.is_empty(), "decay must cull an unreinforced field");
        assert_eq!(total_deaths, 50);
    }

    #[test]
    fn test_reinforcement_keeps_a_node_alive() {
        // Eros perception vs Nemesis decay: a continually-reinforced node survives.
        let cfg = DyadConfig {
            eros_strength: 0.0,
            nemesis_strength: 0.0,
            decay: 0.2,
            death_charge: 0.1,
            synthesis_prob: 0.0,
            ..Default::default()
        };
        let mut f = NodeField::seed_uniform(10, 2, cfg, 4);
        let target = f.nodes[0].pos.clone();
        for _ in 0..40 {
            f.reinforce(&target, 0.5);
            f.step();
        }
        assert!(!f.is_empty(), "a reinforced node must outlast decay");
    }

    #[test]
    fn test_synthesis_begets_offspring() {
        // Two charged nodes a synthesis-band apart, no movement/decay → births.
        let cfg = DyadConfig {
            eros_radius: 0.5,
            eros_strength: 0.0,
            nemesis_radius: 0.05,
            nemesis_strength: 0.0,
            decay: 0.0,
            death_charge: 0.0,
            synthesis_prob: 1.0,
            synthesis_min_charge: 0.5,
            mutation: 0.01,
            max_nodes: 100,
            max_step: 0.05,
        };
        let mut f = NodeField::seed_uniform(2, 2, cfg, 5);
        // Place them 0.2 apart (between nemesis_radius and eros_radius).
        f.nodes[0].pos = vec![0.4, 0.5];
        f.nodes[1].pos = vec![0.6, 0.5];
        let (births, _) = f.step();
        assert!(births >= 1, "high-charge banded pair must beget");
        assert!(f.len() >= 3);
    }
}
