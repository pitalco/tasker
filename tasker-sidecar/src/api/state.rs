use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use crate::agent::WorkflowAgent;
use crate::models::{RecordingSession, ReplaySession, StepResult, WorkflowStep};
use crate::recording::BrowserRecorder;
use crate::replay::WorkflowExecutor;
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

/// Active recorder with its session
pub struct ActiveRecorder {
    pub recorder: Arc<BrowserRecorder>,
    pub session: RecordingSession,
}

/// Active replay (either direct executor or AI agent)
pub enum ActiveReplay {
    Direct {
        executor: Arc<WorkflowExecutor>,
        session: ReplaySession,
    },
    Agent {
        agent: Arc<WorkflowAgent>,
        session: ReplaySession,
    },
}

impl ActiveReplay {
    pub fn session(&self) -> &ReplaySession {
        match self {
            ActiveReplay::Direct { session, .. } => session,
            ActiveReplay::Agent { session, .. } => session,
        }
    }

    pub fn session_mut(&mut self) -> &mut ReplaySession {
        match self {
            ActiveReplay::Direct { session, .. } => session,
            ActiveReplay::Agent { session, .. } => session,
        }
    }
}

/// Shared application state
pub struct AppState {
    /// Active recording sessions: session_id -> recorder
    pub recordings: DashMap<String, ActiveRecorder>,

    /// Active replay sessions: session_id -> replay
    pub replays: DashMap<String, ActiveReplay>,

    /// Active runs: run_id -> run
    pub active_runs: DashMap<String, Run>,

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
            replays: DashMap::new(),
            active_runs: DashMap::new(),
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
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
