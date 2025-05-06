# WebSocket Server Test Scripts

This directory contains various test scripts for the WebSocket Server project. These scripts are used for testing different aspects of the server functionality.

## Main Demo

- **demo.sh** - The main demonstration script that shows the complete workflow including meet creation, joining, updates, and publishing.

## Utility Test Scripts

- **minimal_connect_test.sh** - Basic test for establishing a WebSocket connection.
- **minimal_test.sh** - Minimal test that creates a meet and verifies the response.
- **test_message.sh** - Tests sending a specific message format to the server.
- **test_msg_format.sh** - Tests the message format parsing on the server.
- **test_websocket.sh** - More comprehensive WebSocket connection tests.
- **wscat_test.sh** - Alternative test using wscat instead of websocat tool.

## Test Data Files

- **test_msg.json** - Sample JSON message for WebSocket testing.

## Usage

All scripts can be run directly from this directory:

```bash
# Run the main demo
./demo.sh

# Run a specific test
./minimal_test.sh
```

## Notes

- These scripts create temporary directories and files, which are cleaned up when the script exits
- All scripts use the same port (3000) and will kill any existing server instances before starting
- The scripts require websocat or wscat to be installed 