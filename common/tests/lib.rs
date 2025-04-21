// common/tests/lib.rs
use serde_json::json;
use common::{Update, UpdateWithServerSeq, EndpointPriority};

#[test]
fn test_update_serialization() {
    let update = Update {
        update_key: "test-key".to_string(),
        update_value: json!({ "value": "test" }),
        local_seq_num: 1,
        after_server_seq_num: 0,
    };
    
    let json = serde_json::to_string(&update).unwrap();
    let deserialized: Update = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.update_key, "test-key");
    assert_eq!(deserialized.local_seq_num, 1);
    assert_eq!(deserialized.after_server_seq_num, 0);
}

#[test]
fn test_update_with_server_seq_serialization() {
    let update_with_seq = UpdateWithServerSeq {
        update: Update {
            update_key: "test-key".to_string(),
            update_value: json!({ "value": "test" }),
            local_seq_num: 1,
            after_server_seq_num: 0,
        },
        server_seq_num: 2,
    };
    
    let json = serde_json::to_string(&update_with_seq).unwrap();
    let deserialized: UpdateWithServerSeq = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.update.update_key, "test-key");
    assert_eq!(deserialized.update.local_seq_num, 1);
    assert_eq!(deserialized.update.after_server_seq_num, 0);
    assert_eq!(deserialized.server_seq_num, 2);
}

#[test]
fn test_endpoint_priority_serialization() {
    let endpoint = EndpointPriority {
        location_name: "Test Location".to_string(),
        priority: 1,
    };
    
    let json = serde_json::to_string(&endpoint).unwrap();
    let deserialized: EndpointPriority = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.location_name, "Test Location");
    assert_eq!(deserialized.priority, 1);
} 