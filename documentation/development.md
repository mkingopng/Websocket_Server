# Development Guide

This document provides guidelines and setup instructions for development on the OpenLifter WebSocket Server project.

## Setup

1. Install Rust and Cargo using [rustup](https://rustup.rs/)
2. Clone the repository: `git clone https://github.com/your-username/Websocket_Server.git`
3. Navigate to the project directory: `cd Websocket_Server`
4. Install Git hooks: `./setup-hooks.sh`

## Git Hooks

We use Git hooks to maintain code quality:

### Pre-commit Hook

The pre-commit hook runs automatically before each commit and performs the following checks:

1. Code formatting with `cargo fmt`
2. Linting with `cargo clippy`
3. Compilation check with `cargo check`
4. Unit tests with `cargo test`

If any of these checks fail, the commit will be aborted, and you'll need to fix the issues before committing.

To bypass the hooks in exceptional cases, use the `--no-verify` flag: `git commit --no-verify`.

## Project Structure

- `crates/` - Contains the Rust crates that make up the project
  - `backend-bin/` - The binary crate that runs the server
  - `backend-lib/` - The library crate containing the main server functionality
  - `common/` - Shared code between the server and client

## Testing

Besides the unit tests that run automatically in the pre-commit hook, we have additional test scripts:

- `./demo.sh` - Demonstrates the basic functionality of the WebSocket server
- `./network_resilience_test.sh` - Tests the network resilience features

## Development Workflow

1. Make changes to the codebase
2. Run tests locally: `cargo test --all`
3. run `cargo fmt` and `cargo clippy` to ensure code quality
4. run `cargo check` to ensure the code compiles
5. run e2e tests
6. Commit your changes and run the pre-commit hook
7. Submit a pull request

## Coding Standards

We follow these coding standards:

1. Use `cargo fmt` for consistent code formatting
2. Address all clippy warnings
3. Write tests for new functionality
4. Keep functions small and focused
5. Document public APIs

## Troubleshooting

If you encounter issues with the pre-commit hook:

1. Make sure the hook is executable: `chmod +x .git/hooks/pre-commit`
2. Try running the individual checks manually:
    - `cargo fmt --all -- --check`
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo check --all-targets --all-features`
    - `cargo test --all`


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


## Integration with OpenLifter

The client-side integration with OpenLifter requires:

1. A WebSocket connection to the server
2. Handling of the client-server protocol
3. State synchronization with the server
4. Conflict resolution on the client side

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

## Performance Considerations

The WebSocket server is designed to handle multiple concurrent connections efficiently. Here are some performance considerations:

- **Connection Limits**: By default, the server is configured to handle up to 1000 concurrent connections. Adjust this in the configuration if needed.
- **Resource Usage**: Each WebSocket connection consumes memory. Monitor server resources during high-load periods.
- **Batch Processing**: Updates are processed in batches to reduce overhead. Adjust batch size in configuration for your use case.
- **Scaling**: For high-load scenarios, consider running multiple server instances behind a load balancer.

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

-----

## WebSocket Testing

The `websocket_test.sh` script performs a comprehensive test of the WebSocket server:

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

-----

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

### Docker

Build the image:
```bash
docker build -t openlifter-backend-lib .
```

Run the container:
```bash
docker run -p 3000:3000 -v data:/app/data openlifter-backend-lib
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
