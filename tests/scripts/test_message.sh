#!/bin/bash

# Kill any existing servers
killall -q backend-bin 2>/dev/null
sleep 1

# Start the server with TRACE level logging
echo "Starting server with DEBUG logging..."
RUST_LOG=debug,tower_http=debug,axum=debug,backend_lib=trace cargo run -p backend-bin &
SERVER_PID=$!

# Wait for the server to start
echo "Waiting for server to start..."
sleep 7

# Create a message in a file
cat > /tmp/message.json << 'EOF'
{
  "type": "CreateMeet",
  "payload": {
    "meet_id": "test-meet-99",
    "password": "TestPassword123!",
    "location_name": "Test Location",
    "priority": 10
  }
}
EOF

echo "Test message:"
cat /tmp/message.json
echo

# Send the message using websocat
echo "Attempting to connect with websocat..."
cat /tmp/message.json | websocat -v ws://127.0.0.1:3000/ws

# Clean up
echo "Stopping server..."
kill $SERVER_PID
rm /tmp/message.json
echo "Test complete." 