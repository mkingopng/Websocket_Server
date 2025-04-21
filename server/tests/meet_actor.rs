// ==========================
// server/tests/meet_actor.rs
// ==========================
use tokio::sync::{mpsc, broadcast};
use serde_json::json;
use common::{Update, UpdateWithServerSeq};
use tokio;
use server::{
    meet_actor::{MeetActor, ActorMsg},
    FlatFileStorage,
};

#[tokio::test]
async fn test_meet_actor_update() {
    // Create a test storage
    let storage = FlatFileStorage::new("test-data").expect("Failed to initialize storage");
    
    // Create a broadcast channel for updates
    let (tx_relay, _) = broadcast::channel(32);
    
    // Create the actor
    let actor = MeetActor::new("test-meet".to_string(), storage, tx_relay);
    
    // Create a channel for sending messages to the actor
    let (cmd_tx, rx_cmd) = mpsc::unbounded_channel();
    
    // Spawn the actor
    let actor_handle = tokio::spawn(async move {
        actor.run(rx_cmd).await;
    });
    
    // Create a test update
    let update = Update {
        update_key: "test-key".to_string(),
        update_value: json!({ "value": "test" }),
        local_seq_num: 1,
        after_server_seq_num: 0,
    };
    
    // Create a channel for receiving the response
    let (resp_tx, mut resp_rx) = mpsc::unbounded_channel();
    
    // Send the update to the actor
    cmd_tx.send(ActorMsg::Update {
        client_id: "test-client".to_string(),
        priority: 1,
        updates: vec![update.clone()],
        resp_tx,
    }).unwrap();
    
    // Wait for the response
    let acks = resp_rx.recv().await.unwrap().unwrap();
    
    // Verify the response
    assert_eq!(acks.len(), 1);
    assert_eq!(acks[0].0, 1); // local_seq_num
    assert_eq!(acks[0].1, 1); // server_seq_num
    
    // Abort the actor
    actor_handle.abort();
}

#[tokio::test]
async fn test_meet_actor_pull() {
    // Create a test storage
    let storage = FlatFileStorage::new("test-data").expect("Failed to initialize storage");
    
    // Create a broadcast channel for updates
    let (tx_relay, _) = broadcast::channel(32);
    
    // Create the actor
    let actor = MeetActor::new("test-meet".to_string(), storage, tx_relay);
    
    // Create a channel for sending messages to the actor
    let (cmd_tx, rx_cmd) = mpsc::unbounded_channel();
    
    // Spawn the actor
    let actor_handle = tokio::spawn(async move {
        actor.run(rx_cmd).await;
    });
    
    // Create a test update
    let update = Update {
        update_key: "test-key".to_string(),
        update_value: json!({ "value": "test" }),
        local_seq_num: 1,
        after_server_seq_num: 0,
    };
    
    // Create a channel for receiving the response
    let (resp_tx, mut resp_rx) = mpsc::unbounded_channel();
    
    // Send the update to the actor
    cmd_tx.send(ActorMsg::Update {
        client_id: "test-client".to_string(),
        priority: 1,
        updates: vec![update.clone()],
        resp_tx,
    }).unwrap();
    
    // Wait for the response
    let _acks = resp_rx.recv().await.unwrap().unwrap();
    
    // Now pull the updates
    let (pull_resp_tx, mut pull_resp_rx) = mpsc::unbounded_channel();
    
    // Send the pull request to the actor
    cmd_tx.send(ActorMsg::Pull {
        since: 0,
        resp_tx: pull_resp_tx,
    }).unwrap();
    
    // Wait for the response
    let updates = pull_resp_rx.recv().await.unwrap().unwrap();
    
    // Verify the response
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].update.update_key, "test-key");
    assert_eq!(updates[0].server_seq_num, 1);
    
    // Abort the actor
    actor_handle.abort();
} 