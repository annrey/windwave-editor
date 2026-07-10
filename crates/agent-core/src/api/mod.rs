//! HTTP API module — REST endpoints for agent-core.
//!
//! Feature-gated behind `http-api`.
//! Provides 8 endpoints for agent management, tool execution, and live logging.

use axum::{
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

mod handlers;

// ---------------------------------------------------------------------------
// Shared app state
// ---------------------------------------------------------------------------

/// Application state shared across all API handlers.
#[derive(Clone)]
pub struct AppState {
    pub livelog: super::livelog::SharedLivelog,
}

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct AgentRunRequest {
    pub instruction: String,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub context: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Json<Self> {
        Json(Self {
            ok: true,
            data: Some(data),
            error: None,
        })
    }

    pub fn error(msg: impl Into<String>) -> Json<Self> {
        Json(Self {
            ok: false,
            data: None,
            error: Some(msg.into()),
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct ToolExecuteRequest {
    pub call_id: Option<String>,
    pub parameters: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct SkillRunRequest {
    pub inputs: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Build the full API router with shared state.
pub fn build_router(state: AppState) -> Router {
    use handlers::*;

    Router::new()
        // Health
        .route("/api/v1/health", get(health))
        // Tools
        .route("/api/v1/tools", get(list_tools))
        .route("/api/v1/tools/{tool_name}/execute", post(execute_tool))
        // Agent
        .route("/api/v1/agent/run", post(run_agent))
        .route("/api/v1/agent/status/{id}", get(agent_status))
        // Skills
        .route("/api/v1/skills", get(list_skills))
        .route("/api/v1/skills/{skill_name}/run", post(run_skill))
        // Livelog
        .route("/api/v1/livelog", get(livelog_stream))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(state)
}

/// Run the HTTP server on the given address.
pub async fn serve(router: Router, addr: &str) -> Result<(), std::io::Error> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    log::info!("Agent API server listening on http://{addr}");
    axum::serve(listener, router).await
}
