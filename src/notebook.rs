//! Notebooks — dashboards the user composes (PLAN Phase 13).
//!
//! Every other surface is a composition *we* authored: Pulse decides which
//! vitals matter, Console decides that seventeen tabs is the shape of an
//! operator's day. Each was a reasonable guess and each is wrong for somebody,
//! and today fixing that means editing HTML and shipping a binary — the wrong
//! cost for a layout question.
//!
//! A notebook is an ordered list of **cells**. A cell names a widget from the
//! registry below and binds parameters to it.
//!
//! # A notebook holds references, never markup
//!
//! This is the decision the whole feature stands on. A cell carries a widget
//! **id** and **params** — never HTML, never JavaScript, never a URL to fetch.
//! Notebooks are meant to be exported and handed to a colleague; the moment one
//! could carry a fetch target, "open my dashboard" would be enough to run a
//! request inside the recipient's authenticated session. Inertness has to be
//! structural, not a sanitizer someone remembers to call.
//!
//! The endpoint each widget reads lives *here*, in code the server ships, and
//! is validated against on every write. That is the asymmetry that makes the
//! guarantee hold: the registry is code, the notebook is data.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What kind of value a widget parameter accepts.
///
/// Deliberately tiny. A parameter language rich enough to be interesting is
/// rich enough to smuggle something through, and every widget here needs at
/// most a string, a count, or one of a fixed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    Text,
    Int,
    Enum(&'static [&'static str]),
}

#[derive(Debug, Clone, Copy)]
pub struct ParamDef {
    pub name: &'static str,
    pub kind: ParamKind,
    pub required: bool,
    pub help: &'static str,
}

/// How a widget's data is fetched. Server-authored, never user-supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

impl Method {
    pub fn as_str(&self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
        }
    }
}

/// One entry in the catalogue: what it shows, what it reads, what it accepts.
#[derive(Debug, Clone, Copy)]
pub struct WidgetDef {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub method: Method,
    /// Endpoint path. Params are sent as a query string (GET) or a JSON body
    /// (POST); no widget interpolates them into the path, so a parameter can
    /// never redirect a request somewhere else.
    pub path: &'static str,
    /// Which shared renderer draws the response.
    pub render: &'static str,
    /// For numeric renderers (`kpis`, `bars`): exactly which response fields to
    /// show, in this order. Empty means "every number in the response", which
    /// is right for a vitals tile and wrong for anything claiming to show a
    /// distribution — a "verdict distribution" that also plots `cluster_count`
    /// is not a distribution, it is a dump.
    pub fields: &'static [&'static str],
    pub params: &'static [ParamDef],
    /// Columns out of twelve this widget wants by default.
    pub default_span: u8,
    /// True if the widget's endpoint mutates state. Every v1 widget reads;
    /// the field exists so the editor can mark a writing widget distinctly the
    /// day one is added, rather than discovering it needs to after the fact.
    pub writes: bool,
}

/// The catalogue. Every widget is a thin reader over an endpoint that already
/// exists — if a widget would need a new endpoint, that is separate work with
/// its own justification, not a panel smuggling one in.
pub const WIDGETS: &[WidgetDef] = &[
    WidgetDef {
        id: "coherence.kpis",
        title: "Coherence vitals",
        description: "Nodes, coherence index, verdict counts, clusters, dream cycles.",
        method: Method::Get,
        path: "/api/v1/coherence/snapshot",
        render: "kpis",
        fields: &[
            "total_nodes",
            "coherence_index",
            "asserted_success",
            "asserted_inert",
            "asserted_failure",
            "cluster_count",
            "outlier_count",
            "dream_cycle_count",
        ],
        params: &[],
        default_span: 12,
        writes: false,
    },
    WidgetDef {
        id: "coherence.distribution",
        title: "Verdict distribution",
        description: "Where the graph's metabolism sits: succeeded, inert, failed, unjudged.",
        method: Method::Get,
        path: "/api/v1/coherence/snapshot",
        render: "bars",
        fields: &[
            "asserted_success",
            "asserted_inert",
            "asserted_failure",
            "high_coherence",
            "mid_coherence",
            "low_coherence",
        ],
        params: &[],
        default_span: 6,
        writes: false,
    },
    WidgetDef {
        id: "communities.grid",
        title: "Communities",
        description: "Label-propagation clusters over the coherence graph.",
        method: Method::Get,
        path: "/api/v1/communities",
        fields: &[],
        render: "list",
        params: &[],
        default_span: 6,
        writes: false,
    },
    WidgetDef {
        id: "atlas.summary",
        title: "Graph shape",
        description: "Nodes, recorded vs derived links, communities — the atlas in numbers.",
        method: Method::Get,
        path: "/api/v1/atlas/graph",
        fields: &[],
        render: "kpis",
        params: &[],
        default_span: 6,
        writes: false,
    },
    WidgetDef {
        id: "search.content",
        title: "Pinned search",
        description: "A standing question answered live: sections of your files that say it.",
        method: Method::Post,
        path: "/api/v1/search/content",
        fields: &[],
        render: "hits",
        params: &[
            ParamDef {
                name: "query",
                kind: ParamKind::Text,
                required: true,
                help: "What to look for; \"quoted spans\" must appear verbatim",
            },
            ParamDef {
                name: "place",
                kind: ParamKind::Text,
                required: false,
                help: "Folder facet, relative to the scan root",
            },
            ParamDef {
                name: "mode",
                kind: ParamKind::Enum(&["hybrid", "semantic", "lexical"]),
                required: false,
                help: "Ranking channel",
            },
            ParamDef {
                name: "top_k",
                kind: ParamKind::Int,
                required: false,
                help: "How many sections to show",
            },
        ],
        default_span: 6,
        writes: false,
    },
    WidgetDef {
        id: "corpus.themes",
        title: "Recurring themes",
        description: "What the whole corpus is about — community summaries, computed offline.",
        method: Method::Get,
        path: "/api/v1/corpus/summaries",
        fields: &[],
        render: "list",
        params: &[],
        default_span: 6,
        writes: false,
    },
    WidgetDef {
        id: "quality.trend",
        title: "Classification failures",
        description: "Reported failures over the last 30 days, and what the grid learned.",
        method: Method::Get,
        path: "/api/v1/quality",
        fields: &[],
        render: "bars",
        params: &[],
        default_span: 6,
        writes: false,
    },
    WidgetDef {
        id: "ledger.recent",
        title: "Why things exist",
        description: "Recent causal events: what produced an artifact, and for what.",
        method: Method::Post,
        path: "/api/v1/files/query",
        fields: &[],
        render: "feed",
        params: &[
            ParamDef {
                name: "cause",
                kind: ParamKind::Text,
                required: false,
                help: "Mechanism or actor (generated, received, vault-import, …)",
            },
            ParamDef {
                name: "place",
                kind: ParamKind::Text,
                required: false,
                help: "Only artifacts under this path prefix",
            },
            ParamDef {
                name: "within",
                kind: ParamKind::Int,
                required: false,
                help: "Only the last N days",
            },
        ],
        default_span: 6,
        writes: false,
    },
    WidgetDef {
        id: "goals.list",
        title: "Goals",
        description: "What this tenant is working toward, and how far along.",
        method: Method::Get,
        path: "/api/v1/goals/list",
        fields: &[],
        render: "list",
        params: &[],
        default_span: 4,
        writes: false,
    },
    WidgetDef {
        id: "packs.status",
        title: "Ontology packs",
        description: "Installed dictionaries and whether this process has loaded them.",
        method: Method::Get,
        path: "/api/v1/ontology/packs",
        fields: &[],
        render: "table",
        params: &[],
        default_span: 4,
        writes: false,
    },
    WidgetDef {
        id: "machines.registry",
        title: "Machines",
        description: "The reducers, their modality, and which ones write into the graph.",
        method: Method::Get,
        path: "/api/v1/machines",
        fields: &[],
        render: "table",
        params: &[],
        default_span: 4,
        writes: false,
    },
    WidgetDef {
        id: "semiotic.heatmap",
        title: "Semiotic heat",
        description: "Ontology activation across the DOMAIN × MODE grid.",
        method: Method::Get,
        path: "/api/v1/semiotic/heatmap",
        fields: &[],
        render: "heat",
        params: &[],
        default_span: 12,
        writes: false,
    },
];

pub fn widget(id: &str) -> Option<&'static WidgetDef> {
    WIDGETS.iter().find(|w| w.id == id)
}

/// Column spans a cell may claim, out of twelve. A closed set rather than any
/// integer: arbitrary widths produce layouts that break at the first narrow
/// screen, and nobody is served by a 7-column panel.
pub const SPANS: &[u8] = &[3, 4, 6, 12];
/// Height presets. Not pixels — a notebook composed on a 27" screen has to
/// stay readable on a laptop.
pub const HEIGHTS: &[&str] = &["s", "m", "l", "xl"];

/// A notebook can hold this many cells. Past it, the user has rebuilt the
/// junk drawer the feature exists to replace, and a page fetching sixty
/// endpoints is a load test rather than a dashboard.
pub const MAX_CELLS: usize = 24;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Refresh {
    /// `manual` or `interval`.
    pub mode: String,
    #[serde(default)]
    pub seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cell {
    pub widget: String,
    pub span: u8,
    pub height: String,
    #[serde(default)]
    pub params: serde_json::Map<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh: Option<Refresh>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Notebook {
    pub id: String,
    pub title: String,
    pub cells: Vec<Cell>,
    /// RFC3339; stamped by the caller on save, not by this module — a pure
    /// validator that reads the clock cannot be tested for equality.
    #[serde(default)]
    pub updated_at: String,
}

/// Notebook ids become sled keys and appear in URLs.
pub fn validate_id(id: &str) -> Result<String, String> {
    let t = id.trim();
    if t.is_empty() || t.len() > 48 {
        return Err("notebook id must be 1..48 characters".into());
    }
    if !t
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "notebook id '{t}' may only contain letters, digits, '-' and '_'"
        ));
    }
    Ok(t.to_string())
}

/// Check a posted notebook against the registry.
///
/// Returns every problem at once: a client that has to fix one error per
/// round-trip turns a ten-cell notebook into ten round-trips.
///
/// The rejection list is the security boundary as much as the ergonomic one —
/// an unknown widget id is refused rather than stored and skipped at render,
/// because a stored id nobody validates is how a payload eventually rides in.
pub fn validate(nb: &Notebook) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if let Err(e) = validate_id(&nb.id) {
        errors.push(e);
    }
    if nb.title.trim().is_empty() {
        errors.push("notebook needs a title".into());
    }
    if nb.title.chars().count() > 120 {
        errors.push("title must be at most 120 characters".into());
    }
    if nb.cells.len() > MAX_CELLS {
        errors.push(format!(
            "a notebook holds at most {MAX_CELLS} cells (got {})",
            nb.cells.len()
        ));
    }

    for (i, cell) in nb.cells.iter().enumerate() {
        let at = format!("cell {}", i + 1);
        let Some(def) = widget(&cell.widget) else {
            errors.push(format!(
                "{at}: unknown widget '{}' — the catalogue is at GET /api/v1/notebooks/registry",
                cell.widget
            ));
            continue;
        };
        if !SPANS.contains(&cell.span) {
            errors.push(format!(
                "{at}: span {} is not one of {:?}",
                cell.span, SPANS
            ));
        }
        if !HEIGHTS.contains(&cell.height.as_str()) {
            errors.push(format!(
                "{at}: height '{}' is not one of {:?}",
                cell.height, HEIGHTS
            ));
        }
        if let Some(r) = &cell.refresh {
            if r.mode != "manual" && r.mode != "interval" {
                errors.push(format!("{at}: refresh mode must be manual or interval"));
            }
            // A floor, not a suggestion: a five-second widget is a load
            // generator pointed at the operator's own engine.
            if r.mode == "interval" && r.seconds < 15 {
                errors.push(format!("{at}: refresh interval must be at least 15s"));
            }
        }

        for p in def.params {
            if p.required && !cell.params.contains_key(p.name) {
                errors.push(format!("{at}: '{}' requires '{}'", def.id, p.name));
            }
        }
        for (key, value) in &cell.params {
            let Some(p) = def.params.iter().find(|p| p.name == key) else {
                errors.push(format!("{at}: '{}' takes no parameter '{key}'", def.id));
                continue;
            };
            match p.kind {
                ParamKind::Text => match value.as_str() {
                    Some(s) if s.chars().count() > 500 => {
                        errors.push(format!("{at}: '{key}' is too long (max 500 characters)"));
                    }
                    Some(_) => {}
                    None => errors.push(format!("{at}: '{key}' must be text")),
                },
                ParamKind::Int => {
                    if !value.is_i64() && !value.is_u64() {
                        errors.push(format!("{at}: '{key}' must be a whole number"));
                    }
                }
                ParamKind::Enum(allowed) => match value.as_str() {
                    Some(s) if allowed.contains(&s) => {}
                    _ => errors.push(format!(
                        "{at}: '{key}' must be one of {}",
                        allowed.join(", ")
                    )),
                },
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// The catalogue as JSON, for the picker and the renderer.
pub fn registry_json() -> Value {
    let widgets: Vec<Value> = WIDGETS
        .iter()
        .map(|w| {
            serde_json::json!({
                "id": w.id,
                "title": w.title,
                "description": w.description,
                "method": w.method.as_str(),
                "path": w.path,
                "render": w.render,
                "fields": w.fields,
                "default_span": w.default_span,
                "writes": w.writes,
                "params": w.params.iter().map(|p| serde_json::json!({
                    "name": p.name,
                    "kind": match p.kind {
                        ParamKind::Text => "text",
                        ParamKind::Int => "int",
                        ParamKind::Enum(_) => "enum",
                    },
                    "options": match p.kind {
                        ParamKind::Enum(o) => Some(o.to_vec()),
                        _ => None,
                    },
                    "required": p.required,
                    "help": p.help,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    serde_json::json!({
        "widgets": widgets,
        "spans": SPANS,
        "heights": HEIGHTS,
        "max_cells": MAX_CELLS,
    })
}

/// The composition a user sees before they have composed anything.
///
/// Not an empty grid: a blank page asks someone who has never seen the product
/// to know what it can show. This is the Pulse composition, which they can then
/// take apart.
pub fn starter() -> Notebook {
    let cell = |widget: &str, span: u8, height: &str| Cell {
        widget: widget.to_string(),
        span,
        height: height.to_string(),
        params: Default::default(),
        refresh: None,
    };
    Notebook {
        id: "default".into(),
        title: "My dashboard".into(),
        cells: vec![
            cell("coherence.kpis", 12, "s"),
            cell("coherence.distribution", 6, "m"),
            cell("communities.grid", 6, "m"),
            cell("corpus.themes", 6, "m"),
            cell("ledger.recent", 6, "m"),
        ],
        updated_at: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cell(widget: &str) -> Cell {
        Cell {
            widget: widget.to_string(),
            span: 6,
            height: "m".into(),
            params: Default::default(),
            refresh: None,
        }
    }

    fn nb(cells: Vec<Cell>) -> Notebook {
        Notebook {
            id: "ops".into(),
            title: "Line 2".into(),
            cells,
            updated_at: String::new(),
        }
    }

    #[test]
    fn the_starter_notebook_is_valid() {
        // The one composition every new user sees must not be one the server
        // would refuse to save back.
        validate(&starter()).expect("starter notebook validates");
    }

    #[test]
    fn every_catalogue_widget_reads_an_absolute_api_path() {
        // A widget's endpoint is server-authored; a relative or templated path
        // would be a place for a parameter to redirect a request.
        for w in WIDGETS {
            assert!(w.path.starts_with("/api/v1/"), "{}: {}", w.id, w.path);
            assert!(!w.path.contains('{'), "no path templating: {}", w.path);
            assert!(SPANS.contains(&w.default_span), "{}", w.id);
        }
    }

    #[test]
    fn unknown_widgets_are_refused_not_stored() {
        // Storing an id nobody validates and skipping it at render is how a
        // payload eventually rides in.
        let errors = validate(&nb(vec![cell("totally.made.up")])).unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("unknown widget")),
            "{errors:?}"
        );
    }

    #[test]
    fn parameters_are_checked_against_the_widget_that_declares_them() {
        let mut c = cell("search.content");
        c.params.insert("query".into(), json!("bearing"));
        c.params.insert("top_k".into(), json!("eight")); // wrong type
        c.params.insert("mode".into(), json!("telepathy")); // not in the enum
        c.params.insert("rag_dir".into(), json!("/etc")); // not a parameter at all
        let errors = validate(&nb(vec![c])).unwrap_err();

        assert!(
            errors
                .iter()
                .any(|e| e.contains("'top_k' must be a whole number")),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|e| e.contains("'mode' must be one of")),
            "{errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("takes no parameter 'rag_dir'")),
            "an undeclared parameter is refused, not forwarded: {errors:?}"
        );
    }

    #[test]
    fn a_required_parameter_is_required() {
        let errors = validate(&nb(vec![cell("search.content")])).unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("requires 'query'")),
            "{errors:?}"
        );
    }

    #[test]
    fn geometry_is_a_closed_set() {
        let mut c = cell("goals.list");
        c.span = 7;
        c.height = "enormous".into();
        let errors = validate(&nb(vec![c])).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("span 7")), "{errors:?}");
        assert!(
            errors.iter().any(|e| e.contains("height 'enormous'")),
            "{errors:?}"
        );
    }

    #[test]
    fn refresh_intervals_have_a_floor() {
        // A five-second widget is a load generator pointed at the operator's
        // own engine.
        let mut c = cell("goals.list");
        c.refresh = Some(Refresh {
            mode: "interval".into(),
            seconds: 5,
        });
        let errors = validate(&nb(vec![c])).unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("at least 15s")),
            "{errors:?}"
        );
    }

    #[test]
    fn ids_that_would_escape_their_key_space_are_refused() {
        for bad in ["../evil", "a/b", "", "with space"] {
            assert!(validate_id(bad).is_err(), "accepted {bad:?}");
        }
        assert_eq!(validate_id(" ops-1 ").unwrap(), "ops-1");
    }

    #[test]
    fn every_problem_is_reported_at_once() {
        let mut bad = cell("nope");
        bad.span = 5;
        let errors = validate(&Notebook {
            id: "../x".into(),
            title: "  ".into(),
            cells: vec![bad],
            updated_at: String::new(),
        })
        .unwrap_err();
        // id, title, unknown widget — one round-trip per error is not an editor.
        assert!(errors.len() >= 3, "{errors:?}");
    }

    #[test]
    fn the_cell_cap_is_enforced() {
        let cells: Vec<Cell> = (0..MAX_CELLS + 1).map(|_| cell("goals.list")).collect();
        let errors = validate(&nb(cells)).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("at most")), "{errors:?}");
    }

    #[test]
    fn numeric_widgets_declare_the_fields_they_plot() {
        // A "verdict distribution" that also plots cluster_count is a dump, not
        // a distribution — the renderer takes the list from here rather than
        // guessing from whatever numbers the response happens to carry.
        let dist = widget("coherence.distribution").unwrap();
        assert!(
            dist.fields.contains(&"asserted_failure"),
            "{:?}",
            dist.fields
        );
        assert!(
            !dist.fields.contains(&"cluster_count"),
            "a distribution shows one distribution: {:?}",
            dist.fields
        );
        // Only the numeric renderers need a field list; the rest read arrays.
        for w in WIDGETS {
            if !w.fields.is_empty() {
                assert!(
                    w.render == "kpis" || w.render == "bars",
                    "{} declares fields but renders as {}",
                    w.id,
                    w.render
                );
            }
        }
    }

    #[test]
    fn the_page_never_interpolates_a_cell_field_into_an_attribute_raw() {
        // The server validates what it *stores*, but the page's Import reads a
        // layout pasted from a colleague and renders it before any save — and a
        // shared layout is exactly the input an attacker controls. A span of
        // `6" onload="…` would have broken out of the attribute.
        //
        // A browser test would catch this once; this catches it in CI forever,
        // by pinning the sink itself.
        let page = include_str!("bin/ui/notebook.html");
        assert!(
            page.contains(r#"'data-span="' + Number(c.span) + '" data-height="' + esc(c.height)"#),
            "the cell attributes must be coerced/escaped at the sink"
        );
        assert!(
            !page.contains(r#"'data-span="' + c.span"#),
            "raw interpolation of a cell field into an attribute is how this broke"
        );
        // And imports are rebuilt against the registry rather than filtered,
        // so a field the format does not describe cannot survive the trip.
        assert!(page.contains("function sanitizeImported("), "import must be sanitized");
    }

    #[test]
    fn the_registry_serializes_what_the_editor_needs() {
        let j = registry_json();
        let widgets = j["widgets"].as_array().unwrap();
        assert_eq!(widgets.len(), WIDGETS.len());
        let search = widgets
            .iter()
            .find(|w| w["id"] == "search.content")
            .unwrap();
        assert_eq!(search["method"], "POST");
        let mode = search["params"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == "mode")
            .unwrap();
        assert_eq!(mode["kind"], "enum");
        assert!(mode["options"]
            .as_array()
            .unwrap()
            .contains(&json!("hybrid")));
    }
}
