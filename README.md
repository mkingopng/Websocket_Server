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

## Architecture

The server is split into two crates:

- `openlifter-backend-lib`: Core functionality and business logic
- `openlifter-backend-bin`: Binary crate with CLI and configuration

### Storage

The server supports pluggable storage backends through the `Storage` trait. Currently implemented:

- `FlatFileStorage`: Simple file-based storage (default)

### Authentication

- Password hashing with scrypt
- Session management with TTL and cleanup
- Configurable password requirements

### Metrics

The server exposes Prometheus metrics on port 9091:

- `ws.connection`: WebSocket connection counter
- `ws.active`: Active WebSocket connections gauge
- `meet.created`: Meet creation counter
- `meet.joined`: Meet join counter
- `update.accepted`: Update counter
- `update.batch_size`: Update batch size histogram
- `handler.duration_ms`: Handler duration histogram

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

## Development

### Prerequisites

- Rust 1.75 or later
- Cargo

### Building

```bash
cargo build --release
```

### Testing

```bash
# Run all tests
cargo test --workspace

# Run tests with feature combinations
cargo hack --feature-powerset --each-feature test

# Run specific test
cargo test -p openlifter-backend-lib test_name
```

### Running

```bash
# Run with default settings
cargo run -p openlifter-backend-lib-bin

# Run with custom config
cargo run -p openlifter-backend-lib-bin -- --config config.toml

# Run with custom bind address
cargo run -p openlifter-backend-lib-bin -- --bind 0.0.0.0:3000
```

### Docker

Build the image:
```bash
docker build -t openlifter-backend-lib .
```

Run the container:
```bash
docker run -p 3000:3000 -v data:/app/data openlifter-backend-lib
```

## License

This project is licensed under the GPL-3 License - see the LICENSE file for details.

## Project Structure

```
openlifter-ws-backend/
├─ Cargo.toml              # Workspace manifest
├─ rust-toolchain.toml     # Pinned toolchain
├─ crates/
│  ├─ common/              # Shared protocol/types
│  │   ├─ Cargo.toml
│  │   └─ src/
│  │       └─ lib.rs
│  └─ backend/             # Server logic
│      ├─ Cargo.toml
│      └─ src/
│          ├─ lib.rs       # Core functionality
│          ├─ auth.rs      # Authentication
│          ├─ error.rs     # Error handling
│          ├─ meet_actor.rs # Meet actor
│          ├─ storage.rs   # Storage
│          ├─ ws_router.rs # WebSocket router
│          └─ bin/
│              └─ server.rs # Main binary
├─ infrastructure/         # Infrastructure code
│  ├─ cdk/                # Python CDK app
│  ├─ Dockerfile
│  └─ README.md
└─ .github/
   └─ workflows/ci.yml    # CI workflow
```

## Features

- Multiple browsers running the same meet simultaneously
- Multiple updating browsers
- Multiple display-only browsers
- Browsers can make meets live or join already live meets
- Authentication via meet ID and password
- Updating browsers can continue to operate without contact to server
- Conflicting updates while offline are resolved by server
- Server can recover lost state from clients
- Livestream overlay support

## Architecture

The server is built with Rust and uses the following components:

- **WebSocket Router**: Handles WebSocket connections and message routing
- **Meet Actor**: Manages the state of each meet and handles conflict resolution
- **Authentication**: Manages session tokens and password verification
- **Storage**: Persists meet data to the filesystem

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

## Integration with OpenLifter

The client-side integration with OpenLifter requires:

1. A WebSocket connection to the server
2. Handling of the client-server protocol
3. State synchronization with the server
4. Conflict resolution on the client side
