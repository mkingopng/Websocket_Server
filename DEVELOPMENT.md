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
3. Commit your changes (the pre-commit hook will verify code quality)
4. Submit a pull request

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