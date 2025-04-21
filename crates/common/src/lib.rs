// common/src/lib.rs
use serde::{Deserialize, Serialize};

pub type Seq = u64;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "msgType")]
pub enum ClientToServer {
    CreateMeet {
        this_location_name: String,
        password: String,
        endpoints: Vec<EndpointPriority>,
    },
    JoinMeet {
        meet_id: String,
        password: String,
        location_name: String,
    },
    UpdateInit {
        session_token: String,
        updates: Vec<Update>,
    },
    ClientPull {
        session_token: String,
        last_server_seq: Seq,
    },
    PublishMeet {
        session_token: String,
        return_email: String,
        opl_csv: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EndpointPriority {
    pub location_name: String,
    pub priority: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Update {
    pub update_key: String,
    pub update_value: serde_json::Value,
    pub local_seq_num: Seq,
    pub after_server_seq_num: Seq,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateWithServerSeq {
    #[serde(flatten)]
    pub update: Update,
    #[serde(rename = "serverSeqNum")]
    pub server_seq_num: Seq,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "msgType")]
pub enum ServerToClient {
    MeetCreated { meet_id: String, session_token: String },
    MeetJoined { session_token: String },
    JoinRejected { reason: String },
    UpdateAck { update_acks: Vec<(Seq, Seq)> },
    UpdateRejected { updates_rejected: Vec<(Seq, String)> },
    UpdateRelay { updates_relayed: Vec<UpdateWithServerSeq> },
    ServerPull { last_server_seq: Seq, updates_relayed: Vec<UpdateWithServerSeq> },
    PublishAck,
    MalformedMessage { err_msg: String },
    UnknownMessageType { msg_type: String },
    InvalidSession { session_token: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MeetInfo {
    pub password_hash: String,
    pub endpoints: Vec<EndpointPriority>,
}
