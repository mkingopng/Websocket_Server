# server-app

This is the main **binary crate** for the OpenLifter backend. It starts the WebSocket server and orchestrates the initialization process.

## Purpose

To act as the application entrypoint, wiring together services from `server-core` and message schemas from `server-protocols`.

its only responsibility is startup, orchestration, and graceful shutdown. Core functionality should live in server-core.

## Features

- Loads environment and config
- Initializes logger
- Starts WebSocket server and begins accepting client connections

## Usage

Run locally:
```bash
cargo run -p server-app
```