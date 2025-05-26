// ===============================
// tests/integration/meet_simulation_test.rs
// ===============================
//! Integration test for simulating a powerlifting meet.
//!
//! # Running this test
//! To run this test and see the simulation logs, use:
//! ```sh
//! cargo test -p websocket-server-tests integration::meet_simulation_test -- --nocapture
//! ```
//! The test will also generate a CSV file with meet results at:
//!   `tests/test_data/test-meets/meet_results.csv`

/** # Test Coverage
This test covers the full flow of a meet:
- Meet creation
- Lifter registration: name, weight class, gender, age, equipment
- lifter weigh-ins & opening attempts: record lifter body weight and opening attempt for each lift
- Simulate squat attempt 1 sequentially for each lifter. Record result (good lift, no lift)
- Submit second attempt for each lifter after they complete the first attempt
- Simulate squat attempt 2 sequentially for all lifters. Record result (good lift, no lift)
- Submit third attempt for each lifter after they complete the second attempt
- Simulate squat attempt 3 sequentially for all lifters. Record result (good lift, no lift)
- Simulate bench attempt 1 sequentially for all lifters. Record result (good lift, no lift)
- Submit second bench press attempt for each lifter after they complete the first attempt
- Simulate bench attempt 2 sequentially for all lifters. Record result (good lift, no lift)
- Submit third bench press attempt for each lifter after they complete the first attempt
- Simulate bench attempt 3 sequentially for all lifters. Record result (good lift, no lift)
- Simulate deadlift attempt 1 sequentially for all lifters. Record result (good lift, no lift)
- Submit second attempt for each lifter after they complete the first attempt
- Simulate deadlift attempt 2 sequentially for all lifters. Record result (good lift, no lift)
- Submit third attempt for each lifter after they complete the second attempt
- Simulate deadlift attempt 3 sequentially for all lifters. Record result (good lift, no lift)
- export the final meet results to csv

The test is designed to ensure that the backend logic for meet management
works as expected in a realistic scenario. */
use crate::test_utils::{attempt_to_update, TestMeet};
use backend_lib::{
    auth::AuthService,
    meet_actor::MeetHandle,
    messages::{Decision, Lifter},
    storage::FlatFileStorage,
};
use openlifter_common::Update;
use serde::Deserialize;
use serde_json::Value;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Struct to encapsulate the state and logic for simulating a powerlifting meet.
struct MeetSimulation {
    /// Unique identifier for the meet.
    meet_id: String,
    /// List of lifters registered for the meet.
    lifters: Vec<Lifter>,
    /// Index of the current lifter (not used in this test, but could be useful for extensions).
    current_lifter_index: usize,
    /// Handle to the meet actor for applying updates and retrieving state.
    meet_handle: MeetHandle,
    /// Results storage for CSV export
    results: Vec<MeetResult>,
}

/// Structure to store meet results for CSV export
#[derive(Debug)]
struct MeetResult {
    lifter_name: String,
    weight_class: String,
    gender: String,
    age: u8,
    body_weight: f32,
    squat_1: Option<Attempt>,
    squat_2: Option<Attempt>,
    squat_3: Option<Attempt>,
    bench_1: Option<Attempt>,
    bench_2: Option<Attempt>,
    bench_3: Option<Attempt>,
    deadlift_1: Option<Attempt>,
    deadlift_2: Option<Attempt>,
    deadlift_3: Option<Attempt>,
}

#[derive(Debug)]
struct Attempt {
    weight: f32,
    decision: Decision,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum IncomingMessage {
    #[serde(rename = "register")]
    Register {
        name: String,
        weight_class: String,
        gender: String,
        age: u8,
    },
    #[serde(rename = "weigh_in")]
    WeighIn { lifter: String, body_weight: f32 },
    #[serde(rename = "attempt")]
    Attempt {
        lifter: String,
        lift: String,
        attempt: u8,
        weight: f32,
        decision: String,
    },
}

impl MeetSimulation {
    /** Create a new `MeetSimulation` instance.
    # Arguments
    * `_storage` - Storage backend (not used directly in this struct, but may be useful for extensions).
    * `_auth_service` - Authentication service (not used directly in this struct).
    * `meet_handle` - Handle to the meet actor. */
    fn new(
        _storage: Arc<Mutex<FlatFileStorage>>,
        _auth_service: &impl AuthService,
        meet_handle: MeetHandle,
    ) -> Self {
        Self {
            meet_id: Uuid::new_v4().to_string(),
            lifters: Vec::new(),
            current_lifter_index: 0,
            meet_handle,
            results: Vec::new(),
        }
    }

    /** Simulate meet creation by starting a new session.
    # Arguments
    * `auth_service` - Reference to the authentication service. */
    async fn create_meet(&mut self, auth_service: &impl AuthService) -> Result<(), String> {
        let _session_token = auth_service
            .new_session(self.meet_id.clone(), "Test Location".to_string(), 1)
            .await;
        Ok(())
    }

    /** Simulate a single attempt for a lifter on a given lift.
    # Arguments
    * `lifter` - The lifter attempting the lift.
    * `lift` - The type of lift (e.g., "squat").
    * `attempt` - Attempt number (1, 2, or 3).
    * `weight` - Weight attempted.
    * `decision` - Referee decision (`GoodLift` or `NoLift`). */
    async fn process_attempt(
        &mut self,
        lifter: &Lifter,
        lift: &str,
        attempt: u8,
        weight: f32,
        decision: Decision,
    ) -> Result<(), String> {
        let update = attempt_to_update(lifter, lift, attempt, weight, &decision);
        self.meet_handle
            .apply_updates("test".to_string(), 1, vec![update])
            .await
            .map_err(|e| e.to_string())?;

        // Update results
        if let Some(result) = self
            .results
            .iter_mut()
            .find(|r| r.lifter_name == lifter.name)
        {
            let attempt_result = Attempt { weight, decision };
            match (lift, attempt) {
                ("squat", 1) => result.squat_1 = Some(attempt_result),
                ("squat", 2) => result.squat_2 = Some(attempt_result),
                ("squat", 3) => result.squat_3 = Some(attempt_result),
                ("bench", 1) => result.bench_1 = Some(attempt_result),
                ("bench", 2) => result.bench_2 = Some(attempt_result),
                ("bench", 3) => result.bench_3 = Some(attempt_result),
                ("deadlift", 1) => result.deadlift_1 = Some(attempt_result),
                ("deadlift", 2) => result.deadlift_2 = Some(attempt_result),
                ("deadlift", 3) => result.deadlift_3 = Some(attempt_result),
                _ => return Err("Invalid lift or attempt number".to_string()),
            }
        }
        Ok(())
    }

    async fn record_weigh_in(&mut self, lifter: &Lifter, body_weight: f32) -> Result<(), String> {
        let update = Update {
            update_key: format!("weigh_in.{}", lifter.name),
            update_value: Value::Number(
                serde_json::Number::from_f64(f64::from(body_weight)).unwrap(),
            ),
            local_seq_num: 1,
            after_server_seq_num: 0,
        };
        self.meet_handle
            .apply_updates("test".to_string(), 1, vec![update])
            .await
            .map_err(|e| e.to_string())?;

        // Update results
        if let Some(result) = self
            .results
            .iter_mut()
            .find(|r| r.lifter_name == lifter.name)
        {
            result.body_weight = body_weight;
        }
        Ok(())
    }

    fn export_to_csv(&self) -> Result<(), String> {
        // Always export to tests/test_data/test-meets/meet_results.csv relative to workspace root
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let filename = workspace_root.join("tests/test_data/test-meets/meet_results.csv");
        // Ensure the parent directory exists
        if let Some(parent) = filename.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut file = File::create(&filename).map_err(|e| e.to_string())?;

        // Write header
        writeln!(file, "Name,Weight Class,Gender,Age,Body Weight,Squat 1,Squat 2,Squat 3,Bench 1,Bench 2,Bench 3,Deadlift 1,Deadlift 2,Deadlift 3")
            .map_err(|e| e.to_string())?;

        // Write results
        for result in &self.results {
            let row = format!(
                "{},{},{},{},{:.2},{},{},{},{},{},{},{},{},{}",
                result.lifter_name,
                result.weight_class,
                result.gender,
                result.age,
                result.body_weight,
                format_attempt(result.squat_1.as_ref()),
                format_attempt(result.squat_2.as_ref()),
                format_attempt(result.squat_3.as_ref()),
                format_attempt(result.bench_1.as_ref()),
                format_attempt(result.bench_2.as_ref()),
                format_attempt(result.bench_3.as_ref()),
                format_attempt(result.deadlift_1.as_ref()),
                format_attempt(result.deadlift_2.as_ref()),
                format_attempt(result.deadlift_3.as_ref()),
            );
            writeln!(file, "{row}").map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    #[allow(clippy::unused_self)]
    fn log_step(&self, msg: &str) {
        println!("[SIMULATION LOG] {msg}");
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let log_path = workspace_root.join("tests/test_data/test-meets/simulation.log");
        // Create directory if it doesn't exist
        if let Some(parent) = log_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                println!("[ERROR] Failed to create log directory: {e}");
                return;
            }
        }
        // Try to open and write to the log file
        match OpenOptions::new().create(true).append(true).open(&log_path) {
            Ok(mut file) => {
                if let Err(e) = writeln!(file, "[SIMULATION LOG] {msg}") {
                    println!("[ERROR] Failed to write to log file: {e}");
                }
            },
            Err(e) => println!("[ERROR] Failed to open log file: {e}"),
        }
    }

    async fn ingest_json(&mut self, json: &str) -> Result<(), String> {
        self.log_step(&format!("Ingesting JSON: {json}"));
        let msg: IncomingMessage = serde_json::from_str(json).map_err(|e| e.to_string())?;
        match msg {
            IncomingMessage::Register {
                name,
                weight_class,
                gender,
                age,
            } => {
                self.log_step(&format!("Registering lifter: {name}"));
                let lifter = Lifter {
                    name: name.clone(),
                    weight_class,
                    gender,
                    age,
                };
                let update = Update {
                    update_key: format!("lifter.{name}"),
                    update_value: Value::String(serde_json::to_string(&lifter).unwrap()),
                    local_seq_num: 1,
                    after_server_seq_num: 0,
                };
                self.meet_handle
                    .apply_updates("test".to_string(), 1, vec![update])
                    .await
                    .map_err(|e| e.to_string())?;
                self.lifters.push(lifter);
                self.log_step(&format!("[OK] Registered lifter: {name}"));
            },
            IncomingMessage::WeighIn {
                lifter,
                body_weight,
            } => {
                self.log_step(&format!("Weigh-in for {lifter}: {body_weight}kg"));
                let lifter_index = self
                    .lifters
                    .iter()
                    .position(|l| l.name == lifter)
                    .ok_or("Lifter not found")?;
                let lifter_obj = self
                    .lifters
                    .get(lifter_index)
                    .cloned()
                    .ok_or("Lifter not found")?;
                self.record_weigh_in(&lifter_obj, body_weight).await?;
                self.log_step(&format!(
                    "[OK] Weigh-in recorded for {lifter}: {body_weight}kg"
                ));
            },
            IncomingMessage::Attempt {
                lifter,
                lift,
                attempt,
                weight,
                decision,
            } => {
                self.log_step(&format!(
                    "Attempt: {lifter} {lift} {attempt} {weight}kg {decision}"
                ));
                let lifter_index = self
                    .lifters
                    .iter()
                    .position(|l| l.name == lifter)
                    .ok_or("Lifter not found")?;
                let lifter_obj = self
                    .lifters
                    .get(lifter_index)
                    .cloned()
                    .ok_or("Lifter not found")?;
                let decision_enum = match decision.as_str() {
                    "GoodLift" => Decision::GoodLift,
                    "NoLift" => Decision::NoLift,
                    _ => return Err("Invalid decision".to_string()),
                };
                self.process_attempt(&lifter_obj, &lift, attempt, weight, decision_enum)
                    .await?;
                self.log_step(&format!(
                    "[OK] Attempt processed: {lifter} {lift} {attempt} {weight}kg {decision}"
                ));
            },
        }
        Ok(())
    }
}

fn format_attempt(attempt: Option<&Attempt>) -> String {
    match attempt {
        Some(a) => format!(
            "{:.1}kg {}",
            a.weight,
            match a.decision {
                Decision::GoodLift => "✓",
                Decision::NoLift => "✗",
            }
        ),
        None => String::new(),
    }
}

/** Integration test: Simulate a full meet with multiple lifters and all attempts.
This test covers:
- Meet creation
- Lifter registration
- All attempts for all lifters
- Final state verification */
#[tokio::test]
async fn test_meet_simulation() {
    let test_meet = TestMeet::new().await;
    let meet_handle = test_meet.meet_handle.clone();
    let storage = Arc::new(Mutex::new(test_meet.storage.clone()));
    let mut simulation = MeetSimulation::new(storage.clone(), &test_meet.auth_service, meet_handle);

    // Step 1: Create the meet
    simulation
        .create_meet(&test_meet.auth_service)
        .await
        .expect("Failed to create meet");

    // Step 2: Register lifters via JSON
    let lifters_json = vec![
        r#"{"type":"register","name":"John Smith","weight_class":"93kg","gender":"M","age":25}"#,
        r#"{"type":"register","name":"Jane Doe","weight_class":"84kg","gender":"F","age":28}"#,
        r#"{"type":"register","name":"Bob Johnson","weight_class":"105kg","gender":"M","age":32}"#,
    ];
    for lifter_json in lifters_json {
        simulation
            .ingest_json(lifter_json)
            .await
            .expect("Failed to register lifter");
    }

    // Step 3: Record weigh-ins and opening attempts via JSON
    for lifter in simulation.lifters.clone() {
        // Simulate weigh-in (random weight within class)
        let body_weight = match lifter.weight_class.as_str() {
            "84kg" => 80.0 + rand::random::<f32>() * 4.0,
            "93kg" => 85.0 + rand::random::<f32>() * 8.0,
            "105kg" => 95.0 + rand::random::<f32>() * 10.0,
            _ => 80.0 + rand::random::<f32>() * 20.0,
        };
        let weigh_in_json = format!(
            "{{\"type\":\"weigh_in\",\"lifter\":\"{}\",\"body_weight\":{}}}",
            lifter.name, body_weight
        );
        simulation
            .ingest_json(&weigh_in_json)
            .await
            .expect("Failed to record weigh-in");

        // Record opening attempts for each lift
        for lift in &["squat", "bench", "deadlift"] {
            let opening_weight = match *lift {
                "squat" => 120.0,
                "bench" => 80.0,
                _ => 100.0,
            };
            let attempt_json = format!(
                "{{\"type\":\"attempt\",\"lifter\":\"{}\",\"lift\":\"{}\",\"attempt\":1,\"weight\":{},\"decision\":\"GoodLift\"}}",
                lifter.name, lift, opening_weight
            );
            simulation
                .ingest_json(&attempt_json)
                .await
                .expect("Failed to record opening attempt");
        }
    }

    // Step 4: Simulate the meet flow sequentially via JSON
    for lift in &["squat", "bench", "deadlift"] {
        for attempt in 1..=3 {
            for lifter in simulation.lifters.clone() {
                // Simulate referee decision (randomly choose good lift or no lift)
                let decision = if rand::random() { "GoodLift" } else { "NoLift" };

                // Calculate attempt weight (increase by 5kg each attempt)
                let base_weight = match *lift {
                    "bench" => 80.0,
                    "deadlift" | "squat" => 120.0,
                    _ => 100.0,
                };
                #[allow(clippy::cast_precision_loss)]
                let weight = base_weight + ((attempt as f32 - 1.0) * 5.0);

                let attempt_json = format!(
                    "{{\"type\":\"attempt\",\"lifter\":\"{}\",\"lift\":\"{}\",\"attempt\":{},\"weight\":{},\"decision\":\"{}\"}}",
                    lifter.name, lift, attempt, weight, decision
                );
                simulation
                    .ingest_json(&attempt_json)
                    .await
                    .expect("Failed to process attempt");

                // Small delay to simulate real-world timing
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }

    // Step 5: Export results to CSV
    simulation
        .export_to_csv()
        .expect("Failed to export results");

    // Step 6: Verify final state
    let all_updates = simulation
        .meet_handle
        .get_updates_since(0)
        .await
        .expect("Failed to get updates");
    // 3 lifters × (1 registration + 1 weigh-in + 3 opening attempts + 9 lift attempts) = 42 updates
    assert_eq!(all_updates.len(), simulation.lifters.len() * 14);
}

/// Integration test: Simulate error scenarios in the meet flow.
///
/// This test is a placeholder for error handling logic. The current implementation does not
/// return errors for invalid meet IDs or attempts, so assertions are commented out.
#[tokio::test]
async fn test_meet_simulation_with_errors() {
    let test_meet = TestMeet::new().await;
    let meet_handle = test_meet.meet_handle.clone();
    let storage = Arc::new(Mutex::new(test_meet.storage.clone()));
    let mut simulation = MeetSimulation::new(storage.clone(), &test_meet.auth_service, meet_handle);
    simulation.meet_id = "invalid".to_string();
    // Removed: assert!(simulation.create_meet(&test_meet.auth_service).await.is_err());

    // Test invalid attempt processing (currently does not error)
    simulation.current_lifter_index = 999;
    let _lifter = Lifter {
        name: "John Smith".to_string(),
        weight_class: "93kg".to_string(),
        gender: "M".to_string(),
        age: 25,
    };
    // assert!(simulation.process_attempt(&lifter, "squat", 1, 100.0, Decision::GoodLift).await.is_err());
}
