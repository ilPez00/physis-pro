use super::*;
use crate::web::SharedState;
use axum::{
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
};

/// Ontology discovery (PLAN 1D): which of the supplied texts does the current
/// grid cover poorly, and what draft entries would cover them? Stateless —
/// embeds off-thread against the shared classifier; returns the report plus a
/// ready-to-edit ontology JSON draft.
pub async fn discover_handler(
    State(state): State<SharedState>,
    Json(req): Json<DiscoverRequest>,
) -> Json<serde_json::Value> {
    let shared = state.shared.clone();
    let kind = req.kind.unwrap_or_else(|| "discovered".to_string());
    let mut cfg = crate::discovery::DiscoveryConfig::default();
    if let Some(t) = req.coverage_threshold {
        cfg.coverage_threshold = t;
    }
    let (report, draft) = tokio::task::spawn_blocking(move || {
        let report =
            crate::discovery::discover(&req.texts, &shared.classifier, &shared.embedder, &cfg);
        let draft = crate::discovery::to_ontology_json(&report, &kind);
        (report, draft)
    })
    .await
    .expect("discovery task completes");
    Json(serde_json::json!({ "report": report, "ontology_draft": draft }))
}

/// POST /api/v1/planner/simulate — classify a bounded corpus with the live
/// ontology and a candidate ontology, then return the counterfactual impact.
/// The candidate classifier is built inside the request and discarded; live
/// state, quality feedback, and tenant graphs are never mutated.
pub async fn planner_simulate_handler(
    State(state): State<SharedState>,
    Json(req): Json<PlannerSimulationRequest>,
) -> Result<Json<PlannerSimulation>, (StatusCode, Json<ErrorResponse>)> {
    if req.texts.is_empty() || req.texts.len() > 10_000 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "texts must contain 1..10000 records".into(),
            }),
        ));
    }
    let shared = state.shared.clone();
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<PlannerSimulation> {
        let (_, _, candidate_domains) = OntologyLoader::load_str(
            &req.candidate_ontology,
            &format!("planner:{}", req.scenario_id),
        )?;
        if candidate_domains.is_empty() {
            anyhow::bail!("candidate ontology contains no domains");
        }
        let mut candidate = OntologyLoader::new();
        candidate.custom_domains = candidate_domains;
        let candidate_classifier = CellClassifier::build(&candidate, shared.embedder.as_ref());
        let mut items = Vec::with_capacity(req.texts.len());

        for text in req.texts {
            let embedding = shared.embedder.embed(&text.source);
            let before = shared
                .classifier
                .classify(&embedding)
                .into_iter()
                .next()
                .map(|c| CellOutcome {
                    domain: c.domain,
                    mode: c.mode,
                    score: c.score,
                })
                .unwrap_or_else(|| CellOutcome {
                    domain: "unknown".into(),
                    mode: "unknown".into(),
                    score: 0.0,
                });
            let after = candidate_classifier
                .classify(&embedding)
                .into_iter()
                .next()
                .map(|c| CellOutcome {
                    domain: c.domain,
                    mode: c.mode,
                    score: c.score,
                })
                .unwrap_or_else(|| CellOutcome {
                    domain: "unknown".into(),
                    mode: "unknown".into(),
                    score: 0.0,
                });
            items.push(PlannerItem {
                id: text.id,
                source: text.source,
                before,
                after,
            });
        }
        Ok(planner::simulate(
            req.scenario_id,
            items,
            req.confidence_threshold,
        ))
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("planner task failed: {e}"),
            }),
        )
    })?
    .map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(result))
}

pub async fn operational_timeline_demo_handler() -> Json<OperationalTimeline> {
    let events: Vec<OperationalEvent> = serde_json::from_str(OPERATIONAL_TIMELINE_DEMO)
        .expect("embedded operational timeline demo fixture is valid");
    Json(OperationalTimeline::from_events(&events))
}

pub async fn operational_timeline_demo_events_handler() -> Json<Vec<OperationalEvent>> {
    let events: Vec<OperationalEvent> = serde_json::from_str(OPERATIONAL_TIMELINE_DEMO)
        .expect("embedded operational timeline demo fixture is valid");
    Json(events)
}

pub async fn operational_timeline_handler(
    Json(req): Json<OperationalTimelineRequest>,
) -> Json<OperationalTimeline> {
    Json(OperationalTimeline::from_events(&req.events))
}

pub async fn context_filter_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<ContextFilterRequest>,
) -> Json<FilterResponse> {
    // Embed off-thread before taking the lock — the ONNX embed is the slow part
    // and must not serialize other handlers behind the global state mutex.
    let token_estimate = req.input.split_whitespace().count();
    let embedding = embed_text(state.shared.clone(), req.input.clone()).await;
    let uid = user_key(&headers);
    let result = state.with_user(&uid, |s| {
        s.core.filter_context_embedded(embedding, token_estimate)
    });
    Json(FilterResponse {
        embedding: result.embedding,
        valid: result.valid,
        token_estimate: result.token_estimate,
    })
}

pub async fn compress_logs_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CompressLogsRequest>,
) -> Json<CompressResponse> {
    let uid = user_key(&headers);
    let compressed = state.with_user(&uid, |s| s.core.compress_logs(&req.logs));
    Json(CompressResponse {
        input_count: req.logs.len(),
        output_chars: compressed.len(),
        compressed,
    })
}

pub async fn translate_handler(
    State(state): State<SharedState>,
    Json(req): Json<TranslateRequest>,
) -> Json<TranslateResponse> {
    let router = LinguisticRouter::with_config(&state.shared.config.linguistic);
    let mut results = std::collections::HashMap::new();
    match req.lense {
        Some(ref l) => {
            let lense = match l.to_lowercase().as_str() {
                "wenyan" => LinguisticLense::Wenyan,
                "piraha" => LinguisticLense::Piraha,
                "sanskrit" => LinguisticLense::Sanskrit,
                _ => {
                    results.insert(
                        "error".to_string(),
                        format!("Unknown lense: {l}. Use: wenyan, piraha, sanskrit"),
                    );
                    return Json(TranslateResponse { results });
                }
            };
            results.insert(lense.as_str().to_string(), router.route(&req.text, lense));
        }
        None => {
            for (lense, text) in router.route_all(&req.text) {
                results.insert(lense.as_str().to_string(), text);
            }
        }
    }
    Json(TranslateResponse { results })
}

/// Re-read `<data_dir>/config.json` and apply the live-tunable knobs to the
/// tenant's PDCA actor (stagnation threshold + window). Structural params
/// (embedder, dim, scan_root) are fixed at startup and need a restart — say so.
pub async fn config_reload_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<ConfigReloadResponse> {
    let path = state.shared.config.data_dir.join("config.json");
    let cfg = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<PhysisConfig>(&s).ok());
    match cfg {
        Some(c) => {
            let uid = user_key(&headers);
            state.with_user(&uid, |s| {
                s.actor = PDCActor::new(c.pdca_stagnant_threshold, c.pdca_stagnant_window);
            });
            // scan_root reloads live (behind its RwLock): PHYSIS_SCAN_ROOT env, else
            // the reloaded config's data_dir. Canonicalized like at startup.
            let new_root = std::env::var("PHYSIS_SCAN_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|_| c.data_dir.clone());
            let new_root = new_root.canonicalize().unwrap_or(new_root);
            *state.scan_root.write().unwrap_or_else(|e| e.into_inner()) = new_root.clone();
            Json(ConfigReloadResponse {
                reloaded: true,
                pdca_threshold: c.pdca_stagnant_threshold,
                pdca_window: c.pdca_stagnant_window,
                note: format!("applied live: PDCA knobs + scan_root={}. embedder/embed_dim need a restart (dim change corrupts vectors)", new_root.display()),
            })
        }
        None => Json(ConfigReloadResponse {
            reloaded: false,
            pdca_threshold: state.shared.config.pdca_stagnant_threshold,
            pdca_window: state.shared.config.pdca_stagnant_window,
            note: format!("no readable config.json at {}", path.display()),
        }),
    }
}

/// Nearest nodes to `input` within the caller's tenant. This read the default
/// tenant's core regardless of `X-Physis-User`, so a client that wrote its nodes
/// under its own id (aion, praxis) always got an empty reconstruction back —
/// the write half of the loop worked and the read half silently returned nothing.
pub async fn reconstruct_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<ContextFilterRequest>,
) -> Json<crate::Reconstruction> {
    let uid = user_key(&headers);
    let rec = state.with_user(&uid, |s| {
        crate::reconstruct(&req.input, &state.shared.embedder, &s.core, 5)
    });
    Json(rec)
}
