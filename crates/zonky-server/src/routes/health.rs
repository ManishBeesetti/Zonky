use std::sync::Arc;

use axum::extract::State;
use axum::response::Json;
use axum::routing::get;
use axum::Router;

use crate::state::AppState;
use zonky_core::gpu;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/health", get(health_check))
}

async fn health_check(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let loaded_models = state.manager.list_loaded_models().await;
    let devices = state.manager.devices();

    let gpu_info: Vec<serde_json::Value> = devices
        .iter()
        .map(|d| {
            serde_json::json!({
                "name": d.device_name(),
                "is_gpu": d.is_gpu(),
                "vram_free": d.vram_free(),
            })
        })
        .collect();

    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "loaded_models": loaded_models.len(),
        "models": loaded_models.iter().map(|m| &m.id).collect::<Vec<_>>(),
        "devices": gpu_info,
        "compute": gpu::device_summary(),
    }))
}
