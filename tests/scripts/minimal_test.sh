#!/bin/bash

# Create data directory structure if it doesn't exist
echo "Ensuring data directories exist..."
mkdir -p data/current-meets data/finished-meets data/sessions

# Clean up any previous processes
echo "Cleaning up any existing processes..."
killall -q backend-bin 2>/dev/null
sleep 1

# Start the server with debug logging
echo "Starting WebSocket server..."
RUST_LOG=debug,tower_http=debug,axum=debug,backend_lib=trace cargo run -p backend-bin &
SERVER_PID=$!

# Wait for the server to start
echo "Waiting for server to start (8 seconds)..."
sleep 8

# Test basic connectivity
echo "Testing basic WebSocket connection..."
timeout 2 websocat ws://127.0.0.1:3000/ws
if [ $? -eq 124 ] || [ $? -eq 0 ]; then
    echo "Basic WebSocket connection successful!"
else
    echo "WebSocket connection failed!"
    kill $SERVER_PID
    exit 1
fi

# Create temporary file for responses
RESPONSE_FILE=$(mktemp)

# Create a meet
echo "Creating a meet..."
CREATE_MESSAGE='{
  "msgType": "CreateMeet",
  "meet_id": "test-meet-1",
  "password": "TestPassword123!",
  "location_name": "Test Location",
  "priority": 10
}'

echo "$CREATE_MESSAGE" > /tmp/create_msg.json
cat /tmp/create_msg.json | websocat -v ws://127.0.0.1:3000/ws > $RESPONSE_FILE

echo "Response:"
cat $RESPONSE_FILE

# Clean up
echo "Cleaning up..."
kill $SERVER_PID
rm -f /tmp/create_msg.json $RESPONSE_FILE
echo "Test completed."
