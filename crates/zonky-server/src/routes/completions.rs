use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::post;
use axum::Router;
use tracing::{error, info};

use crate::state::AppState;
use zonky_core::types::CompletionRequest;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/v1/completions", post(completions))
}

async fn completions(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CompletionRequest>,
) -> Response {
    let model_id = request.model.clone();
    info!(model = %model_id, stream = request.stream, "Completion request");

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

    // Convert CompletionRequest to GenerationRequest
    let gen_request = zonky_core::types::GenerationRequest {
        model: request.model,
        messages: vec![zonky_core::types::Message {
            role: zonky_core::types::Role::User,
            content: request.prompt,
        }],
        temperature: request.temperature,
        top_p: request.top_p,
        top_k: None,
        max_tokens: request.max_tokens,
        stream: request.stream,
        repetition_penalty: request.repetition_penalty,
        stop: request.stop,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
        seed: None,
        cancel_token: None,
    };

    if request.stream {
        match state.manager.generate_stream(&model_id, &gen_request).await {
            Ok(mut rx) => {
                let model_name = model_id.clone();
                let stream = async_stream::stream! {
                    while let Some(chunk_result) = rx.recv().await {
                        match chunk_result {
                            Ok(chunk) => {
                                let payload = serde_json::json!({
                                    "id": chunk.id,
                                    "object": "text_completion",
                                    "created": chrono::Utc::now().timestamp(),
                                    "model": model_name,
                                    "choices": [{
                                        "text": chunk.delta_text(),
                                        "index": 0,
                                        "logprobs": null,
                                        "finish_reason": chunk.finish_reason()
                                    }]
                                });
                                yield Ok::<_, std::convert::Infallible>(
                                    Event::default().data(serde_json::to_string(&payload).unwrap())
                                );
                            }
                            Err(e) => {
                                error!(error = %e, "Stream completion error");
                                break;
                            }
                        }
                    }
                    yield Ok(Event::default().data("[DONE]"));
                };

                Sse::new(stream)
                    .keep_alive(KeepAlive::default())
                    .into_response()
            }
            Err(e) => {
                error!(error = %e, "Failed to start stream completion");
                let error_response = serde_json::json!({
                    "error": {
                        "message": e.to_string(),
                        "type": "server_error",
                        "code": "generation_failed"
                    }
                });
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(error_response),
                )
                    .into_response()
            }
        }
    } else {
        match state.manager.generate(&model_id, &gen_request).await {
            Ok(response) => {
                // Convert to legacy completion format
                let completion = serde_json::json!({
                    "id": response.id,
                    "object": "text_completion",
                    "created": response.created,
                    "model": response.model,
                    "choices": response.choices.iter().map(|c| {
                        serde_json::json!({
                            "text": c.message.content,
                            "index": c.index,
                            "logprobs": null,
                            "finish_reason": c.finish_reason
                        })
                    }).collect::<Vec<_>>(),
                    "usage": response.usage
                });
                Json(completion).into_response()
            }
            Err(e) => {
                error!(error = %e, "Completion failed");
                let error_response = serde_json::json!({
                    "error": {
                        "message": e.to_string(),
                        "type": "server_error",
                        "code": "generation_failed"
                    }
                });
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(error_response),
                )
                    .into_response()
            }
        }
    }
}
