#!/bin/bash

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${YELLOW}OpenLifter WebSocket Server Comprehensive Test${NC}"
echo "====================================================="

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

# Create test message files
echo -e "${BLUE}Creating test messages...${NC}"

# Create a meet
cat > create_meet.json << EOF
{
  "type": "CreateMeet",
  "payload": {
    "meet_id": "test-meet-123",
    "password": "TestPassword123!"
  }
}
EOF

# Join a meet
cat > join_meet.json << EOF
{
  "type": "JoinMeet",
  "payload": {
    "meet_id": "test-meet-123",
    "password": "TestPassword123!"
  }
}
EOF

# Send an update
cat > send_update.json << EOF
{
  "type": "UpdateInit",
  "payload": {
    "meet_id": "test-meet-123",
    "session_token": "REPLACE_WITH_TOKEN",
    "updates": [
      {
        "location": "platform",
        "value": "Test Platform",
        "timestamp": 1234567890
      }
    ]
  }
}
EOF

# Test 1: Create a meet
echo -e "${YELLOW}Test 1: Creating a meet...${NC}"
echo "Sending CreateMeet message..."

# Connect to the server and send the create meet message
websocat ws://127.0.0.1:3000/ws --ping-interval 5 < create_meet.json > create_response.json &
WEBSOCAT_PID1=$!

# Wait for a response
echo "Waiting for server response..."
sleep 3

# Display the response
echo -e "${GREEN}Server response:${NC}"
cat create_response.json
echo ""

# Extract session token from response (this is a simplified example)
SESSION_TOKEN=$(grep -o '"session_token":"[^"]*"' create_response.json | cut -d'"' -f4)
echo -e "${BLUE}Session token: ${SESSION_TOKEN}${NC}"

# Update the send_update.json with the session token
sed -i "s/REPLACE_WITH_TOKEN/${SESSION_TOKEN}/g" send_update.json

# Test 2: Join the meet
echo -e "${YELLOW}Test 2: Joining the meet...${NC}"
echo "Sending JoinMeet message..."

# Connect to the server and send the join meet message
websocat ws://127.0.0.1:3000/ws --ping-interval 5 < join_meet.json > join_response.json &
WEBSOCAT_PID2=$!

# Wait for a response
echo "Waiting for server response..."
sleep 3

# Display the response
echo -e "${GREEN}Server response:${NC}"
cat join_response.json
echo ""

# Test 3: Send an update
echo -e "${YELLOW}Test 3: Sending an update...${NC}"
echo "Sending UpdateInit message..."

# Connect to the server and send the update message
websocat ws://127.0.0.1:3000/ws --ping-interval 5 < send_update.json > update_response.json &
WEBSOCAT_PID3=$!

# Wait for a response
echo "Waiting for server response..."
sleep 3

# Display the response
echo -e "${GREEN}Server response:${NC}"
cat update_response.json
echo ""

# Clean up
echo -e "${YELLOW}Cleaning up...${NC}"
kill $WEBSOCAT_PID1 2>/dev/null
kill $WEBSOCAT_PID2 2>/dev/null
kill $WEBSOCAT_PID3 2>/dev/null
kill $SERVER_PID 2>/dev/null
rm create_meet.json join_meet.json send_update.json create_response.json join_response.json update_response.json

echo -e "${GREEN}Comprehensive test completed!${NC}"
echo "To test manually, run: websocat ws://127.0.0.1:3000/ws" 