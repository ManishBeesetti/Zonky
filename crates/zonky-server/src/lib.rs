pub mod routes;
pub mod sse;
pub mod state;

use std::sync::Arc;

use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use state::AppState;
use zonky_core::ModelManager;

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
