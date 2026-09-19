//! Declarative agent route table (Phase 19, B1).
//!
//! Semantic-router parity with one thing Semantic Router cannot do: **the table
//! learns**. A route binds a semantic cell to an action a harness should take.
//! Routes are data — a JSON document in the data dir — never code, under the
//! same composition-as-data rule the notebook registry uses: a shared route
//! table cannot make a harness execute something the engine did not ship.
//!
//! Matching is deliberately thin: the classifier has already resolved
//! ambiguity and context (overlays, salience, rerank), so the route lookup is a
//! cell-key match plus per-route example disambiguation. Misrouted text is
//! meant to be fed back into `QualityTracker` by the caller, which is the half
//! Semantic Router is missing.

use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::Path;

/// A declarative route: a semantic cell bound to an action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentRoute {
    /// Unique route name a harness can branch on.
    pub name: String,
    /// Display-form cell this route is bound to, e.g. `"FABRICATE/WORK"`.
    /// DOMAIN/MODE, both halves non-empty.
    pub cell: String,
    /// What a harness should do when routed here.
    pub action: String,
    /// Example inputs that should land on this route. Used to disambiguate
    /// within a cell, and to seed future example-based matching.
    #[serde(default)]
    pub examples: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default)]
pub struct RouteTable {
    routes: Vec<AgentRoute>,
}

impl RouteTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Report **every** validation problem at once, so an editor gets one
    /// round-trip instead of one error per attempt.
    pub fn validate(route: &AgentRoute) -> Vec<String> {
        let mut errs = Vec::new();
        if route.name.trim().is_empty() {
            errs.push("name must not be empty".into());
        }
        let cell = route.cell.trim();
        if cell.is_empty() {
            errs.push("cell must not be empty".into());
        } else {
            let halves: Vec<&str> = cell.split('/').collect();
            if halves.len() != 2 || halves[0].trim().is_empty() || halves[1].trim().is_empty() {
                errs.push("cell must be DOMAIN/MODE, both halves non-empty".into());
            }
        }
        if route.action.trim().is_empty() {
            errs.push("action must not be empty".into());
        }
        errs
    }

    /// Insert or replace a route. Fails (with every problem) if invalid.
    pub fn upsert(&mut self, route: AgentRoute) -> Result<(), Vec<String>> {
        let errs = Self::validate(&route);
        if !errs.is_empty() {
            return Err(errs);
        }
        let name = route.name.clone();
        if let Some(existing) = self.routes.iter_mut().find(|r| r.name == name) {
            *existing = route;
        } else {
            self.routes.push(route);
        }
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.routes.len();
        self.routes.retain(|r| r.name != name);
        self.routes.len() != before
    }

    pub fn all(&self) -> &[AgentRoute] {
        &self.routes
    }

    /// Routes bound to a classified cell, enabled only, insertion order. The
    /// classifier has already resolved ambiguity and context, so this is a
    /// deterministic key lookup — no re-embedding.
    pub fn for_cell(&self, domain: &str, mode: &str) -> Vec<&AgentRoute> {
        let key = format!(
            "{}/{}",
            domain.trim().to_ascii_uppercase(),
            mode.trim().to_ascii_uppercase()
        );
        self.routes
            .iter()
            .filter(|r| r.enabled && r.cell.trim().to_ascii_uppercase() == key)
            .collect()
    }

    /// Persistent table lives at `{data_dir}/routes.json` — the same
    /// load-once, save-on-mutate contract as `QualityTracker`.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(&self.routes).map_err(io::Error::other)?;
        let mut f = std::fs::File::create(path)?;
        f.write_all(&bytes)
    }

    pub fn load(path: &Path) -> Self {
        match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice::<Vec<AgentRoute>>(&bytes) {
                Ok(routes) => Self { routes },
                Err(e) => {
                    eprintln!("physis: routes.json malformed ({e}); starting with an empty table");
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(name: &str, cell: &str) -> AgentRoute {
        AgentRoute {
            name: name.into(),
            cell: cell.into(),
            action: "open the countermeasure".into(),
            examples: vec![],
            enabled: true,
        }
    }

    #[test]
    fn validation_reports_every_problem_at_once() {
        let bad = AgentRoute {
            name: "  ".into(),
            cell: "ONLYONE".into(),
            action: "".into(),
            examples: vec![],
            enabled: true,
        };
        let errs = RouteTable::validate(&bad);
        assert_eq!(errs.len(), 3, "name, cell, action all reported: {errs:?}");
    }

    #[test]
    fn cell_must_be_domain_slash_mode() {
        let r = route("x", "A/B/C");
        assert_eq!(RouteTable::validate(&r).len(), 1);
        let r = route("x", "/B");
        assert_eq!(RouteTable::validate(&r).len(), 1);
        let r = route("x", "A/B");
        assert!(RouteTable::validate(&r).is_empty());
    }

    #[test]
    fn upsert_inserts_then_replaces() {
        let mut t = RouteTable::new();
        t.upsert(route("downtime", "FABRICATE/WORK")).unwrap();
        t.upsert(route("downtime", "FABRICATE/REST")).unwrap();
        assert_eq!(t.all().len(), 1);
        assert_eq!(t.all()[0].cell, "FABRICATE/REST");
    }

    #[test]
    fn for_cell_matches_case_insensitively_and_skips_disabled() {
        let mut t = RouteTable::new();
        let on = route("downtime", "fabricate/work");
        t.upsert(on.clone()).unwrap();
        let mut off = route("hidden", "FABRICATE/WORK");
        off.enabled = false;
        t.upsert(off).unwrap();
        t.upsert(route("other", "STUDY/LEARN")).unwrap();

        let hits = t.for_cell("FABRICATE", "WORK");
        assert_eq!(hits.len(), 1, "only the enabled route matches");
        assert_eq!(hits[0].name, "downtime");
    }

    #[test]
    fn remove_returns_whether_something_was_removed() {
        let mut t = RouteTable::new();
        t.upsert(route("a", "A/B")).unwrap();
        assert!(t.remove("a"));
        assert!(!t.remove("a"));
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("physis_routes_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("routes.json");
        let mut t = RouteTable::new();
        t.upsert(route("downtime", "FABRICATE/WORK")).unwrap();
        t.save(&path).unwrap();
        let loaded = RouteTable::load(&path);
        assert_eq!(loaded.all().len(), 1);
        assert_eq!(loaded.all()[0].name, "downtime");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
