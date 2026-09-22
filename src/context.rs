//! Context-scoped ontology overlays: what an item means *here*.
//!
//! The base ontologies say what a word means in general. That is rarely the
//! question being asked. "Margin" is a machining allowance to a manufacturing
//! client and a gross-profit line to a publisher; "Release" is a deployment to
//! one project and a contractual discharge to another. A single global
//! ontology has to pick one, and picking one is what collapses distinct
//! meanings into a single cell.
//!
//! An overlay is an ontology scoped to a sector, a client, or a project. A
//! [`Context`] names at most one of each, and resolution walks from most
//! specific to least:
//!
//! ```text
//! project  →  client  →  sector  →  global
//! ```
//!
//! First definition wins. An overlay that does not mention a name leaves the
//! broader meaning intact, so a client overlay holding six entries does not
//! cost you the other nine hundred — it *shadows*, it does not replace.
//!
//! Overlays live under `<data_dir>/ontologies/<scope>/<id>.json` and use the
//! same file format as any other ontology, including its `interpretant` block:
//! a client can declare that, for its work, an inert outcome still confirms.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::OntologyLoader;
use crate::models::{DomainDef, Interpretant};

/// How specific an ontology is. Ordered: a later variant shadows an earlier one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// The built-in ontologies. Always present, never shadowed away entirely.
    Global,
    /// An industry: manufacturing, publishing, healthcare.
    Sector,
    /// One customer.
    Client,
    /// One engagement, which may narrow its client further.
    Project,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Global => "global",
            Scope::Sector => "sector",
            Scope::Client => "client",
            Scope::Project => "project",
        }
    }

    /// Directory name holding overlays of this scope.
    fn dir(self) -> &'static str {
        match self {
            Scope::Global => "global",
            Scope::Sector => "sector",
            Scope::Client => "client",
            Scope::Project => "project",
        }
    }

    /// The scopes an overlay can be authored at, narrowest last.
    pub const OVERLAY_SCOPES: [Scope; 3] = [Scope::Sector, Scope::Client, Scope::Project];
}

/// One loaded overlay file.
#[derive(Debug, Clone)]
pub struct Overlay {
    pub scope: Scope,
    /// Identifier within the scope — the file stem, e.g. `evotec`.
    pub id: String,
    pub interpretant: Option<Interpretant>,
    pub domains: HashMap<String, DomainDef>,
    pub path: PathBuf,
}

/// Which sector / client / project is in play.
///
/// Every field is optional: an empty context resolves exactly like the base
/// ontology, which is what keeps overlays an addition rather than a migration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Context {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sector: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

impl Context {
    /// Parse `sector=manufacturing,client=evotec,project=line3`.
    ///
    /// Also accepts `:` as the separator so `client:evotec` works, since that
    /// is how the scopes read in prose and in the studio.
    pub fn parse(s: &str) -> Self {
        let mut ctx = Context::default();
        for part in s.split(',') {
            let part = part.trim();
            let Some((k, v)) = part.split_once('=').or_else(|| part.split_once(':')) else {
                continue;
            };
            let v = v.trim().to_string();
            if v.is_empty() {
                continue;
            }
            match k.trim().to_ascii_lowercase().as_str() {
                "sector" => ctx.sector = Some(v),
                "client" => ctx.client = Some(v),
                "project" => ctx.project = Some(v),
                _ => {}
            }
        }
        ctx
    }

    /// Read the ambient context from `PHYSIS_CONTEXT`.
    pub fn from_env() -> Self {
        std::env::var("PHYSIS_CONTEXT")
            .map(|s| Context::parse(&s))
            .unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.sector.is_none() && self.client.is_none() && self.project.is_none()
    }

    /// The overlays this context selects, **narrowest first** — the order
    /// resolution walks.
    fn lookup_order(&self) -> Vec<(Scope, &str)> {
        let mut out = Vec::new();
        if let Some(p) = &self.project {
            out.push((Scope::Project, p.as_str()));
        }
        if let Some(c) = &self.client {
            out.push((Scope::Client, c.as_str()));
        }
        if let Some(s) = &self.sector {
            out.push((Scope::Sector, s.as_str()));
        }
        out
    }

    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(s) = &self.sector {
            parts.push(format!("sector:{s}"));
        }
        if let Some(c) = &self.client {
            parts.push(format!("client:{c}"));
        }
        if let Some(p) = &self.project {
            parts.push(format!("project:{p}"));
        }
        if parts.is_empty() {
            "global".to_string()
        } else {
            parts.join(" / ")
        }
    }
}

/// Where a resolved meaning came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Provenance {
    pub scope: Scope,
    /// Overlay id, or the base ontology's kind for `Scope::Global`.
    pub source: String,
}

/// A name resolved against a context.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub def: DomainDef,
    pub provenance: Provenance,
    /// Meanings this resolution shadowed, broadest last. Non-empty exactly when
    /// the context changed what the name means — which is the thing worth
    /// showing a user, so it is returned rather than discarded.
    pub shadowed: Vec<(Provenance, DomainDef)>,
}

/// All overlays found on disk.
#[derive(Debug, Clone, Default)]
pub struct Overlays {
    pub overlays: Vec<Overlay>,
}

impl Overlays {
    /// Load every overlay under `<data_dir>/ontologies/{sector,client,project}`.
    ///
    /// A missing directory is normal, not an error: overlays are opt-in.
    pub fn load(data_dir: &Path) -> Self {
        let root = data_dir.join("ontologies");
        let mut overlays = Vec::new();
        for scope in Scope::OVERLAY_SCOPES {
            let dir = root.join(scope.dir());
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in entries.flatten() {
                let path = e.path();
                if path.extension().and_then(|x| x.to_str()) != Some("json") {
                    continue;
                }
                let Some(id) = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(str::to_string)
                else {
                    continue;
                };
                match OntologyLoader::load_file(&path) {
                    Ok((interpretant, _language, domains)) => {
                        overlays.push(Overlay {
                            scope,
                            id,
                            interpretant,
                            domains,
                            path,
                        });
                    }
                    Err(err) => {
                        eprintln!("Warning: failed to load overlay {}: {err}", path.display());
                    }
                }
            }
        }
        overlays.sort_by(|a, b| (a.scope, &a.id).cmp(&(b.scope, &b.id)));
        Overlays { overlays }
    }

    fn get(&self, scope: Scope, id: &str) -> Option<&Overlay> {
        self.overlays
            .iter()
            .find(|o| o.scope == scope && o.id == id)
    }

    /// Resolve one name in a context, narrowest scope first.
    ///
    /// Returns the winning definition plus everything it shadowed, so a caller
    /// can show *what this means here* against *what it means in general*.
    pub fn resolve(&self, name: &str, ctx: &Context, base: &OntologyLoader) -> Option<Resolved> {
        let mut chain: Vec<(Provenance, DomainDef)> = Vec::new();

        for (scope, id) in ctx.lookup_order() {
            if let Some(o) = self.get(scope, id) {
                if let Some(def) = o.domains.get(name) {
                    chain.push((
                        Provenance {
                            scope,
                            source: o.id.clone(),
                        },
                        def.clone(),
                    ));
                }
            }
        }

        // The base ontology is the floor. `all_domains` merges every builtin and
        // extra, matching what the grid and the studio render.
        if let Some(def) = base.all_domains().get(name) {
            chain.push((
                Provenance {
                    scope: Scope::Global,
                    source: "global".to_string(),
                },
                def.clone(),
            ));
        }

        let mut it = chain.into_iter();
        let (provenance, def) = it.next()?;
        Some(Resolved {
            def,
            provenance,
            shadowed: it.collect(),
        })
    }

    /// Definitions contributed by the selected sector/client/project overlays.
    /// Narrower scopes come first, so callers can apply the same precedence when
    /// two overlays define the same signifier.
    pub fn active_definitions(&self, ctx: &Context) -> Vec<(Provenance, DomainDef)> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (scope, id) in ctx.lookup_order() {
            if let Some(o) = self.get(scope, id) {
                for (name, def) in &o.domains {
                    if seen.insert(name.clone()) {
                        out.push((
                            Provenance {
                                scope,
                                source: o.id.clone(),
                            },
                            def.clone(),
                        ));
                    }
                }
            }
        }
        out
    }

    /// The interpretant governing a context: the narrowest overlay that
    /// declares one, else `fallback` (the base ontology's own rule).
    pub fn interpretant(&self, ctx: &Context, fallback: Interpretant) -> Interpretant {
        for (scope, id) in ctx.lookup_order() {
            if let Some(i) = self.get(scope, id).and_then(|o| o.interpretant.clone()) {
                return i;
            }
        }
        fallback
    }

    /// Every context that defines `name`, for the "what does this mean where?"
    /// view. Includes the global meaning when one exists.
    pub fn meanings(&self, name: &str, base: &OntologyLoader) -> Vec<(Provenance, DomainDef)> {
        let mut out = Vec::new();
        if let Some(def) = base.all_domains().get(name) {
            out.push((
                Provenance {
                    scope: Scope::Global,
                    source: "global".to_string(),
                },
                def.clone(),
            ));
        }
        for o in &self.overlays {
            if let Some(def) = o.domains.get(name) {
                out.push((
                    Provenance {
                        scope: o.scope,
                        source: o.id.clone(),
                    },
                    def.clone(),
                ));
            }
        }
        out
    }

    /// Names defined in more than one place — the ones whose meaning is
    /// genuinely context-dependent, and the only ones worth a comparison view.
    pub fn contested_names(&self, base: &OntologyLoader) -> Vec<String> {
        let all = base.all_domains();
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for o in &self.overlays {
            for name in o.domains.keys() {
                *counts
                    .entry(name.as_str())
                    .or_insert(usize::from(all.contains_key(name))) += 1;
            }
        }
        let mut names: Vec<String> = counts
            .into_iter()
            .filter(|(_, n)| *n > 1)
            .map(|(k, _)| k.to_string())
            .collect();
        names.sort();
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PhysisConfig;

    fn write_overlay(dir: &Path, scope: Scope, id: &str, body: &str) {
        let d = dir.join("ontologies").join(scope.dir());
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join(format!("{id}.json")), body).unwrap();
    }

    /// An entry named the same in two clients, meaning different things.
    fn overlay_json(kind: &str, name: &str, domain: &str, mode: &str, hint: &str) -> String {
        format!(
            r#"{{"kind":"{kind}","domains":[{{"name":"{name}","category":"c",
               "domain":"{domain}","mode":"{mode}","axis_kind":"k","axis_name":"a",
               "unit":"u","hints":["{hint}"]}}]}}"#
        )
    }

    #[test]
    fn context_parses_both_separators_and_ignores_junk() {
        let a = Context::parse("sector=manufacturing,client=evotec,project=line3");
        let b = Context::parse("sector:manufacturing, client:evotec , project:line3");
        assert_eq!(a, b);
        assert_eq!(a.client.as_deref(), Some("evotec"));

        // Unknown keys and empty values must not silently become a context.
        let junk = Context::parse("tenant=x,client=,nonsense");
        assert!(junk.is_empty(), "junk parsed into {junk:?}");
    }

    #[test]
    fn the_narrowest_scope_wins_and_reports_what_it_shadowed() {
        let tmp = tempfile::tempdir().unwrap();
        write_overlay(
            tmp.path(),
            Scope::Sector,
            "manufacturing",
            &overlay_json(
                "sector",
                "Margin",
                "FABRICATE",
                "WORK",
                "machining allowance",
            ),
        );
        write_overlay(
            tmp.path(),
            Scope::Client,
            "evotec",
            &overlay_json("client", "Margin", "CONSTRUCT", "PLAN", "contract margin"),
        );

        let overlays = Overlays::load(tmp.path());
        assert_eq!(overlays.overlays.len(), 2);
        let base = OntologyLoader::load_all(&PhysisConfig::default());

        // Client is narrower than sector, so it wins.
        let ctx = Context::parse("sector:manufacturing,client:evotec");
        let r = overlays.resolve("Margin", &ctx, &base).expect("resolves");
        assert_eq!(r.provenance.scope, Scope::Client);
        assert_eq!(r.def.mode.as_deref(), Some("PLAN"));
        assert_eq!(r.shadowed.first().unwrap().0.scope, Scope::Sector);

        // Drop the client and the sector meaning surfaces.
        let ctx = Context::parse("sector:manufacturing");
        let r = overlays.resolve("Margin", &ctx, &base).unwrap();
        assert_eq!(r.provenance.scope, Scope::Sector);
        assert_eq!(r.def.mode.as_deref(), Some("WORK"));
        assert!(r.shadowed.is_empty(), "nothing left to shadow");
    }

    #[test]
    fn an_overlay_shadows_only_what_it_names() {
        let tmp = tempfile::tempdir().unwrap();
        write_overlay(
            tmp.path(),
            Scope::Client,
            "evotec",
            &overlay_json("client", "Margin", "CONSTRUCT", "PLAN", "contract margin"),
        );
        let overlays = Overlays::load(tmp.path());
        let base = OntologyLoader::load_all(&PhysisConfig::default());
        let ctx = Context::parse("client:evotec");

        // A name the overlay never mentions still resolves, from the base.
        let some_base_name = base.all_domains().keys().next().cloned().unwrap();
        let r = overlays
            .resolve(&some_base_name, &ctx, &base)
            .expect("base still reachable");
        assert_eq!(r.provenance.scope, Scope::Global);
    }

    #[test]
    fn an_empty_context_behaves_exactly_like_no_overlays() {
        let tmp = tempfile::tempdir().unwrap();
        write_overlay(
            tmp.path(),
            Scope::Client,
            "evotec",
            &overlay_json("client", "Margin", "CONSTRUCT", "PLAN", "contract margin"),
        );
        let overlays = Overlays::load(tmp.path());
        let base = OntologyLoader::load_all(&PhysisConfig::default());

        let ctx = Context::default();
        assert!(ctx.is_empty());
        // "Margin" exists only in the overlay, so an empty context finds nothing.
        assert!(overlays.resolve("Margin", &ctx, &base).is_none());
    }

    #[test]
    fn a_context_can_override_the_interpretant() {
        let tmp = tempfile::tempdir().unwrap();
        write_overlay(
            tmp.path(),
            Scope::Client,
            "evotec",
            r#"{
            "kind":"client",
            "interpretant":{"confirms":["success","inert"],"refutes":[],"half_life_days":0,"weight":1.0},
            "domains":[]
        }"#,
        );
        let overlays = Overlays::load(tmp.path());

        let ctx = Context::parse("client:evotec");
        let i = overlays.interpretant(&ctx, Interpretant::default());
        assert_eq!(
            i.delta("inert"),
            1.0,
            "this client counts inert as confirming"
        );
        assert_eq!(i.delta("failure"), 0.0, "and refutes nothing");

        // A context with no overlay keeps the fallback.
        let other = Context::parse("client:someone-else");
        assert_eq!(
            overlays.interpretant(&other, Interpretant::default()),
            Interpretant::default()
        );
    }

    #[test]
    fn contested_names_are_the_ones_defined_in_more_than_one_place() {
        let tmp = tempfile::tempdir().unwrap();
        write_overlay(
            tmp.path(),
            Scope::Client,
            "evotec",
            &overlay_json("client", "Margin", "CONSTRUCT", "PLAN", "contract"),
        );
        write_overlay(
            tmp.path(),
            Scope::Client,
            "publisher",
            &overlay_json("client", "Margin", "STUDY", "WORK", "gross profit"),
        );
        write_overlay(
            tmp.path(),
            Scope::Project,
            "line3",
            &overlay_json("project", "Takt", "FABRICATE", "WORK", "cycle time"),
        );

        let overlays = Overlays::load(tmp.path());
        let base = OntologyLoader::load_all(&PhysisConfig::default());
        let contested = overlays.contested_names(&base);
        assert!(
            contested.contains(&"Margin".to_string()),
            "got {contested:?}"
        );
        assert!(
            !contested.contains(&"Takt".to_string()),
            "defined once — not contested"
        );

        let meanings = overlays.meanings("Margin", &base);
        assert_eq!(meanings.len(), 2);
    }
}
