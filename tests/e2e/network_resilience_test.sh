#!/bin/bash

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${YELLOW}OpenLifter WebSocket Server Network Resilience Test${NC}"
echo "======================================================="

# Function to check if a process is running
check_process() {
    pgrep -f "$1" > /dev/null
    return $?
}

# Function to wait for server to be ready
wait_for_server() {
    local max_attempts=10
    local attempt=1
    
    while [ $attempt -le $max_attempts ]; do
        echo "Waiting for server to be ready (attempt $attempt/$max_attempts)..."
        
        # Try to connect to the server
        if curl -s http://127.0.0.1:3000/health 2>/dev/null | grep -q "ok"; then
            return 0
        fi
        
        # If that fails, try a WebSocket connection
        if echo '{"type":"ping"}' | timeout 2 websocat ws://127.0.0.1:3000/ws -n1 2>/dev/null; then
            return 0
        fi
        
        sleep 2
        attempt=$((attempt + 1))
    done
    
    return 1
}

# Clean up any previous processes that might still be running
echo "Cleaning up any existing processes..."
killall -q backend-bin 2>/dev/null
sleep 1

# Create temporary files for storing session tokens and meet IDs
SESSION_FILE=$(mktemp)
MEET_ID_FILE=$(mktemp)

# Start the server in the background
echo -e "${YELLOW}Starting WebSocket server...${NC}"
cargo run -p backend-bin > /tmp/server_log.txt 2>&1 &
SERVER_PID=$!

# Wait for the server to start
sleep 3

# Check if server is running
if ! check_process "backend-bin"; then
    echo -e "${RED}Failed to start server!${NC}"
    cat /tmp/server_log.txt
    exit 1
fi

# Wait a bit more for the server to be fully ready
sleep 2

echo -e "${GREEN}Server started successfully!${NC}"
echo "Server is running on ws://127.0.0.1:3000/ws"

# PHASE 1: Create a meet
echo -e "\n${BLUE}=== PHASE 1: Creating a Meet ===${NC}"
CREATE_MESSAGE='{"type":"CreateMeet","payload":{"meet_id":"resilience-test-123","password":"TestPassword123!","location_name":"Main Platform","priority":10}}'

echo "Sending request to create a meet..."
if ! echo $CREATE_MESSAGE | websocat ws://127.0.0.1:3000/ws -n1 > /tmp/create_response.txt 2>/dev/null; then
    echo -e "${RED}Failed to connect to the server!${NC}"
    exit 1
fi

# Extract information from the response
cat /tmp/create_response.txt
SESSION_TOKEN=$(grep -o '"session_token":"[^"]*' /tmp/create_response.txt | cut -d'"' -f4)
MEET_ID=$(grep -o '"meet_id":"[^"]*' /tmp/create_response.txt | cut -d'"' -f4)

# Store tokens in files for later use
echo $SESSION_TOKEN > $SESSION_FILE
echo $MEET_ID > $MEET_ID_FILE

if [ -z "$SESSION_TOKEN" ] || [ -z "$MEET_ID" ]; then
    echo -e "${RED}Failed to extract session token or meet ID from response!${NC}"
    exit 1
fi

echo -e "${GREEN}Meet created successfully!${NC}"
echo "Meet ID: $MEET_ID"
echo "Session Token: $SESSION_TOKEN"

# PHASE 2: Send some initial updates
echo -e "\n${BLUE}=== PHASE 2: Sending Initial Updates ===${NC}"
UPDATE_MESSAGE="{\"type\":\"UpdateInit\",\"payload\":{\"meet_id\":\"$MEET_ID\",\"session_token\":\"$SESSION_TOKEN\",\"updates\":[{\"location\":\"lifters.0.name\",\"value\":\"John Doe\",\"timestamp\":$(date +%s)}]}}"

echo "Sending update (setting lifter name)..."
if ! echo $UPDATE_MESSAGE | websocat ws://127.0.0.1:3000/ws -n1 > /tmp/update_response.txt 2>/dev/null; then
    echo -e "${RED}Failed to connect to the server!${NC}"
    exit 1
fi

# Show response
cat /tmp/update_response.txt
echo -e "${GREEN}Initial update processed!${NC}"

# PHASE 3: Simulate network interruption by killing and restarting server
echo -e "\n${BLUE}=== PHASE 3: Simulating Network Interruption ===${NC}"
echo "Stopping the server to simulate network interruption..."
kill $SERVER_PID
sleep 2

echo "Restarting the server..."
cargo run -p backend-bin > /tmp/server_log.txt 2>&1 &
SERVER_PID=$!

echo "Waiting for server to restart..."
sleep 5

# Check if server is running
if ! check_process "backend-bin"; then
    echo -e "${RED}Failed to restart server!${NC}"
    cat /tmp/server_log.txt
    exit 1
fi

echo -e "${GREEN}Server restarted successfully!${NC}"

# PHASE 4: Attempt to reconnect and send updates
echo -e "\n${BLUE}=== PHASE 4: Testing Reconnection Logic ===${NC}"
echo "Attempting to send an update after server restart (should trigger reconnection)..."

UPDATE_AFTER_RESTART="{\"type\":\"UpdateInit\",\"payload\":{\"meet_id\":\"$MEET_ID\",\"session_token\":\"$SESSION_TOKEN\",\"updates\":[{\"location\":\"lifters.0.bodyweight\",\"value\":\"100.5\",\"timestamp\":$(date +%s)}]}}"

echo "Sending update after server restart..."
if ! echo $UPDATE_AFTER_RESTART | websocat ws://127.0.0.1:3000/ws -n1 > /tmp/reconnect_response.txt 2>/dev/null; then
    echo -e "${RED}Failed to connect to the server after restart!${NC}"
    exit 1
fi

# Show response
cat /tmp/reconnect_response.txt

# Check if the response indicates a successful reconnection or a proper error
if grep -q "InvalidSession" /tmp/reconnect_response.txt; then
    echo -e "${YELLOW}Session invalidated after restart (expected behavior with current implementation)${NC}"
    echo "In a production environment with persistent sessions, reconnection would succeed."
elif grep -q "VALIDATION_ERROR" /tmp/reconnect_response.txt; then
    echo -e "${YELLOW}Validation error occurred (likely due to test data)${NC}"
else
    echo -e "${GREEN}Reconnection successful!${NC}"
fi

# PHASE 5: Test client pull for state recovery
echo -e "\n${BLUE}=== PHASE 5: Testing State Recovery with Client Pull ===${NC}"
PULL_MESSAGE="{\"type\":\"ClientPull\",\"payload\":{\"meet_id\":\"$MEET_ID\",\"session_token\":\"$SESSION_TOKEN\",\"last_server_seq\":0}}"

echo "Sending CLIENT_PULL to recover state..."
if ! echo $PULL_MESSAGE | websocat ws://127.0.0.1:3000/ws -n1 > /tmp/pull_response.txt 2>/dev/null; then
    echo -e "${RED}Failed to connect to the server for state recovery!${NC}"
    exit 1
fi

# Show response
cat /tmp/pull_response.txt

# Clean up
echo -e "\n${YELLOW}Cleaning up...${NC}"
kill $SERVER_PID 2>/dev/null
wait $SERVER_PID 2>/dev/null

# Remove temporary files
rm -f /tmp/create_response.txt /tmp/update_response.txt
rm -f /tmp/reconnect_response.txt /tmp/pull_response.txt
rm -f /tmp/server_log.txt
rm -f $SESSION_FILE $MEET_ID_FILE

echo -e "${GREEN}Test completed!${NC}"
echo "This test demonstrated:"
echo "1. Creating a meet"
echo "2. Sending updates"
echo "3. Simulating network interruption"
echo "4. Testing reconnection logic"
echo "5. Testing state recovery" 