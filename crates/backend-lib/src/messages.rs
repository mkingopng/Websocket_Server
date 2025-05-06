// ================
// crates/backend-lib/src/messages.rs
// ================
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "msgType")]
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
    StateRecoveryResponse {
        meet_id: String,
        session_token: String,
        last_seq_num: u64,
        updates: Vec<Update>,
        priority: u8,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "msgType")]
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
    StateRecoveryRequest {
        meet_id: String,
        last_known_seq: u64,
    },
    StateRecovered {
        meet_id: String,
        new_seq_num: u64,
        updates_recovered: usize,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

// Add a test function to verify message serialization formats
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_client_message_serialization() {
        // Test CreateMeet message
        let create_meet = ClientMessage::CreateMeet {
            meet_id: "test-meet".to_string(),
            password: "TestPassword123!".to_string(),
            location_name: "Test Location".to_string(),
            priority: 10,
        };

        let json = serde_json::to_string_pretty(&create_meet).unwrap();
        println!("CreateMeet serialized: {}", json);

        // Verify the structure
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["msgType"], "CreateMeet");
        assert_eq!(parsed["meet_id"], "test-meet");
        assert_eq!(parsed["password"], "TestPassword123!");
        assert_eq!(parsed["location_name"], "Test Location");
        assert_eq!(parsed["priority"], 10);

        // Test parsing from JSON
        let parsed_msg: ClientMessage = serde_json::from_str(&json).unwrap();
        match parsed_msg {
            ClientMessage::CreateMeet {
                meet_id,
                password,
                location_name,
                priority,
            } => {
                assert_eq!(meet_id, "test-meet");
                assert_eq!(password, "TestPassword123!");
                assert_eq!(location_name, "Test Location");
                assert_eq!(priority, 10);
            },
            _ => panic!("Wrong variant"),
        }
    }
}
