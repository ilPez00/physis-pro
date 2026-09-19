use super::*;
use crate::web::{sandbox_existing, SharedState};
use axum::{
    extract::{Json, Query, State},
    http::{HeaderMap, StatusCode},
};

/// Reports the live embedder provenance so degraded (random-projection) mode is
/// observable instead of silent. See PREMORTEM.md P0.
pub async fn embedder_info_handler(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = &state.shared;
    Json(serde_json::json!({
        "kind": s.embedder_kind,
        "dim": s.embed_dim,
        "degraded": s.degraded,
        // Read the stored flag rather than re-deriving it from `degraded`. Since
        // the two were decoupled, `!degraded` is no longer the same fact:
        // explicitly requested random-projection offline mode is non-semantic
        // yet NOT degraded, so `!degraded` reported it as a real semantic model
        // — the exact silent-degradation blindness this endpoint exists to stop.
        "semantic": s.semantic,
        "self_test": s.self_test,
    }))
}

pub async fn chat_handler(
    State(state): State<SharedState>,
    Json(req): Json<ChatApiRequest>,
) -> Result<Json<ChatApiResponse>, StatusCode> {
    let cascade = state.shared.cascade.clone();
    let hint = {
        let task = req.task_type.as_deref().unwrap_or("smart");
        crate::ai::provider::RouteHint {
            task_type: task.to_string(),
            has_image: req.has_image,
        }
    };

    // Optional token-fixed RAG grounding: retrieve a context capped to a fixed
    // token budget and inject it as a system message, so the LLM's input-context
    // cost is bounded and independent of source size.
    let (messages, rag_tokens) = match &req.rag_dir {
        Some(dir) => {
            let path = match sandbox_existing(&state, dir) {
                Ok(p) => p,
                Err((st, _)) => return Err(st),
            };
            let shared = state.shared.clone();
            let query = last_user_text(&req.messages);
            let (context, total) =
                match tokio::task::spawn_blocking(move || -> anyhow::Result<(String, usize)> {
                    let docs = crate::ingest::ingest_all(&[path]);
                    if docs.is_empty() {
                        anyhow::bail!("no documents found under scan dir");
                    }
                    let texts: Vec<String> = docs.iter().map(|d| d.body.clone()).collect();
                    let corpus = RagCorpus::build(&texts, shared.embedder.as_ref());
                    let q_emb = shared.embedder.embed(&query);
                    let retriever = rag::TokenFixedRetriever::new(req.rag_budget, req.rag_top_k);
                    let result = retriever.retrieve(&q_emb, &corpus);
                    Ok((
                        rag::assemble_context(&result, "\n\n---\n\n"),
                        result.total_tokens,
                    ))
                })
                .await
                {
                    Ok(Ok(v)) => v,
                    Ok(Err(e)) => {
                        log::warn!("[chat] rag failed: {}", e);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                    Err(e) => {
                        log::warn!("[chat] rag task failed: {}", e);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                };
            let sys = crate::ai::provider::Message {
                role: "system".into(),
                content: crate::ai::provider::Content::Text(format!(
                    "Answer using ONLY the retrieved context below. If the context \
                     does not contain the answer, say so explicitly.\n\n\
                     --- RETRIEVED CONTEXT (token-budgeted) ---\n{context}\n\
                     --- END CONTEXT ---"
                )),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            };
            let mut msgs = req.messages.clone();
            msgs.insert(0, sys);
            (msgs, Some(total))
        }
        None => (req.messages.clone(), None),
    };

    match cascade
        .complete(&messages, None, &hint, req.max_tokens)
        .await
    {
        Ok((msg, provider)) => Ok(Json(ChatApiResponse {
            content: msg.content.unwrap_or_default(),
            provider,
            tool_calls: msg
                .tool_calls
                .as_ref()
                .map(|tc| serde_json::to_value(tc).unwrap_or_default()),
            rag_tokens,
        })),
        Err(e) => {
            log::warn!("[chat] all providers failed: {}", e);
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

pub async fn providers_handler(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let cascade = state.shared.cascade.clone();
    Json(serde_json::json!({ "providers": cascade.providers_list() }))
}

pub async fn stats_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Json<StatsResponse> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        Json(StatsResponse {
            mapper: s.mapper.stats(),
            core: s.core.snapshot(),
        })
    })
}

pub async fn query_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<QueryParams>,
) -> Json<QueryResponse> {
    let uid = user_key(&headers);
    state.with_user(&uid, |s| {
        let results = s.mapper.query(&params.q);
        let limited: Vec<Vec<String>> = results.into_iter().take(params.max).collect();
        Json(QueryResponse {
            count: limited.len(),
            results: limited,
        })
    })
}

/// GET /physis.css — shared design system ("Bioluminal Forge"). Public like
/// ui-shell.js: stylesheets cannot send Bearer headers either.
pub async fn physis_css_handler() -> impl axum::response::IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../bin/ui/physis.css"),
    )
}

/// GET /ui — the graph file-manager single-page app (public; calls the
/// authenticated fs/* endpoints with a token the user pastes in the page).
pub async fn graph_fm_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/graph_fm.html"))
}

/// GET /search — search by content: every section of every file under the scan
/// root, ranked by keyword and meaning together, with a facet rail and a
/// preview pane. The other surfaces rank labels; this one ranks what the
/// documents actually say.
pub async fn search_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/search.html"))
}

/// GET /organic.js — the self-hosted force-directed canvas renderer.
///
/// Public for the same reason as ui-shell.js: a `<script src>` cannot send a
/// Bearer header. It ships no data, only a simulation. Self-hosted rather than
/// CDN-loaded because the air-gapped installs this engine is sold into would
/// otherwise render an empty graph — the one deployment whose entire promise is
/// that nothing leaves the machine.
pub async fn organic_js_handler() -> impl axum::response::IntoResponse {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        include_str!("../bin/ui/organic.js"),
    )
}

/// GET /atlas — the organic knowledge graph: communities as soft fields,
/// recorded edges distinguished from resemblance, and an object explorer for
/// whatever node you click.
pub async fn atlas_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/atlas.html"))
}

/// GET /studio — the ontology editor: browse installed packs, author terms
/// against the 70-cell grid, test a draft against real text before saving it.
pub async fn studio_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/studio.html"))
}

/// GET /notebook — the dashboard the user composes: an ordered grid of cells,
/// each naming a widget from the registry. The page holds the renderers; the
/// notebook holds only references, so a layout shared between people carries
/// panel names and settings and never code.
pub async fn notebook_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/notebook.html"))
}

/// GET /console — the operator console (interest feed, overview, timeline,
/// OEE, classify, meaning grid, planner, hypotheses, ...).
pub async fn console_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/console.html"))
}

/// GET /pulse — the overview dashboard: coherence KPIs and distribution, the
/// dream-replay attention feed, communities, the wiki execution tour and the
/// semiotic heatmap. Read-only over the tenant's live state.
pub async fn pulse_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/pulse.html"))
}

/// GET /impossible — the workspace as a graphical, clickable surface: the
/// intent and its provenance, the agent roster, the task graph, the action
/// capabilities. Rendered from `/api/v1/workspace`, every row a hyperlink.
pub async fn impossible_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/impossible.html"))
}

/// GET /api/v1/workspace — the state the Impossible Computer page renders.
///
/// Read-only, and every value is a fact the page can link to. `?root=` selects
/// the workspace; it defaults to the server's working directory. The task graph
/// is folded from the observation log at read time, so the page cannot show a
/// graph the log does not support.
pub async fn workspace_handler(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<serde_json::Value> {
    use crate::agents::{self, tasks::TaskGraph};
    use crate::capability::default_capabilities;
    use crate::intent::IntentStore;

    let root = params
        .get("root")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        });

    let store = IntentStore::load(&root);
    let intents: Vec<serde_json::Value> = store
        .intents
        .values()
        .map(|i| {
            serde_json::json!({
                "id": i.id,
                "statement": i.statement,
                "goal_id": i.goal_id,
                "edits": i.history.len(),
                "history": i.history.iter().map(|e| serde_json::json!({
                    "at": e.at.to_rfc3339(),
                    "by": e.by,
                    "from": e.from,
                    "to": e.to,
                    "reason": e.reason,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();

    let roster = agents::default_roster();
    let agent_rows: Vec<serde_json::Value> = roster
        .agents
        .iter()
        .map(|a| {
            let s = agents::status(a);
            serde_json::json!({
                "id": a.id,
                "name": a.name,
                "state": a.state.label(),
                "transport": s.transport,
                "available": s.available,
                "reason": s.reason,
                "capabilities": a.capabilities,
            })
        })
        .collect();

    let log = root.join(".physis").join("observations.jsonl");
    let task_graph = match std::fs::read_to_string(&log) {
        Ok(raw) => {
            let obs: Vec<physis_core::observe::Observation> = raw
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect();
            let g = TaskGraph::fold(&obs);
            serde_json::json!({
                "observations": obs.len(),
                "tasks": g.tasks.values().map(|t| serde_json::json!({
                    "id": t.id,
                    "title": t.title,
                    "state": format!("{:?}", t.state).to_lowercase(),
                    "parent": t.parent,
                    "children": t.children,
                    "verified": t.verified(),
                    "reported": t.reported,
                    "expired_reason": t.expired_reason,
                })).collect::<Vec<_>>(),
            })
        }
        Err(_) => serde_json::json!({ "observations": 0, "tasks": [] }),
    };

    let capabilities: Vec<serde_json::Value> = default_capabilities()
        .iter()
        .map(|c| {
            serde_json::json!({
                "name": c.name,
                "verbs": c.verbs,
                "kinds": c.object_kinds,
                "risk": c.risk,
                "tier": c.tier,
            })
        })
        .collect();

    Json(serde_json::json!({
        "root": root.display().to_string(),
        "intents": intents,
        "agents": agent_rows,
        "task_graph": task_graph,
        "capabilities": capabilities,
    }))
}

/// GET /dashboard — the unified read-only dashboard: health, stats, quality,
/// communities, coherence snapshot, embedder, providers, static MCP tool list.
pub async fn dashboard_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/dashboard.html"))
}

/// GET /ui-shell.js — the runtime every dashboard page loads with a plain
/// `<script src="ui-shell.js">`. It must stay on the PUBLIC router: a script
/// tag cannot send the Bearer header the protected routes demand, so gating it
/// would 401 the file and leave Console, Demo and Study with no `$`, `toast`
/// or `getJSON` at all. It ships no data — only functions.
pub async fn ui_shell_js_handler() -> impl axum::response::IntoResponse {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        include_str!("../bin/ui/ui-shell.js"),
    )
}

/// GET /sunflower.js — self-contained sunflower branding (base64 PNG, no
/// external asset). Injects the logo as favicon and replaces any `.logo`
/// box (letter mark) in the served dashboards with the sunflower image.
pub async fn sunflower_js_handler() -> impl axum::response::IntoResponse {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        include_str!("../bin/ui/sunflower.js"),
    )
}

/// The product demo dashboard — a single self-contained page wired to the live
/// engine: plant OEE, machines, shift report, operational timeline, semiotic
/// heatmap, holarchy graph, and the classify probes. "Seed demo data" drives
/// the real endpoints, so everything on screen is computed live in-process.
pub async fn demo_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/demo.html"))
}

/// GET /study — the learning / teaching dashboard. A student's Study Log
/// (timeline in order, files by type/directory, generated-vs-edited split,
/// chronos study rhythms, place/date context) plus the elenchus argument
/// trainer where you tag claims/warrants/rebuttals and see the standings.
pub async fn study_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/study.html"))
}

/// GET / — the public landing page. The one surface that names what the product
/// actually is: adaptive industrial intelligence that runs air-gapped. Kept
/// public (it is marketing, not state) and free of the product shell's ask bar.
pub async fn landing_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/index.html"))
}

/// GET /pricing — the service-led offer ladder (Pilot → Industriale →
/// Enterprise). Supersedes the per-job SaaS tiers.
pub async fn pricing_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/pricing.html"))
}

/// GET /about — capabilities, run surfaces, licensing. The honest description,
/// not "pay-per-job API platform".
pub async fn about_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/about.html"))
}

/// GET /deploy — this instance's health and embedder provenance, read from the
/// live public endpoints only. A license is an offline key, not an account, so
/// the page carries no billing backend.
pub async fn deployment_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/customer_dashboard.html"))
}

/// GET /settings — the operator console for the install itself.
pub async fn settings_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/settings.html"))
}

/// GET /api/v1/settings — one call answering "is this install healthy, and is
/// it current?".
///
/// Six separate fetches would each need their own failure copy, and a settings
/// screen whose own error state is invisible is the worst kind — so this never
/// fails. A missing licence, an unopenable store and a degraded embedder all
/// come back as *values*, not as a 500, because the whole point of the page is
/// to show the operator what is wrong.
///
/// The version comparison is deliberately **not** done here: this product is
/// sold into air-gapped networks, and an engine that phones home to check for
/// updates is the wrong default. `/settings` asks crates.io from the browser
/// instead, and reports honestly when it cannot reach it.
pub async fn settings_handler(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = &state.shared;
    let scan_root = state
        .scan_root
        .read()
        .map(|g| g.display().to_string())
        .unwrap_or_else(|_| s.config.data_dir.display().to_string());
    let tenants = state.tenants.lock().map(|t| t.len()).unwrap_or(0);

    let license = crate::license::load().ok().map(|l| {
        serde_json::json!({
            "tier": l.tier().name(),
            "sku": l.tier().sku(),
            "org": l.org,
            "seats": l.seats,
            "features": l.features,
            "issued": l.iat,
            "expires": l.exp,
            "maintenance_expires": l.maint_exp,
            "days_remaining": l.days_remaining(),
            "revoked": l.revoked,
            "banner": l.banner(),
        })
    });

    // Effective settings + provenance: which layer set each value. Never 500s:
    // a broken config.json is the operator's problem to see, not a handler
    // crash — report it as a value like every other health fact here.
    // The JEV key is a secret: ship presence only, never the value.
    let (effective, origins, config_error) = match crate::config::PhysisConfig::load_effective() {
        Ok(eff) => {
            let mut v = serde_json::to_value(&eff.config).unwrap_or(serde_json::Value::Null);
            if let Some(agent) = v.get_mut("agent") {
                if let Some(key) = agent.get_mut("jev_api_key") {
                    *key = serde_json::Value::String(
                        if key.as_str().is_some_and(|k| !k.trim().is_empty()) {
                            "<set>".to_string()
                        } else {
                            "<unset>".to_string()
                        },
                    );
                }
            }
            let o: std::collections::HashMap<String, String> = eff
                .origins
                .iter()
                .map(|(k, o)| (k.clone(), o.to_string()))
                .collect();
            (
                v,
                serde_json::to_value(o).unwrap_or(serde_json::Value::Null),
                None,
            )
        }
        Err(e) => (
            serde_json::to_value(&s.config).unwrap_or(serde_json::Value::Null),
            serde_json::Value::Null,
            Some(e.to_string()),
        ),
    };

    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "crate": "physis_pro",
        "engine": {
            "data_dir": s.config.data_dir.display().to_string(),
            "scan_root": scan_root,
            // sled open failed (read-only volume, no data dir) → everything is
            // in-memory and will not survive a restart. That is a health fact
            // an operator must see, not an internal detail.
            "persistence": state.db.is_some(),
            "tenants": tenants,
            "dev_mode": s.dev_mode,
            "watch_dir": std::env::var("PHYSIS_WATCH_DIR").ok(),
            "watch_interval_secs": std::env::var("PHYSIS_WATCH_INTERVAL_SECS").ok(),
        },
        "embedder": {
            "kind": s.embedder_kind,
            "dim": s.embed_dim,
            "semantic": s.semantic,
            "degraded": s.degraded,
            "self_test": s.self_test,
        },
        "effective_config": effective,
        "config_origins": origins,
        "config_error": config_error,
        "license": license,
    }))
}
