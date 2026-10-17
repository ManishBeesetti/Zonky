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
        .route("/api/telemetry", get(get_telemetry).delete(clear_telemetry))
        .route("/api/generation/cancel", post(cancel_generation))
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
    match state
        .manager
        .hub()
        .search_models(&query.q, query.limit)
        .await
    {
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
    let used_vram = state.manager.total_vram_used().await;
    let devices = state.manager.devices();
    let (ram_total, ram_free) = zonky_core::gpu::system_memory_bytes();

    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "ram_total": ram_total,
        "ram_free": ram_free,
        "loaded_models": loaded_models,
        "devices": devices.iter().map(|d| {
            let vram_total = d.vram_total();
            let raw_free = d.vram_free();
            let vram_free = if d.is_gpu() && raw_free == vram_total {
                raw_free.saturating_sub(used_vram)
            } else {
                raw_free
            };
            serde_json::json!({
                "name": d.device_name(),
                "is_gpu": d.is_gpu(),
                "vram_total": vram_total,
                "vram_free": vram_free,
            })
        }).collect::<Vec<_>>(),
    }))
}

async fn get_telemetry(
    State(state): State<Arc<AppState>>,
) -> Json<zonky_core::types::TelemetrySummary> {
    Json(state.manager.get_telemetry().await)
}

async fn clear_telemetry(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    state.manager.clear_telemetry().await;
    Json(serde_json::json!({ "status": "cleared" }))
}

async fn cancel_generation(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let cancelled = state.manager.cancel_generation().await;
    Json(serde_json::json!({ "status": "cancelled", "cancelled_count": cancelled }))
}
