use axum::{
    Router,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    routing::get,
    response::IntoResponse,
};
use backend_lib::{
    AppState, 
    config::Settings, 
    storage::FlatFileStorage,
    messages::{ClientMessage, ServerMessage},
};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use serde_json;
use std::sync::Arc;

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState<FlatFileStorage>>) {
    println!("New WebSocket connection established");
    
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(text) = msg {
            println!("Received message: {}", text);
            
            match serde_json::from_str::<ClientMessage>(&text) {
                Ok(client_msg) => {
                    println!("Successfully parsed message: {:?}", client_msg);
                    let response = match client_msg {
                        ClientMessage::CreateMeet { meet_id, password } => {
                            println!("Creating meet: {}, password: {}", meet_id, password);
                            let session = state.auth.new_session(meet_id.clone(), "default".to_string(), 0).await;
                            ServerMessage::MeetCreated {
                                meet_id,
                                session_token: session,
                            }
                        }
                        ClientMessage::JoinMeet { meet_id, password } => {
                            println!("Joining meet: {}, password: {}", meet_id, password);
                            let session = state.auth.new_session(meet_id.clone(), "default".to_string(), 0).await;
                            ServerMessage::MeetJoined {
                                meet_id,
                                session_token: session,
                            }
                        }
                        ClientMessage::UpdateInit { meet_id, session_token, updates } => {
                            println!("Update init for meet: {}, session: {}", meet_id, session_token);
                            if state.auth.validate_session(&session_token).await {
                                ServerMessage::UpdateAck {
                                    meet_id,
                                    update_ids: updates.iter().map(|_| uuid::Uuid::new_v4().to_string()).collect(),
                                }
                            } else {
                                ServerMessage::Error {
                                    code: "INVALID_SESSION".to_string(),
                                    message: "Invalid or expired session token".to_string(),
                                }
                            }
                        }
                    };

                    let response_text = serde_json::to_string(&response).unwrap_or_else(|e| {
                        println!("Error serializing response: {}", e);
                        serde_json::to_string(&ServerMessage::Error {
                            code: "SERIALIZATION_ERROR".to_string(),
                            message: e.to_string(),
                        })
                        .unwrap_or_else(|_| String::from("{{\"error\": \"Failed to serialize response\"}}"))
                    });

                    println!("Sending response: {}", response_text);
                    if let Err(e) = socket.send(Message::Text(response_text.into())).await {
                        eprintln!("Failed to send message: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    println!("Error parsing message: {}", e);
                    let error_response = serde_json::to_string(&ServerMessage::Error {
                        code: "INVALID_MESSAGE".to_string(),
                        message: e.to_string(),
                    })
                    .unwrap_or_else(|_| String::from("{{\"error\": \"Failed to serialize error response\"}}"));

                    if let Err(e) = socket.send(Message::Text(error_response.into())).await {
                        eprintln!("Failed to send error message: {}", e);
                        break;
                    }
                }
            }
        }
    }

    println!("WebSocket connection closed");
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState<FlatFileStorage>>>,
) -> impl IntoResponse {
    println!("WebSocket upgrade request received");
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Initialize configuration
    let config = Settings::load()?;

    // Create storage
    let storage = FlatFileStorage::new("data")?;

    // Create application state
    let state = Arc::new(AppState::new(storage, config)?);

    // Create the router
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state);

    // Start the server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(&addr).await?;
    println!("listening on {addr}");

    axum::serve(listener, app).await?;

    Ok(())
} 