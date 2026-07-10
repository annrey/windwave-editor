//! API route handlers for the agent-core HTTP API.

use super::{
    AgentRunRequest, ApiResponse, AppState, HealthResponse, SkillRunRequest, ToolExecuteRequest,
};
use axum::{
    extract::{Path, Query, State},
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use serde::Deserialize;
use std::convert::Infallible;
use tokio_stream::wrappers::ReceiverStream;

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

// ---------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------

pub async fn list_tools() -> Json<ApiResponse<serde_json::Value>> {
    // Return a simple placeholder — full integration requires ToolRegistry ref
    ApiResponse::success(serde_json::json!({
        "message": "Tool registry available via direct API; use POST /api/v1/tools/{name}/execute"
    }))
}

pub async fn execute_tool(
    Path(tool_name): Path<String>,
    Json(req): Json<ToolExecuteRequest>,
) -> Json<ApiResponse<serde_json::Value>> {
    // Placeholder — full integration requires ToolRegistry in AppState
    ApiResponse::success(serde_json::json!({
        "tool": tool_name,
        "received_params": req.parameters,
        "note": "Full tool execution available when ToolRegistry is added to AppState"
    }))
}

// ---------------------------------------------------------------------------
// Agent
// ---------------------------------------------------------------------------

pub async fn run_agent(Json(req): Json<AgentRunRequest>) -> Json<ApiResponse<serde_json::Value>> {
    let task_id = req.task_id.unwrap_or_else(|| uuid_v4());
    ApiResponse::success(serde_json::json!({
        "task_id": task_id,
        "instruction": req.instruction,
        "status": "accepted",
        "note": "Full agent dispatch available when AgentRegistry is added to AppState"
    }))
}

#[derive(Deserialize)]
pub struct IdParam {
    id: String,
}

pub async fn agent_status(
    Path(IdParam { id }): Path<IdParam>,
) -> Json<ApiResponse<serde_json::Value>> {
    ApiResponse::success(serde_json::json!({
        "task_id": id,
        "status": "pending",
        "note": "Full status tracking available when AgentRegistry is added to AppState"
    }))
}

// ---------------------------------------------------------------------------
// Skills
// ---------------------------------------------------------------------------

pub async fn list_skills() -> Json<ApiResponse<serde_json::Value>> {
    ApiResponse::success(serde_json::json!({
        "skills": [],
        "note": "Available when SkillRegistry is added to AppState"
    }))
}

pub async fn run_skill(
    Path(skill_name): Path<String>,
    Json(req): Json<SkillRunRequest>,
) -> Json<ApiResponse<serde_json::Value>> {
    ApiResponse::success(serde_json::json!({
        "skill": skill_name,
        "inputs": req.inputs,
        "status": "accepted",
        "note": "Full skill execution available when SkillRegistry is added to AppState"
    }))
}

// ---------------------------------------------------------------------------
// Livelog (SSE)
// ---------------------------------------------------------------------------

pub async fn livelog_stream(
    State(state): State<AppState>,
    Query(params): Query<LivelogQueryParams>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let since = params.since.unwrap_or(0);
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);

    let log = state.livelog.clone();
    tokio::spawn(async move {
        // Send historical entries
        let historical: Vec<String> = {
            let mut guard = log.lock().unwrap();
            guard
                .poll_since(since)
                .iter()
                .map(|e| serde_json::to_string(e).unwrap_or_default())
                .collect()
        };
        for json in historical {
            let _ = tx.send(Ok(Event::default().data(json))).await;
        }

        // Stream new entries via callback
        let tx2 = tx.clone();
        {
            let mut guard = log.lock().unwrap();
            guard.subscribe_stream(Box::new(move |entry| {
                let json = serde_json::to_string(entry).unwrap_or_default();
                let _ = tx2.blocking_send(Ok(Event::default().data(json)));
                true
            }));
        }
    });

    let stream = ReceiverStream::new(rx);
    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[derive(Deserialize)]
pub struct LivelogQueryParams {
    pub since: Option<u64>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("task_{ts:016x}")
}
