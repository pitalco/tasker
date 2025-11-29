use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use tasker_sidecar::api::{routes::create_router, state::AppState};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Load environment
    dotenvy::dotenv().ok();

    // Create application state
    let state = Arc::new(AppState::new());

    // Build router
    let app = create_router(state);

    // Start server
    let addr = SocketAddr::from(([127, 0, 0, 1], 8765));
    tracing::info!("Tasker Sidecar starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
