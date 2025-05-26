# server-app/src

This directory contains the main source code for the `server-app` crate, which is the entrypoint for the OpenLifter backend WebSocket server.

## Key Modules
- `websocket.rs`: WebSocket server logic and client connection management.
- `ws_router.rs`: Routes incoming WebSocket messages to appropriate handlers.
- `meet_actor.rs`: Manages the state and lifecycle of meets (sessions).
- `storage.rs`: Handles persistent storage for meets and session data.
- `messages.rs`: Defines message types and serialization for client-server communication.
- `validation/`: Input validation logic for messages and data (see README in this folder).
- `auth/`: Authentication, session management, and rate limiting (see README in this folder).
- `config/`: Loads and manages configuration settings.
- `handlers/`: Contains request and event handler implementations (see README in this folder).
- `middleware/`: Middleware components such as rate limiting (see README in this folder).
- `error.rs`: Error types and error handling utilities.
- `metrics.rs`: Metrics collection and monitoring utilities.

## Purpose
Implements the core backend logic for the OpenLifter WebSocket server, including authentication, message routing, meet management, and persistent storage. 