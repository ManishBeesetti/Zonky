pub mod routes;
pub mod sse;
pub mod state;

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Json, Response};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use state::AppState;
use zonky_core::ModelManager;

async fn auth_middleware(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    // Health endpoint is always public
    if req.uri().path() == "/health" {
        return next.run(req).await;
    }

    let expected_key = {
        let dynamic_key = state.manager.get_config().await.server.api_key;
        dynamic_key
            .or_else(|| state.config.server.api_key.clone())
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
    };

    if let Some(expected) = expected_key {
        let authorized = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|token| token.trim() == expected)
            .unwrap_or(false);

        if !authorized {
            let error_response = serde_json::json!({
                "error": {
                    "message": "Invalid or missing Bearer token in Authorization header.",
                    "type": "authentication_error",
                    "code": "invalid_api_key"
                }
            });
            return (StatusCode::UNAUTHORIZED, Json(error_response)).into_response();
        }
    }

    next.run(req).await
}

/// Build the Axum application router
pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = if state.config.server.cors_enabled {
        CorsLayer::very_permissive()
    } else {
        CorsLayer::new()
    };

    Router::new()
        // OpenAI-compatible endpoints
        .merge(routes::chat::router())
        .merge(routes::completions::router())
        .merge(routes::models::router())
        // Zonky-specific endpoints
        .merge(routes::health::router())
        .merge(routes::hub::router())
        // Middleware
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

/// Start the server
pub async fn serve(manager: ModelManager, config: zonky_core::ZonkyConfig) -> anyhow::Result<()> {
    let addr = format!("{}:{}", config.server.host, config.server.port);

    let state = Arc::new(AppState::new(Arc::new(manager), config));
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(address = %addr, "Zonky server starting");

    println!("\n  [SERVER] Zonky server running at http://{addr}");
    println!("  [API] OpenAI-compatible API: http://{addr}/v1/chat/completions");
    println!("  [INFO] Model list: http://{addr}/v1/models");
    println!("  [HEALTH] Health: http://{addr}/health\n");

    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_state(api_key: Option<String>) -> Arc<AppState> {
        let mut config = zonky_core::ZonkyConfig::default();
        config.cache_dir =
            std::env::temp_dir().join(format!("zonky_server_test_{}", uuid::Uuid::new_v4()));
        config.server.api_key = api_key;
        let manager = Arc::new(ModelManager::new(config.clone()).unwrap());
        Arc::new(AppState::new(manager, config))
    }

    #[tokio::test]
    async fn health_and_models_routes_respond() {
        let state = test_state(None);
        let app = build_router(state);

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/v1/models")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn completions_and_chat_return_404_when_model_not_loaded() {
        let state = test_state(None);
        let app = build_router(state);

        let chat_req = Request::builder()
            .method("POST")
            .uri("/v1/chat/completions")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"model":"unloaded-model","messages":[{"role":"user","content":"hi"}]}"#,
            ))
            .unwrap();
        let chat_res = app.clone().oneshot(chat_req).await.unwrap();
        assert_eq!(chat_res.status(), StatusCode::NOT_FOUND);

        let comp_req = Request::builder()
            .method("POST")
            .uri("/v1/completions")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"model":"unloaded-model","prompt":"hello"}"#))
            .unwrap();
        let comp_res = app.oneshot(comp_req).await.unwrap();
        assert_eq!(comp_res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn api_key_auth_enforces_bearer_token_except_health() {
        let state = test_state(Some("secret-token-123".to_string()));
        let app = build_router(state);

        // /health is public even when api_key is configured
        let health_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health_res.status(), StatusCode::OK);

        // Missing Authorization header -> 401
        let unauth_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/models")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauth_res.status(), StatusCode::UNAUTHORIZED);

        // Valid Bearer token -> 200
        let auth_res = app
            .oneshot(
                Request::builder()
                    .uri("/v1/models")
                    .header("Authorization", "Bearer secret-token-123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(auth_res.status(), StatusCode::OK);
    }
}
