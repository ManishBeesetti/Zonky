use std::sync::Arc;

use axum::extract::State;
use axum::response::Json;
use axum::routing::get;
use axum::Router;

use crate::state::AppState;
use zonky_core::types::ModelListResponse;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/v1/models", get(list_models))
}

async fn list_models(State(state): State<Arc<AppState>>) -> Json<ModelListResponse> {
    let models = match state.manager.list_all_models().await {
        Ok(m) => m,
        Err(_) => state.manager.list_loaded_models().await,
    };

    Json(ModelListResponse {
        object: "list".to_string(),
        data: models,
    })
}
