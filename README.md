# OpenLifter WebSocket Server

A WebSocket server for real-time meet management in OpenLifter.

## Features
- Real-time meet updates via WebSocket
- Session-based authentication
- Configurable password requirements
- Metrics and monitoring
- Structured logging
- Health checks
- CORS support

- Multiple browsers running the same meet simultaneously
- Multiple updating browsers
- Multiple display-only browsers
- Browsers can make meets live or join already live meets
- Authentication via meet ID and password
- Updating browsers can continue to operate without contact to server
- Conflicting updates while offline are resolved by server
- Server can recover lost state from clients
- Livestream overlay support

## Project structure

```
.
├── .cargo/
├── .git/
├── .gitignore
├── .idea/
├── Cargo.lock
├── Cargo.toml
├── LICENSE
├── README.md
├── config/
│   └── default.toml
├── config.toml
├── crates/
│   ├── backend-bin/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs
│   ├── backend-lib/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── auth.rs
│   │       ├── config.rs
│   │       ├── error.rs
│   │       ├── lib.rs
│   │       ├── meet.rs
│   │       ├── meet_actor.rs
│   │       ├── messages.rs
│   │       ├── metrics.rs
│   │       ├── middleware/
│   │       │   └── rate_limit.rs
│   │       ├── storage.rs
│   │       ├── websocket.rs
│   │       └── ws_router.rs
│   └── common/
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs
├── data/
│   ├── current-meets/
│   └── finished-meets/
├── demo.sh
├── deny.toml
├── Dockerfile
├── infrastructure/
├── pyproject.toml
├── repomix-output.txt
├── rust-toolchain.toml
├── rustfmt.toml
├── target/
└── websocket_test.sh
```

## Architecture

The server is built with Rust and uses the following components:

- **WebSocket Router**: Handles WebSocket connections and message routing
- **Meet Actor**: Manages the state of each meet and handles conflict resolution
- **Authentication**: Manages session tokens and password verification
- **Storage**: Persists meet data to the filesystem

The server is split into two crates:

- `openlifter-backend-lib`: Core functionality and business logic
- `openlifter-backend-bin`: Binary crate with CLI and configuration

## Key Components

### 1. Storage

The server supports pluggable storage backends through the `Storage` trait. Currently implemented:

- `FlatFileStorage`: Simple file-based storage (default)

### 2. Authentication

- Password hashing with scrypt
- Session management with TTL and cleanup
- Configurable password requirements

### 3. Metrics

The server exposes Prometheus metrics on port 9091:

- `ws.connection`: WebSocket connection counter
- `ws.active`: Active WebSocket connections gauge
- `meet.created`: Meet creation counter
- `meet.joined`: Meet join counter
- `update.accepted`: Update counter
- `update.batch_size`: Update batch size histogram
- `handler.duration_ms`: Handler duration histogram

### 5. handlers

### 6. middleware

### 7. validation

## Configuration

Configuration can be provided through:

1. Config file (TOML, YAML, or JSON)
2. Environment variables (prefixed with `OPENLIFTER_`)
3. Command line arguments

Example config.toml:
```toml
bind_addr = "127.0.0.1:3000"
data_dir = "data"
log_level = "info"

[password_requirements]
min_length = 10
require_uppercase = true
require_lowercase = true
require_digit = true
require_special = true
```

## Protocol

The client-server protocol is JSON-based and includes the following message types:

### Client to Server

- `CreateMeet`: Create a new meet
- `JoinMeet`: Join an existing meet
- `UpdateInit`: Send updates to the server
- `ClientPull`: Request updates from the server
- `PublishMeet`: Publish a meet to OpenLifter

### Server to Client

- `MeetCreated`: Response to CreateMeet
- `MeetJoined`: Response to JoinMeet
- `JoinRejected`: Response to JoinMeet (error)
- `UpdateAck`: Acknowledgment of updates
- `UpdateRejected`: Rejection of updates
- `UpdateRelay`: Updates from other clients
- `ServerPull`: Response to ClientPull
- `PublishAck`: Acknowledgment of publish
- `MalformedMessage`: Error for malformed messages
- `UnknownMessageType`: Error for unknown message types
- `InvalidSession`: Error for invalid session tokens

## Conflict Resolution

The server implements a priority-based conflict resolution system:

1. Each location has a priority level
2. When conflicts occur, the higher priority location's updates take precedence
3. Conflicts are detected when:
   - Multiple clients update the same state
   - At least one client's update's last seen server sequence number is before that of at least one other client's update

## Storage

Meet data is stored in the filesystem with the following structure:

```
data/
  current-meets/
    [meet-id]/
      updates.log
      auth.json
  finished-meets/
    [meet-id]/
      updates.log
      auth.json
      opl.csv
      email.txt
```

## API Documentation

### WebSocket Endpoints

- `/ws`: Main WebSocket endpoint for meet management
- `/ws/health`: Health check endpoint

### REST Endpoints

- `GET /health`: Server health check
- `GET /metrics`: Prometheus metrics endpoint
- `POST /admin/log-level`: Change log level

### Message Format

All messages are JSON-encoded with the following structure:

```json
{
  "type": "MessageType",
  "payload": {
    // Message-specific fields
  }
}
```

-----

# Demo
The demo script has been expanded to better demonstrate the WebSocket server's capabilities as outlined in the original design spec. Here's what the new demo does:

1. **Phase 1**: Creates a meet and stores the meet ID and session token
2. **Phase 2**: Simulates a second client joining the meet with the same credentials
3. **Phase 3**: Sends updates from the first client (setting a lifter's name)
4. **Phase 4**: Sends updates from the second client (setting a lifter's bodyweight)
5. **Phase 5**: Simulates concurrent updates from both clients to demonstrate potential conflict handling
6. **Phase 6**: Establishes long-lived WebSocket connections in separate terminals to show real-time updates

The script now illustrates more of the server's capabilities, including:
- Multi-client support
- Session management
- Real-time updates
- Handling of concurrent modifications

This demo provides a much more comprehensive showcase of the functionality described in the original design spec, demonstrating the WebSocket server's ability to support collaborative meet management.