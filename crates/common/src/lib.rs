// ================
// common/src/lib.rs
// ================
//! Common types and structures
//! used for communication between the `OpenLifter` client and server.
//! This module defines the WebSocket protocol messages and supporting types.

use serde::{Deserialize, Serialize};

/// Sequence number type for ordering updates
pub type Seq = u64;

/// Messages sent from client to server
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "msgType")]
pub enum ClientToServer {
    /// Create a new meet
    /// # Fields
    /// * `this_location_name` - Name of the creating location
    /// * `password` - Meet password (min 10 chars)
    /// * `endpoints` - List of endpoints with their conflict resolution priorities
    CreateMeet {
        this_location_name: String,
        password: String,
        endpoints: Vec<EndpointPriority>,
    },
    /// Join an existing meet
    /// # Fields
    /// * `meet_id` - ID of the meet to join
    /// * `password` - Meet password
    /// * `location_name` - Name of the joining location
    JoinMeet {
        meet_id: String,
        password: String,
        location_name: String,
    },
    /// Initialize updates from a client
    /// # Fields
    /// * `session_token` - Client's session token
    /// * `updates` - List of updates to apply
    UpdateInit {
        session_token: String,
        updates: Vec<Update>,
    },
    /// Request updates since a specific sequence number
    /// # Fields
    /// * `session_token` - Client's session token
    /// * `last_server_seq` - Last server sequence number seen by client
    ClientPull {
        session_token: String,
        last_server_seq: Seq,
    },
    /// Publish meet results to OPL
    /// # Fields
    /// * `session_token` - Client's session token
    /// * `return_email` - Email to send results to
    /// * `opl_csv` - CSV data in OPL format
    PublishMeet {
        session_token: String,
        return_email: String,
        opl_csv: String,
    },
}

/// Endpoint priority for conflict resolution
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EndpointPriority {
    /// Name of the endpoint
    pub location_name: String,
    /// Priority level (higher number = higher priority)
    pub priority: u8,
}

/// A single state update
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Update {
    /// Key path of the update (e.g., "lifter.1.name")
    pub update_key: String,
    /// New value for the key
    pub update_value: serde_json::Value,
    /// Local sequence number assigned by client
    pub local_seq_num: Seq,
    /// Last server sequence number seen by client
    pub after_server_seq_num: Seq,
}

/// Update with server-assigned sequence number
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateWithServerSeq {
    /// The original update
    #[serde(flatten)]
    pub update: Update,
    /// Server-assigned sequence number
    #[serde(rename = "serverSeqNum")]
    pub server_seq_num: Seq,
    /// ID of the client that created this update
    #[serde(rename = "sourceClientId", default)]
    pub source_client_id: String,
    /// Priority of the client that created this update
    #[serde(rename = "sourceClientPriority", default)]
    pub source_client_priority: u8,
}

/// Messages sent from server to client
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "msgType")]
pub enum ServerToClient {
    /// Response to successful meet creation
    MeetCreated {
        /// Generated meet ID
        meet_id: String,
        /// Session token for the creating client
        session_token: String,
    },
    /// Response to successful meet join
    MeetJoined {
        /// Session token for the joining client
        session_token: String,
    },
    /// Response to failed meet join
    JoinRejected {
        /// Reason for rejection
        reason: String,
    },
    /// Acknowledgment of updates
    UpdateAck {
        /// List of (`local_seq`, `server_seq`) pairs
        update_acks: Vec<(Seq, Seq)>,
    },
    /// Rejection of updates
    UpdateRejected {
        /// List of rejected updates with reasons
        updates_rejected: Vec<(Seq, String)>,
    },
    /// Relay of updates to other clients
    UpdateRelay {
        /// List of updates to apply
        updates_relayed: Vec<UpdateWithServerSeq>,
    },
    /// Response to client pull request
    ServerPull {
        /// Current server sequence number
        last_server_seq: Seq,
        /// Updates since client's last seen sequence
        updates_relayed: Vec<UpdateWithServerSeq>,
    },
    /// Acknowledgment of meet publication
    PublishAck,
    /// Error response for malformed messages
    MalformedMessage {
        /// Error description
        err_msg: String,
    },
    /// Error response for unknown message types
    UnknownMessageType {
        /// The unknown message type
        msg_type: String,
    },
    /// Error response for invalid sessions
    InvalidSession {
        /// The invalid session token
        session_token: String,
    },
}

/// Meet information stored on the server
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MeetInfo {
    /// Hashed meet password
    pub password_hash: String,
    /// List of endpoints with priorities
    pub endpoints: Vec<EndpointPriority>,
}
