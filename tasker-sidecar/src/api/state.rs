use dashmap::DashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::desktop::DesktopManager;
use crate::models::{ReplaySession, StepResult};
use crate::runs::{Run, RunRepository};

/// WebSocket event types broadcast to clients
#[derive(Debug, Clone)]
pub enum WsEvent {
    ReplayStep {
        session_id: String,
        result: StepResult,
    },
    ReplayComplete {
        session_id: String,
        session: ReplaySession,
    },
    Error {
        session_id: String,
        error: String,
    },
    Pong,
}

/// Connected WebSocket client info
#[derive(Debug)]
pub struct ConnectedClient {
    pub connected_at: Instant,
}

/// Shared application state
pub struct AppState {
    /// Active runs: run_id -> run (for tracking running executions)
    pub active_runs: DashMap<String, Run>,

    /// Cancel tokens for active executors: run_id -> token
    pub active_executors: DashMap<String, CancellationToken>,

    /// Desktop managers for active runs (for pause/resume)
    pub desktop_managers: DashMap<String, Arc<DesktopManager>>,

    /// Global shutdown token for graceful shutdown
    pub shutdown_token: CancellationToken,

    /// Connected WebSocket clients: client_id -> client info
    pub connected_clients: DashMap<String, ConnectedClient>,

    /// Total connection count (for metrics)
    connection_count: AtomicUsize,

    /// Runs repository for persistence
    pub runs_repository: Option<RunRepository>,

    /// Broadcast channel for WebSocket events
    pub ws_broadcast: broadcast::Sender<WsEvent>,
}

impl AppState {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);

        // Initialize runs repository
        let runs_repository = match RunRepository::new() {
            Ok(repo) => {
                tracing::info!("Runs repository initialized");
                Some(repo)
            }
            Err(e) => {
                tracing::error!("Failed to initialize runs repository: {}", e);
                None
            }
        };

        Self {
            active_runs: DashMap::new(),
            active_executors: DashMap::new(),
            desktop_managers: DashMap::new(),
            shutdown_token: CancellationToken::new(),
            connected_clients: DashMap::new(),
            connection_count: AtomicUsize::new(0),
            runs_repository,
            ws_broadcast: tx,
        }
    }

    pub fn broadcast(&self, event: WsEvent) {
        let _ = self.ws_broadcast.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WsEvent> {
        self.ws_broadcast.subscribe()
    }

    /// Register a WebSocket client connection
    pub fn client_connected(&self, client_id: &str) {
        self.connected_clients.insert(
            client_id.to_string(),
            ConnectedClient {
                connected_at: Instant::now(),
            },
        );
        let count = self.connection_count.fetch_add(1, Ordering::Relaxed) + 1;
        tracing::debug!(
            "Client {} connected (total: {}, active: {})",
            client_id,
            count,
            self.connected_clients.len()
        );
    }

    /// Unregister a WebSocket client connection
    pub fn client_disconnected(&self, client_id: &str) {
        if let Some((_, client)) = self.connected_clients.remove(client_id) {
            let duration = client.connected_at.elapsed();
            tracing::debug!(
                "Client {} disconnected after {:?} (active: {})",
                client_id,
                duration,
                self.connected_clients.len()
            );
        }
    }

    /// Get the number of active WebSocket connections
    pub fn active_connection_count(&self) -> usize {
        self.connected_clients.len()
    }

    /// Graceful shutdown - cancel all active runs
    pub async fn shutdown(&self) {
        let active_count = self.active_executors.len();
        if active_count > 0 {
            tracing::info!("Cancelling {} active run(s)...", active_count);
        }

        // Cancel global shutdown token
        self.shutdown_token.cancel();

        // Cancel all active executors
        for entry in self.active_executors.iter() {
            entry.value().cancel();
        }

        // Wait briefly for cleanup
        if active_count > 0 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
