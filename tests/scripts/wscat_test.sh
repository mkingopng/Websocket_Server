#!/bin/bash

# Ensure wscat is installed
if ! command -v wscat &> /dev/null; then
    echo "Installing wscat..."
    npm install -g wscat
fi

# Create data directory structure
echo "Ensuring data directories exist..."
mkdir -p data/current-meets data/finished-meets data/sessions

# Clean up any previous processes
echo "Cleaning up any existing processes..."
killall -q backend-bin 2>/dev/null
sleep 1

# Start the server
echo "Starting WebSocket server..."
RUST_LOG=trace cargo run -p backend-bin &
SERVER_PID=$!

# Wait for the server to start
echo "Waiting for server to start (8 seconds)..."
sleep 8

# Create our test message
cat > message.json << EOF
{
  "msgType": "CreateMeet",
  "meet_id": "test-meet-1",
  "password": "TestPassword123!",
  "location_name": "Test Location",
  "priority": 10
}
EOF

echo "Message content:"
cat message.json

# Use wscat to connect and send the message
echo "Connecting with wscat and sending message..."
wscat -c ws://127.0.0.1:3000/ws --execute "$(cat message.json)"

# Clean up
echo "Cleaning up..."
kill $SERVER_PID
rm -f message.json
echo "Test completed." 