use super::*;
use crate::web::{sandbox_existing, SharedState};
use axum::{extract::{Json, Query, State, Request}, response::{IntoResponse, Response}, http::{StatusCode, HeaderMap}, routing::{get, post}};
use serde::{Deserialize, Serialize};

pub async fn health() -> Json<&'static str> {
    Json("ok")
}

pub async fn embedder_info_handler(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = &state.shared;
    Json(serde_json::json!({
        "kind": s.embedder_kind,
        "dim": s.embed_dim,
        "degraded": s.degraded,
        "semantic": !s.degraded,
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

pub async fn graph_fm_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/console.html"))
}

pub async fn demo_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/demo.html"))
}

pub async fn study_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/study.html"))
}

