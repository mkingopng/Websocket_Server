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

# Run E2E tests
cd tests/e2e
./websocket_test.sh
./network_resilience_test.sh
```

#### Test Structure

The tests are organized in the following structure:

```
tests/
├── e2e/               # End-to-end tests (shell scripts)
│   ├── websocket_test.sh
│   └── network_resilience_test.sh
├── integration/       # Integration tests
├── performance/       # Performance tests
├── unit/              # Unit tests
└── README.md          # Documentation
```

For more details about test coverage, see [documentation/test_coverage.md](documentation/test_coverage.md).

### Running

```bash
# Run with default settings
cargo run -p openlifter-backend-lib-bin

# Run with custom config
cargo run -p openlifter-backend-lib-bin -- --config config.toml

# Run with custom bind address
cargo run -p openlifter-backend-lib-bin -- --bind 0.0.0.0:3000
```

### Important Cargo Commands

```bash
# Build the project
cargo build

# Run the linter to check for code quality issues
cargo clippy

# Run the application
cargo run
```

These commands are essential for development:
- `cargo build` - Compiles the project and checks for compilation errors
- `cargo clippy` - Runs the Rust linter to catch common mistakes and suggest improvements
- `cargo run` - Builds and runs the application

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

## Development Environment Setup

### Recommended Tools

- **rustup**: Rust toolchain installer
- **cargo-edit**: For adding dependencies (`cargo install cargo-edit`)
- **cargo-watch**: For auto-reloading during development (`cargo install cargo-watch`)
- **cargo-expand**: For macro expansion debugging (`cargo install cargo-expand`)
- **rust-analyzer**: IDE extension for Rust language support

### VS Code Extensions

- rust-analyzer
- crates
- even-better-toml
- serde-json
- CodeLLDB

## Troubleshooting

### Common Issues

#### WebSocket Connection Failures

- Check that the server is running and accessible
- Verify firewall settings aren't blocking WebSocket connections
- Ensure the client is using the correct WebSocket URL

#### Authentication Issues

- Verify meet ID and password are correct
- Check that the session hasn't expired
- Ensure the server's authentication configuration matches client expectations

#### Performance Problems

- Check server logs for bottlenecks
- Verify the server has sufficient resources
- Consider increasing the connection pool size

## Contributing Guidelines

We welcome contributions to the OpenLifter WebSocket Server! Here's how you can help:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Code Style

- Follow the Rust standard style guide
- Run `cargo fmt` before submitting changes
- Ensure all tests pass with `cargo test`
- Run `cargo clippy` to catch common mistakes

### Pull Request Process

1. Update the README.md with details of changes if needed
2. Update the CHANGELOG.md with a note describing your changes
3. The PR will be merged once you have the sign-off of at least one maintainer

## Version Information

The project follows [Semantic Versioning](https://semver.org/):

- **Major version**: Incompatible API changes
- **Minor version**: Add functionality in a backward-compatible manner
- **Patch version**: Backward-compatible bug fixes

To check which version you're running:

```bash
cargo run -p openlifter-backend-lib-bin -- --version
```

## Performance Considerations

The WebSocket server is designed to handle multiple concurrent connections efficiently. Here are some performance considerations:

- **Connection Limits**: By default, the server is configured to handle up to 1000 concurrent connections. Adjust this in the configuration if needed.
- **Resource Usage**: Each WebSocket connection consumes memory. Monitor server resources during high-load periods.
- **Batch Processing**: Updates are processed in batches to reduce overhead. Adjust batch size in configuration for your use case.
- **Scaling**: For high-load scenarios, consider running multiple server instances behind a load balancer.

## Security Best Practices

When deploying the OpenLifter WebSocket Server, follow these security best practices:

- **TLS**: Always use TLS for WebSocket connections in production
- **Authentication**: Enable authentication for all meets
- **Password Requirements**: Use strong password requirements
- **Rate Limiting**: Implement rate limiting to prevent abuse
- **Input Validation**: Validate all client inputs
- **Regular Updates**: Keep dependencies updated to patch security vulnerabilities
- **Principle of Least Privilege**: Run the server with minimal required permissions

## Logging Configuration

The server uses structured logging with the following configuration options:

```toml
[logging]
level = "info"  # debug, info, warn, error
format = "json"  # json, text
file = "logs/server.log"  # Optional file output
```

To change log levels at runtime:

```bash
curl -X POST http://localhost:3000/admin/log-level -d '{"level":"debug"}'
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

## Development Workflow

### Typical Development Cycle

1. **Setup**: Clone the repository and install dependencies
2. **Development**: Make changes in a feature branch
3. **Testing**: Run tests and ensure they pass
4. **Code Review**: Submit a pull request for review
5. **Integration**: After approval, merge into the main branch
6. **Deployment**: Deploy to staging, then production

### Branching Strategy

- `main`: Production-ready code
- `develop`: Integration branch for features
- `feature/*`: Feature branches
- `bugfix/*`: Bug fix branches
- `release/*`: Release preparation branches

## Dependencies

The project relies on the following major dependencies:

- **axum**: Web framework for building APIs
- **tokio**: Asynchronous runtime
- **serde**: Serialization/deserialization
- **tracing**: Structured logging
- **metrics**: Metrics collection
- **config**: Configuration management
- **tungstenite**: WebSocket implementation

For a complete list of dependencies, see the Cargo.toml files in each crate.

## WebSocket Testing

The repository includes two test scripts to demonstrate and verify the WebSocket server functionality:

### Basic Demonstration

The `demo.sh` script provides a simple demonstration of the WebSocket server:

```bash
./demo.sh
```

This script:
1. Starts the WebSocket server
2. Connects to it using websocat
3. Sends a test message to create a meet
4. Displays the server response
5. Cleans up resources

### Comprehensive Test

The `websocket_test.sh` script performs a more comprehensive test of the WebSocket server:

```bash
./websocket_test.sh
```

This script demonstrates multiple WebSocket operations:
1. Creating a meet
2. Joining a meet
3. Sending updates to a meet
4. Displaying server responses at each step

### Manual Testing with websocat

You can also test the WebSocket server manually using websocat:

```bash
# Connect to the WebSocket server
websocat ws://127.0.0.1:3000/ws

# Send a JSON message (paste into the terminal)
{"type":"CreateMeet","payload":{"meet_id":"test-meet","password":"TestPassword123!"}}
```

For more information about websocat, visit [https://github.com/vi/websocat](https://github.com/vi/websocat).

---

# project structure

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