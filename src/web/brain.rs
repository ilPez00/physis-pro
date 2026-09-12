//! HTTP surface of the offline document brain (`/brain` demo page + API).
//!
//! Routes (all inside the protected router, like /demo):
//! - `POST /api/v1/brain/seed`    — build the book (bundled POMA-3.8B vs D-14)
//! - `POST /api/v1/brain/ask`     — grounded question answering
//! - `POST /api/v1/brain/faceoff` — agreement/contradiction report
//! - `GET  /api/v1/brain/status`  — book + ngram + model-mode status
//! - `GET  /brain`                — the demo page
//!
//! Answer modes, shown honestly on the page:
//! - `ollama-gemma` — a local small model (Gemma/LFM/Qwen floor) answers from
//!   retrieved context; its verbatim quote must pass the grounding gate.
//! - `ngram-extractive` — no network at all: sentences from the corpus,
//!   scored by embedding cosine + the corpus's own n-gram table.

use super::*;
use crate::brain::{self, BrainBook, BrainReply};
use crate::web::SharedState;
use axum::{extract::{Json, State}, http::StatusCode};
use std::sync::Arc;

const DOC_POMA: &str = include_str!("../../assets/brain/poma-3.8b.md");
const DOC_D14: &str = include_str!("../../assets/brain/supplier-standard-d14.md");

fn bundled_docs() -> Vec<(String, String)> {
    vec![
        (
            "POMA-3.8B — Plant Operations Master Agreement".into(),
            DOC_POMA.into(),
        ),
        (
            "Supplier Standard D-14 — subcontractor binding".into(),
            DOC_D14.into(),
        ),
    ]
}

/// The local small-model floor: localhost Ollama only, so the demo stays
/// offline by construction. Remote cascade providers are deliberately NOT
/// used here — a sales demo must not depend on (or leak to) the internet.
fn floor_provider() -> Option<crate::ai::provider::Provider> {
    let url = std::env::var("OLLAMA_FLOOR_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "http://localhost:11434/v1".into());
    let model = std::env::var("OLLAMA_FLOOR_MODEL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "llama3.1".into());
    Some(crate::ai::provider::Provider::new(
        crate::ai::provider::ProviderConfig {
            name: "ollama_floor".into(),
            base_url: url,
            model,
            api_key: String::new(),
            supports_vision: false,
            supports_tools: false,
            max_tokens: 512,
            timeout_secs: 60.0,
        },
    ))
}

#[derive(serde::Deserialize)]
pub struct BrainAskRequest {
    pub question: String,
}

#[derive(serde::Deserialize)]
pub struct BrainSeedRequest {
    /// Optional custom corpus; defaults to the bundled POMA vs D-14 pair.
    #[serde(default)]
    pub docs: Option<Vec<BrainSeedDoc>>,
}

#[derive(serde::Deserialize)]
pub struct BrainSeedDoc {
    pub name: String,
    pub text: String,
}

pub async fn seed_handler(
    State(state): State<SharedState>,
    Json(req): Json<BrainSeedRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let shared = state.shared.clone();
    let docs: Vec<(String, String)> = match req.docs {
        Some(list) if !list.is_empty() => {
            list.into_iter().map(|d| (d.name, d.text)).collect()
        }
        _ => bundled_docs(),
    };
    let built = tokio::task::spawn_blocking(move || {
        BrainBook::build(&docs, shared.embedder.as_ref(), shared.embedder_kind)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let docinfo = built.docs.clone();
    let chunks = built.chunks.len();
    let ngram = built.table.size();
    brain::store_book(built);
    Ok(Json(serde_json::json!({
        "ok": true,
        "docs": docinfo,
        "chunks": chunks,
        "ngram_cells": ngram,
    })))
}

pub async fn ask_handler(
    State(state): State<SharedState>,
    Json(req): Json<BrainAskRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let book = brain::current_book().ok_or(StatusCode::PRECONDITION_REQUIRED)?;
    let question = req.question.trim().to_string();
    if question.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let shared = state.shared.clone();
    let q = question.clone();
    let q_emb = tokio::task::spawn_blocking(move || shared.embedder.embed(&q))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let top = book.retrieve(&q_emb, 4);
    let top_names: Vec<String> = top.iter().map(|c| c.doc_name.clone()).collect();

    // Attempt 1 — the local small model, grounded. Its answer counts only if
    // the verbatim quote it cites survives the token-match gate.
    if let Some(provider) = floor_provider() {
        let context: String = top
            .iter()
            .map(|c| format!("[{} — {}]\n{}", c.doc_name, c.heading, c.text))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");
        let sys = "You answer strictly from the provided context excerpts of two industrial \
documents. End your reply with a last line: QUOTE: <a verbatim sentence copied \
word-for-word from the context that supports your answer>. If the context does \
not contain the answer, reply exactly: NOT IN DOCUMENTS.";
        let msgs = vec![
            crate::ai::provider::Message { role: "system".into(), content: crate::ai::provider::Content::Text(sys.into()) },
            crate::ai::provider::Message { role: "user".into(), content: crate::ai::provider::Content::Text(format!("Context:\n{context}\n\nQuestion: {question}")) },
        ];
        let route = crate::ai::provider::RouteHint { task_type: "fast".into(), has_image: false };
        if let Ok((resp, _)) = provider.complete(&msgs, None, &route, Some(512)).await {
            let text = match &resp.content {
                crate::ai::provider::Content::Text(t) => t.clone(),
                _ => String::new(),
            };
            if !text.contains("NOT IN DOCUMENTS") {
                let (answer, quote) = split_quote(&text);
                let verdict = brain::ground_quote(&quote, &top);
                if verdict == crate::grounding::Verdict::Grounded {
                    return Ok(Json(serde_json::json!({
                        "mode": "ollama-small-model",
                        "model": provider.model(),
                        "question": question,
                        "answer": answer.trim(),
                        "quote": quote,
                        "grounding": "grounded",
                        "sources": top_names,
                        "extractive": book.ask_extractive(&question, &q_emb),
                    })));
                }
                // Ungrounded → fall through; the gate is not a rubber stamp.
            }
        }
    }

    // Attempt 2 — fully offline: ngram-table extractive answer.
    let reply: BrainReply = book.ask_extractive(&question, &q_emb);
    Ok(Json(serde_json::json!({
        "mode": "ngram-extractive",
        "model": book.embedder_kind,
        "question": question,
        "answer": reply.answers.first().map(|a| a.text.clone()).unwrap_or_default(),
        "answers": reply.answers,
        "sources": top_names,
        "citations": reply.citations,
        "grounding": "verbatim",
    })))
}

/// Split the model reply into answer body + the QUOTE: line.
fn split_quote(text: &str) -> (String, String) {
    let mut quote = String::new();
    let mut body: Vec<&str> = Vec::new();
    for line in text.lines() {
        let low = line.trim().to_lowercase();
        if low.starts_with("quote") {
            quote = line
                .split_once(':')
                .map(|(_, q)| q.trim().trim_matches(['"', '\'', '`']).to_string())
                .unwrap_or_default();
        } else {
            body.push(line);
        }
    }
    (body.join("\n").trim().to_string(), quote)
}

pub async fn faceoff_handler() -> Result<Json<serde_json::Value>, StatusCode> {
    let book = brain::current_book().ok_or(StatusCode::PRECONDITION_REQUIRED)?;
    let report = book.faceoff();
    Ok(Json(serde_json::to_value(report).unwrap_or_default()))
}

pub async fn status_handler() -> Json<serde_json::Value> {
    let book = brain::current_book();
    Json(serde_json::json!({
        "seeded": book.is_some(),
        "docs": book.as_ref().map(|b| b.docs.clone()).unwrap_or_default(),
        "chunks": book.as_ref().map(|b| b.chunks.len()).unwrap_or(0),
        "ngram_cells": book.as_ref().map(|b| b.table.size()).unwrap_or(0),
        "ngram_words": book.as_ref().map(|b| b.table.words()).unwrap_or(0),
        "embedder": book.as_ref().map(|b| b.embedder_kind.clone()).unwrap_or_default(),
        "llm_floor_configured": floor_provider().is_some(),
        "llm_floor_model": std::env::var("OLLAMA_FLOOR_MODEL").unwrap_or_else(|_| "llama3.1".into()),
    }))
}

/// GET /brain — the offline brain-multiplier demo page.
pub async fn brain_ui_handler() -> Html<&'static str> {
    Html(include_str!("../bin/ui/brain.html"))
}
