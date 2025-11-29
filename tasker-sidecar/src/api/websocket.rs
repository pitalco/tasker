use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::state::{AppState, WsEvent};

#[derive(Debug, Deserialize)]
struct WsIncoming {
    #[serde(rename = "type")]
    msg_type: String,
}

#[derive(Debug, Serialize)]
struct WsOutgoing {
    #[serde(rename = "type")]
    msg_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    step: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(client_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    tracing::info!("WebSocket connection request from client: {}", client_id);
    ws.on_upgrade(move |socket| handle_socket(socket, client_id, state))
}

async fn handle_socket(socket: WebSocket, client_id: String, state: Arc<AppState>) {
    tracing::info!("WebSocket connected: {}", client_id);

    let (mut sender, mut receiver) = socket.split();

    // Subscribe to broadcast events
    let mut rx = state.subscribe();

    // Task to forward broadcast events to this client
    let send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let msg = match event {
                WsEvent::RecordingStep { session_id, step } => WsOutgoing {
                    msg_type: "recording_step".to_string(),
                    session_id: Some(session_id),
                    step: Some(serde_json::to_value(&step).unwrap_or_default()),
                    result: None,
                    session: None,
                    error: None,
                },
                WsEvent::ReplayStep { session_id, result } => WsOutgoing {
                    msg_type: "replay_step".to_string(),
                    session_id: Some(session_id),
                    step: None,
                    result: Some(serde_json::to_value(&result).unwrap_or_default()),
                    session: None,
                    error: None,
                },
                WsEvent::ReplayComplete { session_id, session } => WsOutgoing {
                    msg_type: "replay_complete".to_string(),
                    session_id: Some(session_id),
                    step: None,
                    result: None,
                    session: Some(serde_json::to_value(&session).unwrap_or_default()),
                    error: None,
                },
                WsEvent::Error { session_id, error } => WsOutgoing {
                    msg_type: "error".to_string(),
                    session_id: Some(session_id),
                    step: None,
                    result: None,
                    session: None,
                    error: Some(error),
                },
                WsEvent::Pong => WsOutgoing {
                    msg_type: "pong".to_string(),
                    session_id: None,
                    step: None,
                    result: None,
                    session: None,
                    error: None,
                },
            };

            let json = match serde_json::to_string(&msg) {
                Ok(j) => j,
                Err(_) => continue,
            };

            if sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    // Task to handle incoming messages (ping/pong)
    let state_clone = state.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(incoming) = serde_json::from_str::<WsIncoming>(&text) {
                    if incoming.msg_type == "ping" {
                        state_clone.broadcast(WsEvent::Pong);
                    }
                }
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    tracing::info!("WebSocket disconnected: {}", client_id);
}
