use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use tasker_sidecar::api::{routes::create_router, state::AppState};

#[tokio::main]
async fn main() {
    // Initialize tracing with filter to suppress noisy chromiumoxide logs
    let filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive("chromiumoxide::conn=off".parse().unwrap())
        .add_directive("chromiumoxide::handler=off".parse().unwrap());

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    // Load environment
    dotenvy::dotenv().ok();

    // Create application state
    let state = Arc::new(AppState::new());

    // Build router
    let app = create_router(Arc::clone(&state));

    // Start server
    let addr = SocketAddr::from(([127, 0, 0, 1], 8765));
    tracing::info!("Tasker Sidecar starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    // Run server with graceful shutdown on Ctrl+C
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(state))
        .await
        .unwrap();
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM) and perform graceful shutdown
async fn shutdown_signal(state: Arc<AppState>) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, cancelling active runs...");
    state.shutdown().await;
    tracing::info!("Shutdown complete");
}
