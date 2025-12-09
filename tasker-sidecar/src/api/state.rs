use dashmap::DashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, Mutex};

use crate::models::{RecordingSession, ReplaySession, StepResult, WorkflowStep};
use crate::recording::BrowserRecorder;
use crate::runs::{Run, RunRepository};

/// WebSocket event types broadcast to clients
#[derive(Debug, Clone)]
pub enum WsEvent {
    RecordingStep {
        session_id: String,
        step: WorkflowStep,
    },
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

/// Active recorder with its session
pub struct ActiveRecorder {
    pub recorder: Arc<BrowserRecorder>,
    pub session: RecordingSession,
    /// Optional client ID that started this recording
    pub client_id: Option<String>,
}

/// Shared application state
pub struct AppState {
    /// Active recording sessions: session_id -> recorder
    pub recordings: DashMap<String, ActiveRecorder>,

    /// Active runs: run_id -> run (for tracking running executions)
    pub active_runs: DashMap<String, Run>,

    /// Connected WebSocket clients: client_id -> client info
    pub connected_clients: DashMap<String, ConnectedClient>,

    /// Total connection count (for metrics)
    connection_count: AtomicUsize,

    /// Runs repository for persistence
    pub runs_repository: Option<RunRepository>,

    /// Broadcast channel for WebSocket events
    pub ws_broadcast: broadcast::Sender<WsEvent>,

    /// Global lock to prevent multiple concurrent recording starts
    /// This prevents race condition where two browser instances are launched
    pub recording_lock: Mutex<()>,
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
            recordings: DashMap::new(),
            active_runs: DashMap::new(),
            connected_clients: DashMap::new(),
            connection_count: AtomicUsize::new(0),
            runs_repository,
            ws_broadcast: tx,
            recording_lock: Mutex::new(()),
        }
    }

    pub fn broadcast(&self, event: WsEvent) {
        // Ignore send errors (no receivers)
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

    /// Unregister a WebSocket client connection and clean up any associated resources
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

        // Clean up any recordings associated with this client
        // (In case the client disconnected while recording)
        self.recordings.retain(|session_id, active_recorder| {
            let keep = active_recorder.client_id.as_deref() != Some(client_id);
            if !keep {
                tracing::info!("Cleaning up orphaned recording session: {}", session_id);
            }
            keep
        });
    }

    /// Get the number of active WebSocket connections
    pub fn active_connection_count(&self) -> usize {
        self.connected_clients.len()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
