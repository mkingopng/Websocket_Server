use std::sync::Arc;
use tokio::sync::broadcast;
use anyhow::Result;
use crate::AppState;
use crate::messages::{ClientMessage, ServerMessage};

pub struct WebSocketHandler<S> {
    state: Arc<AppState<S>>,
    tx: broadcast::Sender<ServerMessage>,
}

impl<S> WebSocketHandler<S> {
    pub fn new(state: Arc<AppState<S>>) -> Self {
        let (tx, _) = broadcast::channel(100);
        Self { state, tx }
    }

    pub async fn handle_message(&self, msg: ClientMessage) -> Result<()> {
        match msg {
            ClientMessage::CreateMeet { meet_id, password: _ } => {
                let session = self.state.auth.new_session(meet_id.clone(), "default".to_string(), 0).await;
                let response = ServerMessage::MeetCreated {
                    meet_id,
                    session_token: session,
                };
                self.tx.send(response)?;
            }
            ClientMessage::JoinMeet { meet_id, password: _ } => {
                let session = self.state.auth.new_session(meet_id.clone(), "default".to_string(), 0).await;
                let response = ServerMessage::MeetJoined {
                    meet_id,
                    session_token: session,
                };
                self.tx.send(response)?;
            }
            ClientMessage::UpdateInit { meet_id, session_token, updates } => {
                if self.state.auth.validate_session(&session_token).await {
                    let response = ServerMessage::UpdateAck {
                        meet_id,
                        update_ids: updates.iter().map(|_| uuid::Uuid::new_v4().to_string()).collect(),
                    };
                    self.tx.send(response)?;
                } else {
                    let response = ServerMessage::Error {
                        code: "INVALID_SESSION".to_string(),
                        message: "Invalid or expired session token".to_string(),
                    };
                    self.tx.send(response)?;
                }
            }
        }
        Ok(())
    }
} 