//! Ontology studio — the write half of the dictionary.
//!
//! The studio could already *show* every authored entry, which cell it lands
//! on and whether classification can reach it. What it could not do is change
//! any of it: editing the vocabulary meant hand-writing JSON into
//! `{data_dir}/packs/` on the server's filesystem, which is not a thing an
//! operator can be asked to do and not a thing a customer can be sold.
//!
//! A pack is data, not code (Phase 10b T4). This module gives that fact a
//! surface: list what is installed, validate a draft against the 70-cell grid,
//! write it atomically, and delete it again — all confined to the packs
//! directory, all with the cell vocabulary enforced rather than assumed.
//!
//! **What it deliberately does not do** is hot-swap the live classifier. The
//! ontology and its pre-computed cell centroids are immutable in `SharedCtx`
//! and read lock-free on the classification hot path; making them swappable is
//! a real change with real risk, and pretending a saved pack is already live
//! would be worse than saying it is not. So the listing reports, per pack,
//! whether the running process has it — and the draft can still be *tested*
//! against real text right now through `/api/v1/planner/simulate`, which
//! builds a throwaway classifier from a candidate ontology and was designed
//! for exactly this.

use super::*;
use crate::models::{HumanDomain, HumanMode, OntologyEntry};
use crate::web::SharedState;
use axum::{
    extract::{Json, State},
    http::StatusCode,
};

/// One draft entry as the studio posts it.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DraftEntry {
    pub name: String,
    #[serde(default)]
    pub category: Option<String>,
    pub domain: String,
    pub mode: String,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub hints: Vec<String>,
    #[serde(default)]
    pub axis_kind: Option<String>,
    #[serde(default)]
    pub axis_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SavePackRequest {
    /// Pack identity; becomes `{data_dir}/packs/{name}.json` and the key prefix
    /// every entry is filed under.
    pub name: String,
    pub entries: Vec<DraftEntry>,
}

#[derive(Debug, Deserialize)]
pub struct DeletePackRequest {
    pub name: String,
}

/// A validated pack, ready to serialize in the on-disk format.
///
/// The entries are [`OntologyEntry`], not `DomainDef`. They look
/// interchangeable and are not: `DomainDef` is the *loaded* shape, where
/// `axis_kind`/`axis_name` are optional, while the file format requires them.
/// Serializing the loaded shape produced `"axis_kind": null` — valid JSON that
/// the loader rejects, so the studio would have written packs the engine
/// silently skipped at the next start. The round-trip test below is what
/// caught it.
#[derive(Debug, Serialize)]
pub struct PackDoc {
    pub kind: String,
    pub domains: Vec<OntologyEntry>,
}

/// Pack names become filenames and ontology key prefixes. Anything outside
/// this alphabet is rejected rather than sanitized: silently renaming a
/// customer's pack is how "my pack disappeared" bugs are made, and `..` in a
/// name that gets joined onto a directory is how sandboxes are escaped.
pub fn validate_name(name: &str) -> Result<String, String> {
    let n = name.trim();
    if n.is_empty() || n.len() > 48 {
        return Err("pack name must be 1..48 characters".into());
    }
    if !n
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "pack name '{n}' may only contain letters, digits, '-' and '_'"
        ));
    }
    Ok(n.to_string())
}

/// Check a draft against the grid vocabulary and the format the loader expects.
///
/// Returns every problem at once rather than the first: a studio that reports
/// one error per round-trip turns a ten-entry pack into ten round-trips.
pub fn validate_pack(name: &str, entries: &[DraftEntry]) -> Result<PackDoc, Vec<String>> {
    let mut errors = Vec::new();
    let kind = match validate_name(name) {
        Ok(k) => k,
        Err(e) => {
            errors.push(e);
            String::new()
        }
    };
    if entries.is_empty() {
        errors.push("a pack needs at least one entry".into());
    }

    let mut seen: std::collections::HashSet<String> = Default::default();
    let mut domains = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let where_ = format!("entry {}", i + 1);
        let entry_name = e.name.trim();
        if entry_name.is_empty() {
            errors.push(format!("{where_}: name is required"));
        } else if !seen.insert(entry_name.to_ascii_lowercase()) {
            // Duplicates would silently collapse in the loader's HashMap, so
            // the pack the operator saved would not be the pack that loads.
            errors.push(format!("{where_}: duplicate name '{entry_name}'"));
        }

        let domain = e.domain.trim().to_ascii_uppercase();
        if HumanDomain::from_str(&domain).is_none() {
            errors.push(format!(
                "{where_}: '{}' is not a domain — expected one of {}",
                e.domain,
                HumanDomain::all()
                    .iter()
                    .map(|d| d.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        let mode = e.mode.trim().to_ascii_uppercase();
        if HumanMode::from_str(&mode).is_none() {
            errors.push(format!(
                "{where_}: '{}' is not a mode — expected one of {}",
                e.mode,
                HumanMode::all()
                    .iter()
                    .map(|m| m.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        // Hints are what the entry is embedded from. An entry with none is
        // reachable only by its own name, which is almost never what the
        // author meant — but it is legal, so this is a warning the UI shows,
        // not an error that blocks the save.
        let hints: Vec<String> = e
            .hints
            .iter()
            .map(|h| h.trim().to_string())
            .filter(|h| !h.is_empty())
            .collect();

        domains.push(OntologyEntry {
            name: entry_name.to_string(),
            category: e
                .category
                .as_ref()
                .map(|c| c.trim().to_string())
                .filter(|c| !c.is_empty()),
            domain,
            mode,
            // Axis fields are how an entry joins a measurable axis. Absent is
            // the honest default for a hand-authored pack, and the loader reads
            // an empty string as "none declared".
            axis_kind: e.axis_kind.clone().unwrap_or_default().trim().to_string(),
            axis_name: e.axis_name.clone().unwrap_or_default().trim().to_string(),
            unit: e
                .unit
                .as_ref()
                .map(|u| u.trim().to_string())
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| "items".to_string()),
            hints,
            facets: Default::default(),
        });
    }

    if errors.is_empty() {
        Ok(PackDoc { kind, domains })
    } else {
        Err(errors)
    }
}

fn packs_dir(state: &SharedState) -> PathBuf {
    state.shared.config.data_dir.join("packs")
}

/// Resolve `{packs}/{name}.json`, refusing anything that would land elsewhere.
fn pack_path(state: &SharedState, name: &str) -> Result<PathBuf, String> {
    let name = validate_name(name)?;
    Ok(packs_dir(state).join(format!("{name}.json")))
}

/// GET /api/v1/ontology/packs — what is installed, and what the running
/// process actually has loaded.
pub async fn packs_list_handler(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let dir = packs_dir(&state);
    // Live packs are keyed `"<stem>:<entry>"` in `custom_domains` — the same
    // convention the loader writes — so "is this pack live" is answerable
    // without re-reading the disk.
    let live_prefixes: std::collections::HashSet<String> = state
        .shared
        .ontology
        .custom_domains
        .keys()
        .filter_map(|k| k.split_once(':').map(|(p, _)| p.to_string()))
        .collect();

    let mut packs: Vec<serde_json::Value> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
            .collect();
        paths.sort();
        for path in paths {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let parsed: Option<serde_json::Value> = serde_json::from_str(&text).ok();
            let count = parsed
                .as_ref()
                .and_then(|v| v.get("domains"))
                .and_then(|d| d.as_array())
                .map(|a| a.len());
            packs.push(serde_json::json!({
                "name": stem,
                "path": path.display().to_string(),
                "entries": count,
                // A pack that fails to parse is skipped at load with a warning
                // on stderr nobody reads. Here it is visible.
                "valid": parsed.is_some(),
                "live": live_prefixes.contains(&stem),
            }));
        }
    }

    Json(serde_json::json!({
        "dir": dir.display().to_string(),
        "packs": packs,
        // Stated once, here, rather than implied: saving does not reclassify
        // anything until the process restarts.
        "note": "packs load at startup; a saved pack becomes live on restart. \
                 Use /api/v1/planner/simulate to see a draft's effect now.",
    }))
}

/// POST /api/v1/ontology/packs/save
pub async fn pack_save_handler(
    State(state): State<SharedState>,
    Json(req): Json<SavePackRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let doc = validate_pack(&req.name, &req.entries).map_err(|errors| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "pack rejected",
                "errors": errors,
            })),
        )
    })?;

    let dir = packs_dir(&state);
    std::fs::create_dir_all(&dir).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("cannot create {}: {e}", dir.display()) })),
        )
    })?;
    let path = dir.join(format!("{}.json", doc.kind));
    let bytes = serde_json::to_vec_pretty(&doc).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("serialize: {e}") })),
        )
    })?;
    // Atomic: a half-written pack is a pack the loader skips at the next start,
    // which would look like the save silently doing nothing.
    crate::atomic::write_atomic(&path, &bytes).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("write {}: {e}", path.display()) })),
        )
    })?;

    let hintless: Vec<&str> = doc
        .domains
        .iter()
        .filter(|d| d.hints.is_empty())
        .map(|d| d.name.as_str())
        .collect();

    Ok(Json(serde_json::json!({
        "saved": true,
        "name": doc.kind,
        "path": path.display().to_string(),
        "entries": doc.domains.len(),
        "live": false,
        "warnings": hintless.iter().map(|n| format!(
            "'{n}' has no hints — it can only be matched by its own name"
        )).collect::<Vec<_>>(),
        "note": "written to disk; loaded into the classifier at the next restart",
    })))
}

/// POST /api/v1/ontology/packs/delete
pub async fn pack_delete_handler(
    State(state): State<SharedState>,
    Json(req): Json<DeletePackRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let path = pack_path(&state, &req.name).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )
    })?;
    if !path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("no pack named '{}'", req.name) })),
        ));
    }
    std::fs::remove_file(&path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("delete: {e}") })),
        )
    })?;
    Ok(Json(serde_json::json!({
        "deleted": true,
        "name": req.name,
        "note": "removed from disk; entries stay live until the next restart",
    })))
}

/// GET /api/v1/ontology/packs/read?name=… — the draft behind an installed
/// pack, in the studio's own shape, so editing an existing pack is possible
/// without hand-parsing the file format in the browser.
#[derive(Debug, Deserialize)]
pub struct ReadPackQuery {
    pub name: String,
}

pub async fn pack_read_handler(
    State(state): State<SharedState>,
    axum::extract::Query(q): axum::extract::Query<ReadPackQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let path = pack_path(&state, &q.name).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )
    })?;
    let text = std::fs::read_to_string(&path).map_err(|e| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("read {}: {e}", path.display()) })),
        )
    })?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({ "error": format!("pack is not valid JSON: {e}") })),
        )
    })?;
    Ok(Json(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, domain: &str, mode: &str) -> DraftEntry {
        DraftEntry {
            name: name.into(),
            category: Some("Logistics".into()),
            domain: domain.into(),
            mode: mode.into(),
            unit: None,
            hints: vec!["freight".into(), "carrier".into()],
            axis_kind: None,
            axis_name: None,
        }
    }

    #[test]
    fn a_valid_draft_becomes_the_on_disk_format() {
        let doc =
            validate_pack("logistics", &[entry("Freight Booking", "bond", "work")]).expect("valid");
        assert_eq!(doc.kind, "logistics");
        assert_eq!(doc.domains[0].domain, "BOND");
        assert_eq!(doc.domains[0].mode, "WORK");
        // Unit is required by the loader's type; a draft that omits it gets a
        // neutral default rather than failing on a field nobody thinks about.
        assert_eq!(doc.domains[0].unit, "items");
        // Round-trips through the real loader — the only proof that what the
        // studio writes is what the engine can read.
        let json = serde_json::to_string(&doc).unwrap();
        let (_, map) = crate::config::OntologyLoader::load_str(&json, "logistics").unwrap();
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn every_problem_is_reported_at_once() {
        let errors = validate_pack(
            "ok-name",
            &[
                entry("", "BOND", "WORK"),
                entry("Ghost Cell", "NOPE", "ALSO-NOPE"),
            ],
        )
        .unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("name is required")),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|e| e.contains("not a domain")),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|e| e.contains("not a mode")),
            "{errors:?}"
        );
        assert!(
            errors.len() >= 3,
            "one round-trip per error is not a studio: {errors:?}"
        );
    }

    #[test]
    fn duplicate_names_are_rejected_because_the_loader_would_collapse_them() {
        let errors = validate_pack(
            "dupes",
            &[
                entry("Freight Booking", "BOND", "WORK"),
                entry("freight booking", "STUDY", "SENSE"),
            ],
        )
        .unwrap_err();
        assert!(errors.iter().any(|e| e.contains("duplicate")), "{errors:?}");
    }

    #[test]
    fn pack_names_that_would_escape_the_directory_are_refused() {
        for bad in ["../evil", "a/b", "", "with space", "..\\win"] {
            assert!(validate_name(bad).is_err(), "accepted {bad:?}");
        }
        assert_eq!(validate_name(" fine-name_1 ").unwrap(), "fine-name_1");
    }

    #[test]
    fn an_empty_pack_is_not_a_pack() {
        let errors = validate_pack("empty", &[]).unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("at least one entry")),
            "{errors:?}"
        );
    }
}
