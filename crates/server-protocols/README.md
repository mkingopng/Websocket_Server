# server-protocols

This crate defines the **WebSocket protocol messages** and supporting data structures used for communication between OpenLifter clients and the server.

It includes:
- Message enums for client-to-server and server-to-client communication
- Update and sync data structures
- Meet metadata like endpoint priorities

## Purpose

To serve as a shared schema crate across both `server-core` and clients (or simulations). It enables serialization and deserialization of structured messages using `serde`.

## Usage

```rust
use server_protocols::{ClientToServer, ServerToClient};
```