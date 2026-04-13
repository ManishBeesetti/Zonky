use std::sync::Arc;

use axum::extract::State;
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
    info!(model = %model_id, "Completion request");

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
        stream: false,
        repetition_penalty: 1.1,
        stop: request.stop,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
        seed: None,
    };

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
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)).into_response()
        }
    }
}
