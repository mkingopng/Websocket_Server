use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ClientMessage {
    CreateMeet {
        meet_id: String,
        password: String,
        location_name: String,
        priority: u8,
    },
    JoinMeet {
        meet_id: String,
        password: String,
        location_name: String,
        priority: u8,
    },
    UpdateInit {
        meet_id: String,
        session_token: String,
        updates: Vec<Update>,
    },
    ClientPull {
        meet_id: String,
        session_token: String,
        last_server_seq: u64,
    },
    PublishMeet {
        meet_id: String,
        session_token: String,
        return_email: String,
        opl_csv: String,
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
    UpdateRelay {
        meet_id: String,
        updates: Vec<UpdateWithMetadata>,
    },
    JoinRejected {
        reason: String,
    },
    UpdateRejected {
        meet_id: String,
        updates_rejected: Vec<(String, String)>,
    },
    ServerPull {
        meet_id: String,
        last_server_seq: u64,
        updates_relayed: Vec<UpdateWithMetadata>,
    },
    PublishAck {
        meet_id: String,
    },
    MalformedMessage {
        err_msg: String,
    },
    UnknownMessageType {
        msg_type: String,
    },
    InvalidSession {
        session_token: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWithMetadata {
    pub update: Update,
    pub source_client: String,
    pub server_seq: u64,
    pub priority: u8,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub token: String,
    pub meet_id: String,
    pub location_name: String,
    pub priority: u8,
}

impl Session {
    pub fn new(meet_id: String, location_name: String, priority: u8) -> Self {
        Self {
            token: Uuid::new_v4().to_string(),
            meet_id,
            location_name,
            priority,
        }
    }
}

// Store client information with priority
#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub session_token: String,
    pub location_name: String,
    pub priority: u8,
}

// Store meet information
#[derive(Debug, Clone)]
pub struct MeetInfo {
    pub meet_id: String,
    pub password_hash: String,
    pub clients: Vec<ClientInfo>,
}
