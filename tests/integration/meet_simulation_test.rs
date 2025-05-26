use std::sync::Arc;
use tokio::sync::Mutex;
use serde_json::json;
use uuid::Uuid;
use backend_lib::messages::{Decision, Lifter};
use crate::test_utils::{TestMeet, create_test_lifters, attempt_to_update};

use backend_lib::{
    auth::service::AuthService,
    storage::Storage,
    meet_actor::MeetActor,
    messages::{
        ClientMessage, ServerMessage,
        MeetCreation, MeetJoin, UpdateInit,
        Attempt, NextAttempt,
    },
};

#[derive(Debug)]
struct MeetSimulation {
    meet_id: String,
    password: String,
    session_token: String,
    lifters: Vec<Lifter>,
    current_lift: String,
    current_attempt: u8,
    current_lifter_index: usize,
}

impl MeetSimulation {
    fn new() -> Self {
        Self {
            meet_id: Uuid::new_v4().to_string(),
            password: "TestPassword123!".to_string(),
            session_token: String::new(),
            lifters: Vec::new(),
            current_lift: "squat".to_string(),
            current_attempt: 1,
            current_lifter_index: 0,
        }
    }

    async fn create_meet(&mut self, auth_service: &AuthService) -> Result<(), String> {
        let creation = MeetCreation {
            meet_id: self.meet_id.clone(),
            password: self.password.clone(),
        };

        let response = auth_service.new_session(self.meet_id.clone(), "Test Location".to_string(), 1).await
            .map_err(|e| e.to_string())?;

        self.session_token = response;
        Ok(())
    }

    async fn register_lifters(&mut self, storage: &Arc<Mutex<dyn Storage>>) -> Result<(), String> {
        // Simulate CSV data with 10 lifters
        let csv_data = r#"
            name,weight_class,gender,age
            John Smith,93kg,M,25
            Jane Doe,84kg,F,28
            Mike Johnson,105kg,M,32
            Sarah Wilson,76kg,F,24
            David Brown,120kg,M,30
            Lisa Anderson,69kg,F,27
            Chris Taylor,93kg,M,29
            Emma White,84kg,F,26
            Tom Harris,105kg,M,31
            Rachel Green,76kg,F,23
        "#;

        // Parse CSV and create lifters
        let mut rdr = csv::Reader::from_reader(csv_data.as_bytes());
        for result in rdr.deserialize() {
            let lifter: Lifter = result.map_err(|e| e.to_string())?;
            self.lifters.push(lifter);
        }

        // Save lifters to storage as updates
        let mut storage = storage.lock().await;
        for lifter in &self.lifters {
            let update = openlifter_common::Update {
                update_key: format!("lifters.{}", lifter.name),
                update_value: serde_json::to_string(lifter).unwrap(),
                local_seq_num: 1,
                after_server_seq_num: 0,
            };
            storage.append_update(&self.meet_id, &serde_json::to_string(&update).unwrap()).await
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    async fn process_attempt(
        &mut self,
        meet_actor: &MeetActor,
        decision: Decision,
    ) -> Result<(), String> {
        let current_lifter = &self.lifters[self.current_lifter_index];
        
        // Record the attempt
        let attempt = Attempt {
            lifter_name: current_lifter.name.clone(),
            lift: self.current_lift.clone(),
            attempt_number: self.current_attempt,
            weight: 100.0, // Example weight
            decision: decision,
        };

        let update = openlifter_common::Update {
            update_key: format!("{}.{}.attempt{}", current_lifter.name, self.current_lift, self.current_attempt),
            update_value: serde_json::to_string(&attempt).unwrap(),
            local_seq_num: self.current_attempt as u64,
            after_server_seq_num: 0,
        };

        meet_actor.apply_updates("test", 1, vec![update]).await
            .map_err(|e| e.to_string())?;

        // If not the last attempt, submit next attempt
        if self.current_attempt < 3 {
            let next_attempt = NextAttempt {
                lifter_name: current_lifter.name.clone(),
                lift: self.current_lift.clone(),
                attempt_number: self.current_attempt + 1,
                weight: 105.0, // Example weight increase
            };

            let update = openlifter_common::Update {
                update_key: format!("{}.{}.next_attempt{}", current_lifter.name, self.current_lift, self.current_attempt + 1),
                update_value: serde_json::to_string(&next_attempt).unwrap(),
                local_seq_num: (self.current_attempt + 1) as u64,
                after_server_seq_num: 0,
            };

            meet_actor.apply_updates("test", 1, vec![update]).await
                .map_err(|e| e.to_string())?;
        }

        // Move to next lifter or next attempt
        self.current_lifter_index += 1;
        if self.current_lifter_index >= self.lifters.len() {
            self.current_lifter_index = 0;
            self.current_attempt += 1;
            
            // If we've completed all attempts for this lift, move to next lift
            if self.current_attempt > 3 {
                self.current_attempt = 1;
                self.current_lift = match self.current_lift.as_str() {
                    "squat" => "bench".to_string(),
                    "bench" => "deadlift".to_string(),
                    "deadlift" => "finished".to_string(),
                    _ => return Ok(()),
                };
            }
        }

        Ok(())
    }
}

#[tokio::test]
async fn test_meet_simulation() {
    let test_meet = TestMeet::new().await;
    let lifters = create_test_lifters();

    // Step 1: Create meet
    test_meet.create_meet("TestPassword123!").await
        .expect("Failed to create meet");

    // Step 2: Register lifters
    test_meet.register_lifters(&lifters).await
        .expect("Failed to register lifters");

    // Step 3: Simulate the meet flow
    for lift in &["squat", "bench", "deadlift"] {
        for attempt in 1..=3 {
            for lifter in &lifters {
                // Simulate referee decision (randomly choose good lift or no lift)
                let decision = if rand::random() {
                    Decision::GoodLift
                } else {
                    Decision::NoLift
                };
                let update = attempt_to_update(lifter, lift, attempt, 100.0 + (attempt as f32 * 5.0), decision);
                test_meet.apply_attempt_update(
                    &lifter.name,
                    5, // Example priority
                    update,
                ).await.expect("Failed to process attempt");
            }
        }
    }

    // Verify final state: 10 lifters * 3 lifts * 3 attempts = 90 updates
    let all_updates = test_meet.get_updates_since(0).await.expect("Failed to get updates");
    assert_eq!(all_updates.len(), lifters.len() * 9); // 3 lifts * 3 attempts
}

#[tokio::test]
async fn test_meet_simulation_with_errors() {
    // Similar to above but with error cases
    let auth_service = AuthService::new();
    let storage = Arc::new(Mutex::new(FlatFileStorage::new()));
    let meet_actor = MeetActor::new(storage.clone());

    let mut simulation = MeetSimulation::new();

    // Test invalid meet creation
    simulation.password = "weak".to_string();
    assert!(simulation.create_meet(&auth_service).await.is_err());

    // Test invalid lifter registration
    simulation.lifters.push(Lifter {
        name: "".to_string(),
        weight_class: "invalid".to_string(),
        gender: "invalid".to_string(),
        age: 0,
    });
    assert!(simulation.register_lifters(&storage).await.is_err());

    // Test invalid attempt processing
    simulation.current_lifter_index = 999;
    assert!(simulation.process_attempt(&meet_actor, Decision::GoodLift).await.is_err());
} 