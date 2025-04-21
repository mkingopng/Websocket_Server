use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ClientMessage {
    CreateMeet {
        meet_id: String,
        password: String,
    },
    JoinMeet {
        meet_id: String,
        password: String,
    },
    UpdateInit {
        meet_id: String,
        session_token: String,
        updates: Vec<Update>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ServerMessage {
    MeetCreated {
        meet_id: String,
        session_token: String,
    },
    MeetJoined {
        meet_id: String,
        session_token: String,
    },
    UpdateAck {
        meet_id: String,
        update_ids: Vec<String>,
    },
    Error {
        code: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Update {
    pub location: String,
    pub value: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub token: String,
    pub meet_id: String,
}

impl Session {
    pub fn new(meet_id: String) -> Self {
        Self {
            token: Uuid::new_v4().to_string(),
            meet_id,
        }
    }
} 