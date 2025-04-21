#!/bin/bash

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${YELLOW}OpenLifter WebSocket Server Demonstration${NC}"
echo "============================================="

# Function to check if a process is running
check_process() {
    pgrep -f "$1" > /dev/null
    return $?
}

# Start the server in the background
echo -e "${YELLOW}Starting WebSocket server...${NC}"
cargo run -p backend-bin &
SERVER_PID=$!

# Wait for the server to start
echo "Waiting for server to start..."
sleep 3

# Check if server is running
if ! check_process "backend-bin"; then
    echo -e "${RED}Failed to start server!${NC}"
    exit 1
fi

echo -e "${GREEN}Server started successfully!${NC}"
echo "Server is running on ws://127.0.0.1:3000/ws"

# Create a test message file
cat > test_message.json << EOF
{
  "type": "CreateMeet",
  "payload": {
    "meet_id": "demo-meet-123",
    "password": "TestPassword123!"
  }
}
EOF

echo -e "${YELLOW}Connecting to WebSocket server with websocat...${NC}"
echo "Sending test message to create a meet..."

# Connect to the server and send the test message
websocat ws://127.0.0.1:3000/ws --ping-interval 5 < test_message.json &
WEBSOCAT_PID=$!

# Wait for a response
echo "Waiting for server response..."
sleep 5

# Clean up
echo -e "${YELLOW}Cleaning up...${NC}"
kill $WEBSOCAT_PID 2>/dev/null
kill $SERVER_PID 2>/dev/null
rm test_message.json

echo -e "${GREEN}Demonstration completed!${NC}"
echo "To test manually, run: websocat ws://127.0.0.1:3000/ws" 