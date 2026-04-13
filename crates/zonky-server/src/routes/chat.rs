use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::post;
use axum::Router;
use tracing::{error, info};

use crate::state::AppState;
use zonky_core::types::GenerationRequest;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/v1/chat/completions", post(chat_completions))
}

async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(request): Json<GenerationRequest>,
) -> Response {
    let model_id = request.model.clone();

    info!(model = %model_id, stream = request.stream, "Chat completion request");

    // Check if model is loaded
    let loaded = state.manager.list_loaded_models().await;
    if !loaded.iter().any(|m| m.id == model_id) {
        let error_response = serde_json::json!({
            "error": {
                "message": format!("Model '{}' is not loaded. Use POST /api/models/load to load it first.", model_id),
                "type": "invalid_request_error",
                "code": "model_not_found"
            }
        });
        return (axum::http::StatusCode::NOT_FOUND, Json(error_response)).into_response();
    }

    if request.stream {
        // Streaming response via SSE
        match state.manager.generate_stream(&model_id, &request).await {
            Ok(mut rx) => {
                let stream = async_stream::stream! {
                    while let Some(chunk_result) = rx.recv().await {
                        match chunk_result {
                            Ok(chunk) => {
                                let json_value = serde_json::to_value(&chunk).unwrap();
                                yield Ok::<_, std::convert::Infallible>(
                                    Event::default().data(serde_json::to_string(&json_value).unwrap())
                                );
                            }
                            Err(e) => {
                                error!(error = %e, "Stream generation error");
                                break;
                            }
                        }
                    }
                    // Send [DONE] marker
                    yield Ok(Event::default().data("[DONE]"));
                };

                Sse::new(stream)
                    .keep_alive(KeepAlive::default())
                    .into_response()
            }
            Err(e) => {
                error!(error = %e, "Failed to start stream generation");
                let error_response = serde_json::json!({
                    "error": {
                        "message": e.to_string(),
                        "type": "server_error",
                        "code": "generation_failed"
                    }
                });
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)).into_response()
            }
        }
    } else {
        // Non-streaming response
        match state.manager.generate(&model_id, &request).await {
            Ok(response) => Json(response).into_response(),
            Err(e) => {
                error!(error = %e, "Generation failed");
                let error_response = serde_json::json!({
                    "error": {
                        "message": e.to_string(),
                        "type": "server_error",
                        "code": "generation_failed"
                    }
                });
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)).into_response()
            }
        }
    }
}
