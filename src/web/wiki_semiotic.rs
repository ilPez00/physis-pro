use super::*;
use crate::web::SharedState;
use crate::web::{WikiIndexRequest, WikiSearchRequest};
use axum::{extract::{Json, Query, State}, http::{StatusCode, HeaderMap}};

/// Image (base64) → `embed_image`, else text — off-thread.
pub async fn embed_input(shared: Arc<SharedCtx>, text: String, image: Option<String>) -> Vec<f32> {
    tokio::task::spawn_blocking(move || {
        if let Some(b64) = image {
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&b64) {
                if let Some(v) = shared.embedder.embed_image(&bytes) {
                    return v;
                }
                log::warn!("image embedding unavailable; falling back to text");
            } else {
                log::warn!("base64 decode failed; falling back to text");
            }
        }
        shared.embedder.embed(&text)
    })
    .await
    .unwrap_or_default()
}

pub async fn ontology_list_handler(State(state): State<SharedState>) -> Json<Vec<OntologyListing>> {
    let s = &state.shared;
    Json(vec![
        OntologyListing {
            kind: "human".into(),
            count: s.ontology.human_domains.len(),
        },
        OntologyListing {
            kind: "machine".into(),
            count: s.ontology.machine_domains.len(),
        },
        OntologyListing {
            kind: "semiotic".into(),
            count: s.ontology.semiotic_domains.len(),
        },
        OntologyListing {
            kind: "category".into(),
            count: s.ontology.category_domains.len(),
        },
        OntologyListing {
            kind: "agent".into(),
            count: s.ontology.agent_domains.len(),
        },
        OntologyListing {
            kind: "natural".into(),
            count: s.ontology.natural_domains.len(),
        },
        OntologyListing {
            kind: "social".into(),
            count: s.ontology.social_domains.len(),
        },
        OntologyListing {
            kind: "abstract".into(),
            count: s.ontology.abstract_domains.len(),
        },
        OntologyListing {
            kind: "engineering".into(),
            count: s.ontology.engineering_domains.len(),
        },
    ])
}

/// What an item means in each context that defines it.
///
/// The point of overlays is that a name is not one thing: "Margin" is a
/// machining allowance to one client and a profit line to another. This
/// endpoint answers "what does this mean where?" rather than "what does this
/// mean", and lists the contested names — those defined in more than one
/// place — because those are the only ones where the question has an answer
/// worth reading.
pub async fn ontology_meaning_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<MeaningParams>,
) -> Json<serde_json::Value> {
    let s = &state.shared;
    let overlays = crate::context::Overlays::load(&s.config.data_dir);

    let available: Vec<serde_json::Value> = overlays
        .overlays
        .iter()
        .map(|o| {
            serde_json::json!({
                "scope": o.scope.as_str(),
                "id": o.id,
                "entries": o.domains.len(),
                "declares_interpretant": o.interpretant.is_some(),
            })
        })
        .collect();

    let contested = overlays.contested_names(&s.ontology);

    let meanings = params.name.as_deref().map(|name| {
        overlays
            .meanings(name, &s.ontology)
            .into_iter()
            .map(|(prov, def)| {
                serde_json::json!({
                    "scope": prov.scope.as_str(),
                    "source": prov.source,
                    "domain": def.domain,
                    "mode": def.mode,
                    "category": def.category,
                    "hints": def.hints,
                })
            })
            .collect::<Vec<_>>()
    });

    // What the interpretant loop has learned, for the calling tenant in the
    // ambient context. Scoped by uid as well as context — reporting another
    // tenant's accumulated evidence would leak their working habits.
    let uid = user_key(&headers);
    let ctx = crate::context::Context::from_env();
    let interp = overlays.interpretant(&ctx, s.ontology.interpretant("human"));
    let store = state.salience.lock().unwrap_or_else(|e| e.into_inner());
    let ranked: Vec<serde_json::Value> = store
        .ranked(&uid, &ctx, &interp)
        .into_iter()
        .take(40)
        .map(|(name, score, obs)| {
            serde_json::json!({ "entry": name, "salience": score, "observations": obs })
        })
        .collect();

    Json(serde_json::json!({
        "overlays": available,
        "contested": contested,
        "name": params.name,
        "meanings": meanings,
        "overlay_dir": s.config.data_dir.join("ontologies"),
        "context": ctx.label(),
        "tenant": uid,
        "salience": ranked,
        "bias_strength": crate::salience::BIAS_STRENGTH,
    }))
}

/// Everything the ontology studio needs in one call: every authored entry,
/// which ontology it came from, whether classification can reach it, and
/// whether it carries any facet.
pub async fn ontology_studio_handler(State(state): State<SharedState>) -> Json<StudioResponse> {
    let s = &state.shared;
    let o = &s.ontology;

    // Names the classifier actually scores against, so each entry can be
    // marked reachable or not.
    let reachable: std::collections::HashSet<&str> = o
        .classification_domains()
        .map(|d| d.name.as_str())
        .collect();

    let sources: [(
        &str,
        &std::collections::HashMap<String, crate::models::DomainDef>,
    ); 11] = [
        ("human", &o.human_domains),
        ("custom", &o.custom_domains),
        ("machine", &o.machine_domains),
        ("semiotic", &o.semiotic_domains),
        ("category", &o.category_domains),
        ("agent", &o.agent_domains),
        ("natural", &o.natural_domains),
        ("social", &o.social_domains),
        ("abstract", &o.abstract_domains),
        ("engineering", &o.engineering_domains),
        ("office", &o.office_domains),
    ];

    let mut entries = Vec::new();
    let mut kinds = Vec::new();
    // `custom` holds all 20 extra ontologies keyed `"<ontology>:<name>"`.
    // Reporting them as one bucket hides 20 different interpretants, so they
    // are split back out by that prefix.
    let mut custom_counts: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    for (kind, map) in sources {
        let mut reach = 0usize;
        for (key, def) in map.iter() {
            let classified = reachable.contains(def.name.as_str());
            if classified {
                reach += 1;
            }
            // Entries from the extra ontologies report their own source rather
            // than the shared `custom` bucket, so the studio can attribute each
            // one to the interpretant that actually governs it.
            let source = match (kind, key.split_once(':')) {
                ("custom", Some((src, _))) => {
                    let e = custom_counts.entry(src.to_string()).or_insert((0, 0));
                    e.0 += 1;
                    e.1 += usize::from(classified);
                    src.to_string()
                }
                _ => kind.to_string(),
            };
            let f = &def.facets;
            let faceted = f.sub_domain.is_some()
                || f.sub_mode.is_some()
                || f.lifecycle.is_some()
                || f.agency.is_some()
                || f.scale.is_some()
                || f.abstraction.is_some();
            entries.push(StudioEntry {
                name: def.name.clone(),
                kind: source,
                domain: def.domain.clone(),
                mode: def.mode.clone(),
                category: def.category.clone(),
                hints: def.hints.clone(),
                facets: def.facets.clone(),
                classified,
                faceted,
            });
        }
        // The `custom` bucket is reported as its constituent ontologies below.
        if kind == "custom" {
            continue;
        }
        let i = o.interpretant(kind);
        kinds.push(serde_json::json!({
            "kind": kind, "count": map.len(), "classified": reach,
            "interpretant": {
                "confirms": i.confirms, "refutes": i.refutes,
                "half_life_days": i.half_life_days, "weight": i.weight,
                "note": i.note,
                // Distinguishes "declared this explicitly" from "inherited the
                // default", which the studio shows differently: an inherited
                // rule is an unanswered question, not a decision.
                "declared": o.interpretants.contains_key(kind),
            },
        }));
    }

    for (src, (count, reach)) in custom_counts {
        let i = o.interpretant(&src);
        kinds.push(serde_json::json!({
            "kind": src, "count": count, "classified": reach,
            "interpretant": {
                "confirms": i.confirms, "refutes": i.refutes,
                "half_life_days": i.half_life_days, "weight": i.weight,
                "note": i.note,
                "declared": o.interpretants.contains_key(&src),
            },
        }));
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let total = entries.len();
    let classified = entries.iter().filter(|e| e.classified).count();
    let faceted = entries.iter().filter(|e| e.faceted).count();
    Json(StudioResponse {
        entries,
        kinds,
        total,
        classified,
        faceted,
    })
}

/// The DOMAIN x MODE grid with the entries that populate each cell.
///
/// Built from `all_domains()` — every loaded ontology — rather than a hand-kept
/// list of collections. The previous version enumerated eight maps by hand and
/// omitted `custom_domains`, which is where the 20 always-on EXTRA_ONTOLOGIES
/// land and what `classification_domains()` scores against. The grid therefore
/// reported 27 of 70 cells populated while `classify` was happily placing text
/// in the other 43 — e.g. FABRICATE x MAINTAIN ("Machine Maintenance") read as
/// empty here yet matched real input. `office_domains` and `machine_domains`
/// were missing for the same reason.
pub async fn semiotic_grid_handler(State(state): State<SharedState>) -> Json<SemioticGridResponse> {
    let s = &state.shared;
    let mut grid = SemioticGrid::new();
    for def in s.ontology.all_domains().values() {
        if let (Some(d), Some(m)) = (
            def.domain.as_ref().and_then(|d| HumanDomain::from_str(d)),
            def.mode.as_ref().and_then(|m| HumanMode::from_str(m)),
        ) {
            grid.classify(&def.name, d, m);
        }
    }
    let cells: Vec<serde_json::Value> = grid
        .cells
        .iter()
        .map(|c| {
            serde_json::json!({
                "domain": c.domain.as_str(),
                "mode": c.mode.as_str(),
                "entries": c.entries,
                "activation": c.activation,
            })
        })
        .collect();
    Json(SemioticGridResponse { cells })
}

pub async fn semiotic_triangle_handler() -> Json<SemioticTriangleResponse> {
    let mermaid = crate::output::format_semiotic_triangle(
        "Peircean",
        "Sign\n(Quality/Feeling)",
        "Object\n(Fact/Reality)",
        "Interpretant\n(Thought/Meaning)",
    );
    Json(SemioticTriangleResponse { mermaid })
}

pub async fn greimas_square_handler() -> Json<GreimasSquareResponse> {
    let mermaid =
        crate::output::format_greimas_square("Physis", "HEAL", "FABRICATE", "CONSTRUCT", "STUDY");
    Json(GreimasSquareResponse { mermaid })
}

pub async fn category_diagram_handler(
    Json(req): Json<CategoryDiagramRequest>,
) -> Json<CategoryDiagramResponse> {
    let objects: Vec<(&str, &str)> = req
        .objects
        .iter()
        .map(|o| (o.id.as_str(), o.label.as_str()))
        .collect();
    let morphisms: Vec<(&str, &str, &str)> = req
        .morphisms
        .iter()
        .map(|m| (m.from.as_str(), m.to.as_str(), m.label.as_str()))
        .collect();
    let mermaid = crate::output::format_category_diagram(&objects, &morphisms);
    Json(CategoryDiagramResponse { mermaid })
}

/// POST /api/v1/wiki/search — BM25 over an on-disk wiki.
///
/// Always mounted. Without `wiki-embedded` it answers with an explicit
/// "not enabled" body rather than a 404, so a caller can tell a build that
/// lacks the feature apart from a URL it got wrong.
pub async fn wiki_search_handler(Json(req): Json<WikiSearchRequest>) -> Json<serde_json::Value> {
    #[cfg_attr(not(feature = "wiki-embedded"), allow(unused_variables))]
    let root = PathBuf::from(&req.wiki_root);
    #[cfg(feature = "wiki-embedded")]
    match llmwiki_physis::search_bm25(&root, &req.query, req.top_k.unwrap_or(10)) {
        Ok(results) => Json(serde_json::json!({
            "count": results.len(),
            "results": llmwiki_physis::results_to_json(&results),
        })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string(), "results": [] })),
    }
    #[cfg(not(feature = "wiki-embedded"))]
    Json(serde_json::json!({ "error": "wiki-embedded feature not enabled", "results": [] }))
}

/// POST /api/v1/wiki/index — build or refresh the wiki index.
///
/// Mounted and answering unconditionally, for the reason given on
/// [`wiki_search_handler`].
pub async fn wiki_index_handler(Json(req): Json<WikiIndexRequest>) -> Json<serde_json::Value> {
    #[cfg_attr(not(feature = "wiki-embedded"), allow(unused_variables))]
    let root = PathBuf::from(&req.wiki_root);
    #[cfg(feature = "wiki-embedded")]
    match llmwiki_physis::index(&root, req.incremental) {
        Ok(s) => Json(serde_json::json!({
            "added": s.added, "updated": s.updated,
            "skipped": s.skipped, "removed": s.removed,
        })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
    #[cfg(not(feature = "wiki-embedded"))]
    Json(serde_json::json!({ "error": "wiki-embedded feature not enabled" }))
}

pub async fn classify_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<ClassifyRequest>,
) -> Json<ClassifyResponse> {
    let embedding = embed_input(state.shared.clone(), req.text.clone(), req.image.clone()).await;
    // Accumulated interpretant evidence adjusts the ranking within a bounded
    // budget — see salience::BIAS_STRENGTH. Scoped to the calling tenant: the
    // context comes from one process-wide env var, so without the uid one
    // customer's evidence would reorder another's results. With no observations
    // recorded this is byte-identical to the unbiased classify.
    let uid = user_key(&headers);
    let ctx = crate::context::Context::from_env();
    let overlays = crate::context::Overlays::load(&state.shared.config.data_dir);
    let interp = overlays.interpretant(&ctx, state.shared.ontology.interpretant("human"));
    let mut results = {
        let store = state.salience.lock().unwrap_or_else(|e| e.into_inner());
        state.shared.classifier.classify_biased(&embedding, &uid, &ctx, &store, &interp)
    };
    let overlay_count = overlays.active_definitions(&ctx).len();
    results = state.shared.classifier.classify_with_overlays(
        &embedding,
        results,
        &overlays,
        &ctx,
        state.shared.embedder.as_ref(),
    );

    // Re-rank ambiguous, text-only cases with the LLM. Activated by the server
    // (PHYSIS_RERANK) OR by a caller-supplied key (BYOK) — the latter lets users
    // bring their own provider without a server key (and pay for their own calls).
    let byok = req.llm_key.as_ref().filter(|k| !k.is_empty()).map(|key| {
        let mut c = ProviderCascade::new();
        c.add(ProviderConfig {
            name: "byok".into(),
            base_url: req.llm_base_url.clone().unwrap_or_else(|| {
                "https://generativelanguage.googleapis.com/v1beta/openai".into()
            }),
            model: req
                .llm_model
                .clone()
                .unwrap_or_else(|| "gemini-2.5-flash".into()),
            api_key: key.clone(),
            supports_vision: false,
            supports_tools: false,
            max_tokens: 64,
            timeout_secs: 15.0,
        });
        c
    });
    let rerank_on = byok.is_some() || std::env::var("PHYSIS_RERANK").is_ok();
    if rerank_on && req.image.is_none() && results.len() >= 2 {
        let margin_thresh: f32 = std::env::var("PHYSIS_RERANK_MARGIN")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.05);
        if results[0].score - results[1].score < margin_thresh {
            let topk = results[..results.len().min(4)].to_vec();
            // BYOK uses the caller's provider with the "smart" hint (so their chosen
            // model isn't dropped by the fast-path gpt-4 filter); else the server
            // cascade on the "fast" path.
            let (cascade, hint): (&ProviderCascade, &str) = match &byok {
                Some(c) => (c, "smart"),
                None => (state.shared.cascade.as_ref(), "fast"),
            };
            if let Some((d, m)) = rerank_cell(cascade, &req.text, &topk, hint).await {
                if let Some(pos) = results.iter().position(|c| c.domain == d && c.mode == m) {
                    let chosen = results.remove(pos);
                    results.insert(0, chosen);
        }
    }
}

    }

    let top = results.first().cloned().unwrap_or(CellScore {
        domain: "unknown".into(),
        mode: "unknown".into(),
        score: 0.0,
        entries: vec![],
        facets: crate::models::Facets::default(),
    });
    let coverage = state
        .shared
        .classifier
        .best_entry_sim(&embedding)
        .map(|(score, _, _)| score)
        .unwrap_or(0.0);
    let assessment = ClassificationAssessment::from_results(
        &results,
        coverage,
        state.shared.embedder_kind,
        !state.shared.degraded && state.shared.self_test,
    );
    let discovery = if assessment.semantic && assessment.abstained {
        crate::discovery::suggest_text(&req.text, &embedding, &state.shared.classifier)
    } else {
        None
    };
    Json(ClassifyResponse {
        results,
        top,
        assessment,
        discovery,
        context: ctx.label(),
        overlay_count,
    })
}

pub async fn embed_handler(
    State(state): State<SharedState>,
    Json(req): Json<EmbedRequest>,
) -> Json<EmbedResponse> {
    let embedding = embed_input(state.shared.clone(), req.text.clone(), req.image.clone()).await;
    let dim = embedding.len();
    Json(EmbedResponse { embedding, dim })
}

/// Embed a video into the shared space: ffmpeg frame-sample → image-embed each →
/// temporal mean-pool. Runs on a blocking thread (ffmpeg + N inferences).
pub async fn embed_video_handler(
    State(state): State<SharedState>,
    Json(req): Json<VideoEmbedRequest>,
) -> Result<Json<VideoEmbedResponse>, (StatusCode, Json<ErrorResponse>)> {
    let opts = crate::video::VideoEmbedOptions {
        fps: req.fps.unwrap_or(1.0).clamp(0.1, 30.0),
        max_frames: req.max_frames.unwrap_or(32).clamp(1, 128),
    };

    enum Src {
        Path(PathBuf),
        Bytes(Vec<u8>, String),
    }

    let src = if let Some(p) = req.path {
        // Same path-traversal guard as /scan: confine to scan_root.
        let canonical = PathBuf::from(&p).canonicalize().map_err(|e| {
            err_response(
                StatusCode::BAD_REQUEST,
                &format!("cannot resolve path: {e}"),
            )
        })?;
        if !canonical.starts_with(state.scan_root()) {
            return Err(err_response(
                StatusCode::FORBIDDEN,
                &format!("path outside scan root ({})", state.scan_root().display()),
            ));
        }
        Src::Path(canonical)
    } else if let Some(b64) = req.video_base64 {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| {
                err_response(
                    StatusCode::BAD_REQUEST,
                    &format!("base64 decode failed: {e}"),
                )
            })?;
        Src::Bytes(bytes, req.ext.unwrap_or_else(|| "mp4".to_string()))
    } else {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            "provide `path` or `video_base64`",
        ));
    };

    let shared = state.shared.clone();
    let result = tokio::task::spawn_blocking(move || {
        let emb = shared.embedder.as_ref();
        match src {
            Src::Path(p) => crate::video::embed_video_file(&p, emb, &opts),
            Src::Bytes(b, ext) => crate::video::embed_video_bytes(&b, &ext, emb, &opts),
        }
    })
    .await
    .map_err(|e| {
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("video task failed: {e}"),
        )
    })?;

    match result {
        Some(r) => Ok(Json(VideoEmbedResponse {
            dim: r.embedding.len(),
            embedding: r.embedding,
            frames: r.frames,
        })),
        None => Err(err_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "video embedding unavailable: ffmpeg missing, embedder has no image path, or no frames decoded",
        )),
    }
}

pub async fn heatmap_handler(State(state): State<SharedState>) -> Json<HeatmapResponse> {
    let s = &state.shared;
    let mut grid = SemioticGrid::new();
    for def in s.ontology.human_domains.values() {
        if let (Some(d), Some(m)) = (
            def.domain.as_ref().and_then(|d| HumanDomain::from_str(d)),
            def.mode.as_ref().and_then(|m| HumanMode::from_str(m)),
        ) {
            grid.classify(&def.name, d, m);
        }
    }
    let table = crate::output::format_heatmap_table(&grid);
    let matrix = grid.heatmap_matrix();
    Json(HeatmapResponse { table, matrix })
}

