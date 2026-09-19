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
//! region-key match plus per-route example disambiguation. Misrouted text is
//! meant to be fed back into `QualityTracker` by the caller, which is the half
//! Semantic Router is missing.
//!
//! ## The routing key is the region, domain first
//!
//! A route binds a region: `DOMAIN` or `DOMAIN/MODE`. Domain is the strongest
//! measured axis (normalised self-recovery 0.585, against 0.472 for mode and
//! 0.416 for the joint cell — `docs/WHAT_PHYSIS_IS.md` §2), so a route may bind
//! the domain alone and the router falls back to it when the mode is unknown or
//! misclassified. The domain list is open (`PHYSIS_DOMAINS`); nothing here
//! hardcodes a domain name.
//!
//! Backward compatible with the shipped `routes.json` contract: an existing
//! `DOMAIN/MODE` route still loads and still matches. The change only *relaxes*
//! validation — a bare domain, previously rejected, is now a valid route.

use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::Path;

/// A declarative route: a region of the coordinate system bound to an action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentRoute {
    /// Unique route name a harness can branch on.
    pub name: String,
    /// The region this route is bound to: `DOMAIN` or `DOMAIN/MODE`.
    ///
    /// Domain is the strongest measured axis (normalised self-recovery 0.585)
    /// and outranks the joint cell (0.416); mode sits between them (0.472). See
    /// `docs/WHAT_PHYSIS_IS.md` §2. A route may therefore bind a domain alone —
    /// naming a mode narrows it, it is not required, and the router falls back
    /// to the domain route when a mode is unknown or misclassified. The domain
    /// list is open (`PHYSIS_DOMAINS`), so no domain name is hardcoded here.
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

impl AgentRoute {
    /// `(domain, mode)`, uppercased. `mode` is `None` for a domain-level route.
    /// A trailing or empty mode half is preserved so validation can report it.
    pub fn scope(&self) -> (String, Option<String>) {
        let cell = self.cell.trim();
        match cell.split_once('/') {
            Some((d, m)) => (
                d.trim().to_ascii_uppercase(),
                Some(m.trim().to_ascii_uppercase()),
            ),
            None => (cell.to_ascii_uppercase(), None),
        }
    }

    /// True when the route binds a domain and no mode.
    pub fn is_domain_level(&self) -> bool {
        self.scope().1.is_none()
    }
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
        let (domain, mode) = route.scope();
        if domain.is_empty() {
            errs.push("cell must name a domain (`DOMAIN` or `DOMAIN/MODE`)".into());
        }
        if let Some(m) = &mode {
            if m.is_empty() {
                errs.push("cell mode half must not be empty when `/` is present".into());
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

    /// Routes bound to a domain and no mode, enabled only, insertion order.
    /// This is the broad, reliable lookup: domain is the strongest measured
    /// axis, so a domain route still fires when the mode is unknown or wrong.
    pub fn for_domain(&self, domain: &str) -> Vec<&AgentRoute> {
        let key = domain.trim().to_ascii_uppercase();
        self.routes
            .iter()
            .filter(|r| r.enabled && r.scope() == (key.clone(), None))
            .collect()
    }

    /// Routes bound to an exact `DOMAIN/MODE`, enabled only, insertion order.
    /// The narrow lookup: the classifier has already resolved ambiguity and
    /// context, so this is a deterministic key match — no re-embedding.
    pub fn for_cell(&self, domain: &str, mode: &str) -> Vec<&AgentRoute> {
        let key = (
            domain.trim().to_ascii_uppercase(),
            Some(mode.trim().to_ascii_uppercase()),
        );
        self.routes
            .iter()
            .filter(|r| r.enabled && r.scope() == key)
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
            // Empty domain. A bare domain such as `ONLYONE` is now valid — a
            // route may bind the domain axis alone.
            cell: "  ".into(),
            action: "".into(),
            examples: vec![],
            enabled: true,
        };
        let errs = RouteTable::validate(&bad);
        assert_eq!(errs.len(), 3, "name, cell, action all reported: {errs:?}");
    }

    #[test]
    fn cell_names_a_domain_with_an_optional_mode() {
        // A domain alone is valid: it is the strongest measured axis.
        assert!(RouteTable::validate(&route("x", "A")).is_empty());
        // A domain and mode is valid.
        assert!(RouteTable::validate(&route("x", "A/B")).is_empty());
        // An empty domain, a `/` with no mode, or whitespace is not.
        assert_eq!(RouteTable::validate(&route("x", "/B")).len(), 1);
        assert_eq!(RouteTable::validate(&route("x", "A/")).len(), 1);
        assert_eq!(RouteTable::validate(&route("x", "  ")).len(), 1);
    }

    #[test]
    fn domain_routes_are_broad_and_cell_routes_are_narrow() {
        let mut t = RouteTable::new();
        t.upsert(route("broad", "FABRICATE")).unwrap();
        t.upsert(route("narrow", "FABRICATE/WORK")).unwrap();
        t.upsert(route("other", "STUDY")).unwrap();

        let broad = t.for_domain("FABRICATE");
        assert_eq!(
            broad.len(),
            1,
            "for_domain returns domain-level routes only"
        );
        assert_eq!(broad[0].name, "broad");

        let narrow = t.for_cell("FABRICATE", "WORK");
        assert_eq!(narrow.len(), 1);
        assert_eq!(narrow[0].name, "narrow");

        // The two lookups do not bleed into each other.
        assert!(t.for_cell("FABRICATE", "REST").is_empty());
        assert!(t.for_domain("STUDY")[0].is_domain_level());
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
