use super::*;
use crate::web::SharedState;
use axum::{extract::{Json, Query, State, Request}, response::{IntoResponse, Response}, http::{StatusCode, HeaderMap}, routing::{get, post}};
use serde::{Deserialize, Serialize};

pub async fn mcp_sse_handler(
    state: axum::extract::State<Arc<AppCtx>>,
) -> Result<
    axum::response::sse::Sse<
        impl futures::Stream<Item = Result<axum::response::sse::Event, std::io::Error>>,
    >,
    axum::response::Response<String>,
> {
    use futures::channel::mpsc;
    use futures::{SinkExt, StreamExt};
    use rmcp::{model::*, ServiceExt};

    let session_id = format!("{:016x}", rand::random::<u128>());

    // Server → client channel: rmcp writes responses here
    let (server_tx, server_rx) = mpsc::channel::<ServerJsonRpcMessage>(64);
    // Client → server channel: POST handler writes here
    let (client_tx, client_rx) = mpsc::channel::<ClientJsonRpcMessage>(64);

    // Register client_tx so the POST handler can deliver messages
    state
        .mcp_sessions
        .lock()
        .await
        .insert(session_id.clone(), client_tx);

    // Create fresh PhysisMcpServer using the shared config
    let handler = crate::mcp::PhysisMcpServer::new(state.shared.config.clone());

    // Build the transport with error mapping (mpsc::SendError → io::Error)
    use std::io;
    let transport = (
        server_tx.sink_map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "mcp session closed")),
        client_rx,
    );

    // rmcp's serve_inner spawns an internal task — it returns RunningService
    // immediately while the message loop runs in the background. Do NOT clean
    // up the session here. The session (client_tx sender) must stay alive in
    // the map until the SSE connection ends; otherwise the message loop's
    // receiver gets None and terminates prematurely.
    let sid = session_id.clone();
    tokio::spawn(async move {
        if let Err(e) = handler.serve(transport).await {
            eprintln!("[mcp] session {sid} error: {e}");
        }
    });

    // Build the SSE response. When the client disconnects, server_rx ends
    // and we remove the session from the map (dropping client_tx), which
    // eventually causes the serve_inner loop to exit.
    let post_path = format!("/mcp/message?session_id={session_id}");
    let endpoint_event = axum::response::sse::Event::default()
        .event("endpoint")
        .data(post_path);

    let message_stream = server_rx.map(|msg| {
        serde_json::to_string(&msg)
            .map(|json| {
                axum::response::sse::Event::default()
                    .event("message")
                    .data(json)
            })
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    });

    let sid = session_id.clone();
    let mcp = state.mcp_sessions.clone();
    let cleanup = futures::stream::once(async move {
        mcp.lock().await.remove(&sid);
        // The SSE connection is closing; this event will not reach the client.
        Ok(axum::response::sse::Event::default()
            .event("close")
            .data(""))
    });

    let stream = futures::stream::once(futures::future::ok(endpoint_event))
        .chain(message_stream)
        .chain(cleanup);
    Ok(axum::response::sse::Sse::new(stream))
}

pub async fn mcp_message_handler(
    state: axum::extract::State<Arc<AppCtx>>,
    axum::extract::Query(query): axum::extract::Query<std::collections::HashMap<String, String>>,
    axum::Json(msg): axum::Json<serde_json::Value>,
) -> axum::http::StatusCode {
    use futures::SinkExt;
    use rmcp::model::ClientJsonRpcMessage;

    let session_id = match query.get("session_id").or_else(|| query.get("sessionId")) {
        Some(id) => id,
        None => return axum::http::StatusCode::BAD_REQUEST,
    };

    let msg: ClientJsonRpcMessage = match serde_json::from_value(msg) {
        Ok(m) => m,
        Err(_) => return axum::http::StatusCode::BAD_REQUEST,
    };

    let mut tx = match state.mcp_sessions.lock().await.get(session_id).cloned() {
        Some(tx) => tx,
        None => return axum::http::StatusCode::NOT_FOUND,
    };

    if tx.send(msg).await.is_err() {
        return axum::http::StatusCode::GONE;
    }
    axum::http::StatusCode::ACCEPTED
}

