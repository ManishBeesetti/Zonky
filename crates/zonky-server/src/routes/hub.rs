use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use tracing::{error, info};

use crate::state::AppState;
use zonky_core::types::{BackendChoice, DeviceChoice};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/hub/search", get(search_hub))
        .route("/api/models/load", post(load_model))
        .route("/api/models/unload", post(unload_model))
        .route("/api/system/info", get(system_info))
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    20
}

async fn search_hub(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> Response {
    match state.manager.hub().search_models(&query.q, query.limit).await {
        Ok(models) => Json(serde_json::json!({
            "models": models,
            "count": models.len(),
        }))
        .into_response(),
        Err(e) => {
            error!(error = %e, "Hub search failed");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct LoadModelRequest {
    model_id: String,
    #[serde(default)]
    backend: BackendChoice,
    #[serde(default)]
    device: DeviceChoice,
}

async fn load_model(
    State(state): State<Arc<AppState>>,
    Json(request): Json<LoadModelRequest>,
) -> Response {
    info!(model_id = %request.model_id, "Loading model via API");

    match state
        .manager
        .load_model(&request.model_id, request.backend, request.device)
        .await
    {
        Ok(()) => Json(serde_json::json!({
            "status": "loaded",
            "model_id": request.model_id,
        }))
        .into_response(),
        Err(e) => {
            error!(error = %e, "Failed to load model");
            (
                axum::http::StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
struct UnloadModelRequest {
    model_id: String,
}

async fn unload_model(
    State(state): State<Arc<AppState>>,
    Json(request): Json<UnloadModelRequest>,
) -> Response {
    info!(model_id = %request.model_id, "Unloading model via API");

    match state.manager.unload_model(&request.model_id).await {
        Ok(()) => Json(serde_json::json!({
            "status": "unloaded",
            "model_id": request.model_id,
        }))
        .into_response(),
        Err(e) => {
            error!(error = %e, "Failed to unload model");
            (
                axum::http::StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

async fn system_info(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let loaded_models = state.manager.list_loaded_models().await;
    let devices = state.manager.devices();

    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "loaded_models": loaded_models,
        "devices": devices.iter().map(|d| serde_json::json!({
            "name": d.device_name(),
            "is_gpu": d.is_gpu(),
            "vram_free": d.vram_free(),
        })).collect::<Vec<_>>(),
    }))
}
