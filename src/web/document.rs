//! Adaptive-document service (PLAN.md Phase 26 §26.7).
//!
//! One managed document per tenant, served through the existing Bearer-gated
//! web surface. The wire verbs are the six the plan froze — `snapshot`,
//! `edit_source`, `propose`, `decide_patch`, `propose_undo`, `history` — folded
//! into four endpoints because `propose` is part of `edit_source` (a source edit
//! is the only thing that triggers a proposal) and `history` is a read view
//! carried by `snapshot`.
//!
//! Binding logic lives in [`crate::adaptive_document`], never here. This module
//! only resolves a tenant to a store root, serialises the read-modify-write,
//! and maps [`ApplyError`] to an HTTP status. A second implementation of the
//! same rules in JS would be the drift §26.7 forbids.

use super::*;
use crate::adaptive_document::proposal::{
    apply_patch, apply_source_edit, propose_after_source_edit, reject_patch, undo_patch,
};
use crate::adaptive_document::store::{Patches, Store};
use crate::adaptive_document::{
    parse_blocks, ApplyError, Block, Change, Document, Link, LinkBehavior, LinkProvenance,
};

/// Default document id. The page opens on this without asking for one; an
/// explicit id is what makes a second managed document possible later.
const DEFAULT_DOCUMENT: &str = "field-report";

/// Only letters, digits, `-` and `_`, so a document id can never escape the
/// tenant directory. Same rule as notebook ids; the error names this verb
/// because a caller who typed a path here needs to see why it was refused.
fn validate_document_id(id: &str) -> Result<String, String> {
    let t = id.trim();
    if t.is_empty() || t.len() > 48 {
        return Err("document id must be 1..48 characters".into());
    }
    if !t
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "document id '{t}' may only contain letters, digits, '-' and '_'"
        ));
    }
    Ok(t.to_string())
}

/// One tenant's document root. Keyed by the same `X-Physis-User` value every
/// other tenant-scoped handler uses, so AD14 (one context cannot read or apply
/// another's patches) holds by construction: the paths cannot meet.
fn store_for(state: &AppCtx, uid: &str, document_id: &str) -> Store {
    Store::open(
        &state
            .shared
            .config
            .data_dir
            .join("documents")
            .join(uid)
            .join(document_id),
    )
}

/// The canonical fixture from §26.8, seeded on first read so the page has a
/// document whose dependencies are visible before the user authors anything.
///
/// Ground truth for the acceptance suite is *not* this function — it lives in
/// `tests/adaptive_document.rs` expectations. This is demo seed data.
fn canonical_fixture() -> (String, Vec<Link>) {
    let raw = "# Field report\n\n\
               We ordered 18 units of bolts.\n\n\
               We ordered 18 units of bolts.\n\n\
               Budget is 36 euros.\n\n\
               Check supplier terms.\n\n\
               The 18 units arrived last year.\n\n\
               <!-- locked -->Do not change this line.\n";
    let doc = parse_blocks(DEFAULT_DOCUMENT, 0, raw);
    // Index-derived ids: parse order is deterministic, and the duplicate
    // sentence above gets distinct ids from the occurrence counter.
    let ids: Vec<String> = doc.blocks.iter().map(|b| b.id.clone()).collect();
    let link = |id: &str, to: usize, behavior, transform: Option<&str>| Link {
        id: id.into(),
        source_block_id: ids[1].clone(),
        dependent_block_id: ids[to].clone(),
        behavior,
        transform_id: transform.map(|s| s.to_string()),
        provenance: LinkProvenance::HumanConfirmed,
        source_revision_at_confirmation: 0,
        active: true,
    };
    let links = vec![
        link("l-copy", 2, LinkBehavior::Copy, Some("copy")),
        link("l-calc", 3, LinkBehavior::Calculate, Some("scale:2")),
        link("l-review", 4, LinkBehavior::Review, None),
        link("l-lock", 6, LinkBehavior::Review, None),
    ];
    (raw.to_string(), links)
}

/// Read the tenant's document, seeding the fixture on first access. Returns the
/// document plus whether it was just created.
fn load_or_seed(store: &Store, uid: &str) -> std::io::Result<(Document, bool)> {
    if let Some(doc) = store.load_document()? {
        return Ok((doc, false));
    }
    let (raw, links) = canonical_fixture();
    let mut doc = parse_blocks(DEFAULT_DOCUMENT, 0, &raw);
    doc.links = links;
    doc.graph_revision = 1;
    doc.actor = uid.to_string();
    store.save_document(&doc)?;
    Ok((doc, true))
}

fn doc_path_block(doc: &Document, b: &Block) -> serde_json::Value {
    serde_json::json!({
        "id": b.id,
        "kind": b.kind,
        "text": b.text(&doc.raw),
        "span": [b.span.0, b.span.1],
        "locked": b.locked,
        "authored_by": b.authored_by,
    })
}

fn snapshot_json(doc: &Document, patches: &Patches, events: &[Change]) -> serde_json::Value {
    serde_json::json!({
        "document_id": doc.id,
        "revision": doc.revision,
        "graph_revision": doc.graph_revision,
        "raw": doc.raw,
        "blocks": doc.blocks.iter().map(|b| doc_path_block(doc, b)).collect::<Vec<_>>(),
        "links": doc.links,
        "patches": patches.patches,
        // Newest last: the file order *is* store sequence (AD16), and reversing
        // it here would let a display convention masquerade as replay order.
        "events": events,
    })
}

fn apply_error(err: ApplyError) -> (StatusCode, Json<serde_json::Value>) {
    let (status, kind) = match err {
        ApplyError::NotProposed => (StatusCode::CONFLICT, "not_proposed"),
        ApplyError::StaleDocumentRevision => (StatusCode::CONFLICT, "stale_document_revision"),
        ApplyError::StaleGraphRevision => (StatusCode::CONFLICT, "stale_graph_revision"),
        ApplyError::ContentMismatch => (StatusCode::CONFLICT, "content_mismatch"),
        ApplyError::OverlappingRanges => (StatusCode::BAD_REQUEST, "overlapping_ranges"),
        ApplyError::MissingBlock => (StatusCode::NOT_FOUND, "missing_block"),
        ApplyError::UndoConflict => (StatusCode::CONFLICT, "undo_conflict"),
    };
    (
        status,
        Json(serde_json::json!({ "error": kind, "kind": kind })),
    )
}

fn bad_request(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": msg })),
    )
}

fn io_error(e: std::io::Error) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": e.to_string() })),
    )
}

/// GET /document — the page. Gated like every other product page.
pub async fn document_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/document.html"))
}

/// GET /api/v1/document?document_id=… — the whole read view: blocks, links,
/// patches (each carrying its reasons) and the event history.
pub async fn snapshot_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(q): Query<SnapshotQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let uid = user_key(&headers);
    let document_id = validate_document_id(q.document_id.as_deref().unwrap_or(DEFAULT_DOCUMENT))
        .map_err(|e| bad_request(&e))?;
    let store = store_for(&state, &uid, &document_id);
    let _guard = state.documents.lock().unwrap_or_else(|e| e.into_inner());
    let (doc, _seeded) = load_or_seed(&store, &uid).map_err(io_error)?;
    let patches = store.load_patches().map_err(io_error)?;
    let events = store.load_events().map_err(io_error)?;
    Ok(Json(snapshot_json(&doc, &patches, &events)))
}

#[derive(Deserialize)]
pub struct SnapshotQuery {
    #[serde(default)]
    document_id: Option<String>,
}

#[derive(Deserialize)]
pub struct EditRequest {
    #[serde(default)]
    document_id: Option<String>,
    block_id: String,
    new_bytes: String,
    /// The revision the client believed it was editing. A mismatch is a
    /// conflict, not a silent overwrite (AD07).
    expected_revision: u64,
    #[serde(default)]
    actor: Option<String>,
}

/// POST /api/v1/document/edit — record one human source edit and return the
/// proposal bound to the resulting revision. Never applies the proposal.
pub async fn edit_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<EditRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let uid = user_key(&headers);
    let document_id = validate_document_id(req.document_id.as_deref().unwrap_or(DEFAULT_DOCUMENT))
        .map_err(|e| bad_request(&e))?;
    let store = store_for(&state, &uid, &document_id);
    let _guard = state.documents.lock().unwrap_or_else(|e| e.into_inner());
    let (doc, _) = load_or_seed(&store, &uid).map_err(io_error)?;

    if req.expected_revision != doc.revision {
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "stale_document_revision",
                "expected": req.expected_revision,
                "current": doc.revision,
            })),
        ));
    }
    if doc.block(&req.block_id).is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "missing_block" })),
        ));
    }

    let actor = req.actor.clone().unwrap_or_else(|| uid.clone());
    let events = store.load_events().map_err(io_error)?;
    let sequence = events.last().map(|e| e.sequence + 1).unwrap_or(0);
    let recorded_at = now_secs();

    // A no-op edit creates no semantic-change claim (§26.4) and therefore no
    // proposal: the caller gets the current snapshot back unchanged.
    let Some((next, change)) = apply_source_edit(
        &doc,
        &req.block_id,
        &req.new_bytes,
        &actor,
        sequence,
        recorded_at,
    ) else {
        let patches = store.load_patches().map_err(io_error)?;
        let mut body = snapshot_json(&doc, &patches, &events);
        body["no_change"] = serde_json::json!(true);
        return Ok(Json(body));
    };

    let patch = propose_after_source_edit(&next, &change, &format!("p-{}", change.event_id));
    store.save_document(&next).map_err(io_error)?;
    store.append_event(&change).map_err(io_error)?;
    let mut patches = store.load_patches().map_err(io_error)?;
    patches.patches.push(patch.clone());
    store.save_patches(&patches).map_err(io_error)?;

    Ok(Json(serde_json::json!({
        "document_id": next.id,
        "revision": next.revision,
        "change": change,
        "patch": patch,
        "snapshot": snapshot_json(&next, &patches, &events),
    })))
}

#[derive(Deserialize)]
pub struct DecideRequest {
    #[serde(default)]
    document_id: Option<String>,
    patch_id: String,
    /// `accept` or `reject`. An unknown value is refused rather than guessed.
    decision: String,
    #[serde(default)]
    actor: Option<String>,
}

/// POST /api/v1/document/decide — accept applies the patch; reject records the
/// rejection and changes zero document bytes (AD09).
pub async fn decide_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<DecideRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let uid = user_key(&headers);
    let document_id = validate_document_id(req.document_id.as_deref().unwrap_or(DEFAULT_DOCUMENT))
        .map_err(|e| bad_request(&e))?;
    let store = store_for(&state, &uid, &document_id);
    let _guard = state.documents.lock().unwrap_or_else(|e| e.into_inner());
    let (doc, _) = load_or_seed(&store, &uid).map_err(io_error)?;
    let mut patches = store.load_patches().map_err(io_error)?;
    let idx = patches
        .patches
        .iter()
        .position(|p| p.id == req.patch_id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "missing_patch" })),
            )
        })?;
    let patch = patches.patches[idx].clone();

    match req.decision.as_str() {
        "accept" => {
            let events = store.load_events().map_err(io_error)?;
            let sequence = events.last().map(|e| e.sequence + 1).unwrap_or(0);
            let actor = req.actor.clone().unwrap_or_else(|| uid.clone());
            let (next, change, applied) =
                apply_patch(&doc, &patch, &actor, sequence, now_secs()).map_err(apply_error)?;
            store.save_document(&next).map_err(io_error)?;
            store.append_event(&change).map_err(io_error)?;
            patches.patches[idx] = applied.clone();
            store.save_patches(&patches).map_err(io_error)?;
            Ok(Json(serde_json::json!({
                "patch": applied,
                "revision": next.revision,
                "snapshot": snapshot_json(&next, &patches, &store.load_events().map_err(io_error)?),
            })))
        }
        "reject" => {
            let rejected = reject_patch(&patch).map_err(apply_error)?;
            patches.patches[idx] = rejected.clone();
            store.save_patches(&patches).map_err(io_error)?;
            Ok(Json(serde_json::json!({
                "patch": rejected,
                "revision": doc.revision,
                "snapshot": snapshot_json(&doc, &patches, &store.load_events().map_err(io_error)?),
            })))
        }
        other => Err(bad_request(&format!(
            "decision must be 'accept' or 'reject', got '{other}'"
        ))),
    }
}

#[derive(Deserialize)]
pub struct UndoRequest {
    #[serde(default)]
    document_id: Option<String>,
    patch_id: String,
    #[serde(default)]
    actor: Option<String>,
}

/// POST /api/v1/document/undo — a compensating patch, never a revision rewind.
/// The original human edit survives; only the dependent repair is reverted
/// (§26.6, AD10).
pub async fn undo_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<UndoRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let uid = user_key(&headers);
    let document_id = validate_document_id(req.document_id.as_deref().unwrap_or(DEFAULT_DOCUMENT))
        .map_err(|e| bad_request(&e))?;
    let store = store_for(&state, &uid, &document_id);
    let _guard = state.documents.lock().unwrap_or_else(|e| e.into_inner());
    let (doc, _) = load_or_seed(&store, &uid).map_err(io_error)?;
    let mut patches = store.load_patches().map_err(io_error)?;
    let idx = patches
        .patches
        .iter()
        .position(|p| p.id == req.patch_id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "missing_patch" })),
            )
        })?;
    let patch = patches.patches[idx].clone();
    let events = store.load_events().map_err(io_error)?;
    let sequence = events.last().map(|e| e.sequence + 1).unwrap_or(0);
    let actor = req.actor.clone().unwrap_or_else(|| uid.clone());
    let (next, change, reverted) =
        undo_patch(&doc, &patch, &actor, sequence, now_secs()).map_err(apply_error)?;
    store.save_document(&next).map_err(io_error)?;
    store.append_event(&change).map_err(io_error)?;
    patches.patches[idx] = reverted.clone();
    store.save_patches(&patches).map_err(io_error)?;
    Ok(Json(serde_json::json!({
        "patch": reverted,
        "revision": next.revision,
        "snapshot": snapshot_json(&next, &patches, &store.load_events().map_err(io_error)?),
    })))
}

#[derive(Deserialize)]
pub struct ResetRequest {
    #[serde(default)]
    document_id: Option<String>,
}

/// POST /api/v1/document/reset — delete this tenant's managed document so the
/// next read reseeds the fixture. Explicit, because "reload and obtain the same
/// history" is the demo's point and an accidental reset would erase it.
pub async fn reset_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<ResetRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let uid = user_key(&headers);
    let document_id = validate_document_id(req.document_id.as_deref().unwrap_or(DEFAULT_DOCUMENT))
        .map_err(|e| bad_request(&e))?;
    let store = store_for(&state, &uid, &document_id);
    let _guard = state.documents.lock().unwrap_or_else(|e| e.into_inner());
    if store.root.exists() {
        std::fs::remove_dir_all(&store.root).map_err(io_error)?;
    }
    let (doc, _) = load_or_seed(&store, &uid).map_err(io_error)?;
    Ok(Json(snapshot_json(&doc, &Patches::default(), &[])))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adaptive_document::PatchStatus;

    #[test]
    fn document_ids_cannot_escape_the_tenant_directory() {
        assert!(validate_document_id("field-report").is_ok());
        assert!(validate_document_id("../../etc/passwd").is_err());
        assert!(validate_document_id("a/b").is_err());
        assert!(validate_document_id("").is_err());
    }

    /// AD04, at the level the handler composes it: the canonical fixture's
    /// confirmed copy and calculation produce exactly 3 and 6, and the
    /// review-only links invent no prose.
    #[test]
    fn canonical_fixture_proposes_exactly_three_and_six() {
        let (raw, links) = canonical_fixture();
        let mut doc = parse_blocks(DEFAULT_DOCUMENT, 0, &raw);
        doc.links = links;
        doc.graph_revision = 1;
        let source = doc.blocks[1].id.clone();
        let (next, change) =
            apply_source_edit(&doc, &source, "We ordered 3 units of bolts.", "test", 0, 1)
                .expect("the edit changes bytes");
        let patch = propose_after_source_edit(&next, &change, "p-test");

        let mut by_block: std::collections::BTreeMap<String, String> = Default::default();
        for r in &patch.replacements {
            by_block.insert(r.block_id.clone(), r.new_bytes.clone());
        }
        assert_eq!(
            by_block.get(&next.blocks[2].id).map(String::as_str),
            Some("We ordered 3 units of bolts."),
            "the copy dependent becomes the edited source"
        );
        assert_eq!(
            by_block.get(&next.blocks[3].id).map(String::as_str),
            Some("Budget is 6 euros."),
            "the calculation dependent is 2 x 3"
        );
        // The unrelated quotation (block 5) and the locked block (6) are never
        // in the replacement set.
        assert!(!by_block.contains_key(&next.blocks[5].id));
        assert!(!by_block.contains_key(&next.blocks[6].id));
        assert!(
            patch
                .review_only
                .iter()
                .any(|n| n.block_id == next.blocks[6].id && n.reason == "locked dependent"),
            "the locked dependent is flagged, not rewritten"
        );
        assert!(
            patch
                .review_only
                .iter()
                .any(|n| n.block_id == next.blocks[4].id),
            "the review-only prose is flagged"
        );
    }

    #[test]
    fn accepted_then_undone_keeps_the_source_edit() {
        let (raw, links) = canonical_fixture();
        let mut doc = parse_blocks(DEFAULT_DOCUMENT, 0, &raw);
        doc.links = links;
        doc.graph_revision = 1;
        let source = doc.blocks[1].id.clone();
        let (next, change) =
            apply_source_edit(&doc, &source, "We ordered 3 units of bolts.", "test", 0, 1).unwrap();
        let patch = propose_after_source_edit(&next, &change, "p-test");
        let (applied_doc, _, applied_patch) = apply_patch(&next, &patch, "test", 1, 2).unwrap();
        assert!(applied_doc.raw.contains("Budget is 6 euros."));
        let (undone, _, reverted) = undo_patch(&applied_doc, &applied_patch, "test", 2, 3).unwrap();
        assert!(undone.raw.contains("Budget is 36 euros."));
        assert!(
            undone.raw.contains("We ordered 3 units of bolts."),
            "undo must not revert the human edit"
        );
        assert_eq!(reverted.status, PatchStatus::Reverted);
    }
}
