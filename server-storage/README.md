# server-storage

This directory is used for persistent storage of meet and session data by the OpenLifter backend server.

## Subdirectories
- `current-meets/`: Data for ongoing meets (e.g., `meet_123.json`)
- `finished-meets/`: Data for completed meets (e.g., `meet_456.json`)
- `sessions/`: Session keys and authentication data (e.g., `session_key`)

## Note
This directory is primarily for development and testing. Data may be reset or cleared as needed. Files are typically in JSON or plain text format. 