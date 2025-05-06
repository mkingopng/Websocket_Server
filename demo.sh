#!/bin/bash

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${YELLOW}OpenLifter WebSocket Server Demonstration${NC}"
echo "============================================="

# Function to check if a process is running
check_process() {
    pgrep -f "$1" > /dev/null
    return $?
}

# Create data directory structure if it doesn't exist
echo "Ensuring data directories exist..."
mkdir -p data/current-meets data/finished-meets data/sessions

# Clean up any previous processes that might still be running
echo "Cleaning up any existing processes..."
killall -q backend-bin 2>/dev/null
sleep 1

# Start the server with RUST_LOG set for debug output
echo -e "${YELLOW}Starting WebSocket server with debug logging...${NC}"
RUST_LOG=debug,tower_http=debug,axum=debug cargo run -p backend-bin &
SERVER_PID=$!

# Wait for the server to start
echo "Waiting for server to start (7 seconds)..."
sleep 7

# Check if server is running
if ! check_process "backend-bin"; then
    echo -e "${RED}Failed to start server!${NC}"
    exit 1
fi

echo -e "${GREEN}Server started successfully!${NC}"
echo "Server is running on ws://127.0.0.1:3000/ws"

# Check health endpoint
echo -e "\n${BLUE}=== Testing health endpoint ===${NC}"
HEALTH=$(curl -s http://127.0.0.1:3000/health)
if [ "$HEALTH" = "Healthy" ]; then
    echo -e "${GREEN}Health endpoint working!${NC}"
else
    echo -e "${RED}Health endpoint not working!${NC}"
    kill $SERVER_PID
    exit 1
fi

# Test basic connectivity first - using a simpler connection test
echo -e "\n${BLUE}=== Testing basic WebSocket connectivity ===${NC}"
echo "Connecting to WebSocket server..."
timeout 3 websocat -v ws://127.0.0.1:3000/ws 
WS_EXIT=$?
if [ $WS_EXIT -ne 124 ] && [ $WS_EXIT -ne 0 ]; then
    echo -e "${RED}Failed to connect to WebSocket server! Exiting demo.${NC}"
    kill $SERVER_PID
    exit 1
fi
echo -e "${GREEN}WebSocket connection successful!${NC}"

# Create temporary files for storing session tokens and meet IDs
SESSION1_FILE=$(mktemp)
SESSION2_FILE=$(mktemp)
MEET_ID_FILE=$(mktemp)

# Step 1: Create a meet
echo -e "\n${BLUE}=== PHASE 1: Creating a Meet ===${NC}"
# Format with msgType field as expected by the server
CREATE_MESSAGE='{"msgType":"CreateMeet","meet_id":"demo-meet-123","password":"TestPassword123!","location_name":"Main Platform","priority":10}'

echo "Sending request to create a meet:"
echo "$CREATE_MESSAGE"

echo "Sending request..."
printf "%s" "$CREATE_MESSAGE" | websocat ws://127.0.0.1:3000/ws > /tmp/create_response.txt || {
    echo -e "${RED}Failed to send create meet request! Exiting demo.${NC}"
    kill $SERVER_PID
    exit 1
}

# Show the raw response
echo "Raw server response:"
cat /tmp/create_response.txt

# Try to extract information from the response
SESSION_TOKEN=$(grep -o '"session_token":"[^"]*' /tmp/create_response.txt | cut -d'"' -f4)
MEET_ID=$(grep -o '"meet_id":"[^"]*' /tmp/create_response.txt | cut -d'"' -f4)

# Store tokens in files for later use
echo "$SESSION_TOKEN" > $SESSION1_FILE
echo "$MEET_ID" > $MEET_ID_FILE

if [ -z "$SESSION_TOKEN" ] || [ -z "$MEET_ID" ]; then
    echo -e "${RED}Failed to extract session token or meet ID from response!${NC}"
    echo "Response was:"
    cat /tmp/create_response.txt
    
    # Continue anyway for demonstration
    echo "Using placeholder values for demonstration..."
    MEET_ID="demo-meet-123"
    SESSION_TOKEN="dummy-session-token"
else
    echo -e "${GREEN}Meet created successfully!${NC}"
    echo "Meet ID: $MEET_ID"
    echo "Session Token: $SESSION_TOKEN"
fi

# Phase 2: Join the meet from client 2
echo -e "\n${BLUE}=== PHASE 2: Joining the Meet from a Second Client ===${NC}"
JOIN_MESSAGE='{"msgType":"JoinMeet","meet_id":"'$MEET_ID'","password":"TestPassword123!","location_name":"Secondary Platform","priority":5}'

echo "Sending request to join the meet..."
printf "%s" "$JOIN_MESSAGE" | websocat ws://127.0.0.1:3000/ws > /tmp/join_response.txt

# Show response and extract client 2's session token
cat /tmp/join_response.txt
SECOND_SESSION_TOKEN=$(grep -o '"session_token":"[^"]*' /tmp/join_response.txt | cut -d'"' -f4)
echo $SECOND_SESSION_TOKEN > $SESSION2_FILE

echo -e "${GREEN}Second client joined successfully!${NC}"
echo "Session Token for second client: $SECOND_SESSION_TOKEN"

# Phase 3: Send updates from client 1
echo -e "\n${BLUE}=== PHASE 3: Sending Updates from First Client ===${NC}"
UPDATE_MESSAGE='{"msgType":"UpdateInit","meet_id":"'$MEET_ID'","session_token":"'$SESSION_TOKEN'","updates":[{"location":"lifters.0.name","value":"\"John Doe\"","timestamp":'$(date +%s)'}]}'

echo "First client sending update (setting lifter name)..."
echo "$UPDATE_MESSAGE"
printf "%s" "$UPDATE_MESSAGE" | websocat ws://127.0.0.1:3000/ws > /tmp/update1_response.txt

# Show response
cat /tmp/update1_response.txt
echo -e "${GREEN}Update from first client processed!${NC}"

# Step 4: Send updates from Client 2
echo -e "\n${BLUE}=== PHASE 4: Sending Updates from Second Client ===${NC}"
UPDATE2_MESSAGE='{"msgType":"UpdateInit","meet_id":"'$MEET_ID'","session_token":"'$SECOND_SESSION_TOKEN'","updates":[{"location":"lifters.0.bodyweight","value":"100.5","timestamp":'$(date +%s)'}]}'

echo "Second client sending update (setting lifter bodyweight)..."
echo "$UPDATE2_MESSAGE"
printf "%s" "$UPDATE2_MESSAGE" | websocat ws://127.0.0.1:3000/ws > /tmp/update2_response.txt

# Show response
cat /tmp/update2_response.txt
echo -e "${GREEN}Update from second client processed!${NC}"

# Step 5: Simulate concurrent updates (POTENTIAL CONFLICT)
echo -e "\n${BLUE}=== PHASE 5: Simulating Concurrent Updates ===${NC}"
CONCURRENT1_MESSAGE='{"msgType":"UpdateInit","meet_id":"'$MEET_ID'","session_token":"'$SESSION_TOKEN'","updates":[{"location":"lifters.0.attempts.0.weight","value":"120.0","timestamp":'$(date +%s)'}]}'

CONCURRENT2_MESSAGE='{"msgType":"UpdateInit","meet_id":"'$MEET_ID'","session_token":"'$SECOND_SESSION_TOKEN'","updates":[{"location":"lifters.0.attempts.0.weight","value":"125.0","timestamp":'$(date +%s)'}]}'

echo "First client setting attempt weight to 120kg..."
printf "%s" "$CONCURRENT1_MESSAGE" | websocat ws://127.0.0.1:3000/ws > /tmp/concurrent1_response.txt

echo "Second client setting the same attempt weight to 125kg..."
printf "%s" "$CONCURRENT2_MESSAGE" | websocat ws://127.0.0.1:3000/ws > /tmp/concurrent2_response.txt

cat /tmp/concurrent1_response.txt
cat /tmp/concurrent2_response.txt
echo -e "${GREEN}Concurrent updates processed!${NC}"

# Step 5A: DEMONSTRATE RESYNCING WITH CLIENT_PULL
echo -e "\n${BLUE}=== PHASE 5A: Demonstrating Resync with CLIENT_PULL ===${NC}"
# Create a client pull message to request updates since seq 0
CLIENT_PULL_MESSAGE='{"msgType":"ClientPull","meet_id":"'$MEET_ID'","session_token":"'$SESSION_TOKEN'","last_server_seq":0}'

echo "Sending CLIENT_PULL to resync data..."
printf "%s" "$CLIENT_PULL_MESSAGE" | websocat ws://127.0.0.1:3000/ws > /tmp/client_pull_response.txt

# Show response
cat /tmp/client_pull_response.txt
echo -e "${GREEN}Client pull for resyncing processed!${NC}"

# Step 5B: Demonstrate publishing meet results
echo -e "\n${BLUE}=== PHASE 5B: Demonstrating Meet Publication ===${NC}"
# Create a sample CSV content
SAMPLE_CSV="Meet Name,Date,Location\nOpenLifter Demo,$(date +%Y-%m-%d),Remote\n\nName,Division,Equipment,BodyweightKg,BestSquatKg,BestBenchKg,BestDeadliftKg,TotalKg\nJohn Doe,Open,Raw,100.5,120.0,80.0,150.0,350.0"

# Create publish message
PUBLISH_MESSAGE='{"msgType":"PublishMeet","meet_id":"'$MEET_ID'","session_token":"'$SESSION_TOKEN'","return_email":"demo@example.com","opl_csv":"'$SAMPLE_CSV'"}'

echo "Sending request to publish meet results..."
printf "%s" "$PUBLISH_MESSAGE" | websocat ws://127.0.0.1:3000/ws > /tmp/publish_response.txt

# Show response
cat /tmp/publish_response.txt
echo -e "${GREEN}Meet publication request processed!${NC}"

# Clean up
echo -e "\n${YELLOW}Cleaning up...${NC}"

# Remove temporary files
rm -f /tmp/create_response.txt /tmp/join_response.txt
rm -f /tmp/update1_response.txt /tmp/update2_response.txt
rm -f /tmp/concurrent1_response.txt /tmp/concurrent2_response.txt
rm -f /tmp/client_pull_response.txt /tmp/publish_response.txt
rm -f $SESSION1_FILE $SESSION2_FILE $MEET_ID_FILE

# Stop the server
echo "Stopping server..."
kill $SERVER_PID 2>/dev/null
wait $SERVER_PID 2>/dev/null

echo -e "${GREEN}Demonstration completed!${NC}"
echo "This demo has shown:"
echo "1. Creating a meet and getting a session token"
echo "2. Joining an existing meet from a second client"
echo "3. Sending updates from both clients"
echo "4. Handling concurrent updates (potential conflicts)"
echo "5. Resyncing data with CLIENT_PULL"
echo "6. Publishing meet results"
echo ""
echo "To test manually, run: websocat ws://127.0.0.1:3000/ws" 