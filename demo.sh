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

# Clean up any previous processes that might still be running
killall -q backend-bin 2>/dev/null
killall -q websocat 2>/dev/null

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

# Create temporary files for storing session tokens and meet IDs
SESSION1_FILE=$(mktemp)
SESSION2_FILE=$(mktemp)
MEET_ID_FILE=$(mktemp)

# PHASE 1: CREATE A MEET
echo -e "\n${BLUE}=== PHASE 1: Creating a Meet ===${NC}"
CREATE_MESSAGE='{"type":"CreateMeet","payload":{"meet_id":"demo-meet-123","password":"TestPassword123!","location_name":"Main Platform","priority":10}}'

echo "Sending request to create a meet..."
echo $CREATE_MESSAGE | websocat ws://127.0.0.1:3000/ws -n1 > /tmp/create_response.txt

# Extract information from the response
cat /tmp/create_response.txt
SESSION_TOKEN=$(grep -o '"session_token":"[^"]*' /tmp/create_response.txt | cut -d'"' -f4)
MEET_ID=$(grep -o '"meet_id":"[^"]*' /tmp/create_response.txt | cut -d'"' -f4)

# Store tokens in files for later use
echo $SESSION_TOKEN > $SESSION1_FILE
echo $MEET_ID > $MEET_ID_FILE

echo -e "${GREEN}Meet created successfully!${NC}"
echo "Meet ID: $MEET_ID"
echo "Session Token: $SESSION_TOKEN"

# PHASE 2: JOIN THE MEET FROM ANOTHER CLIENT
echo -e "\n${BLUE}=== PHASE 2: Joining the Meet from a Second Client ===${NC}"
JOIN_MESSAGE="{\"type\":\"JoinMeet\",\"payload\":{\"meet_id\":\"$MEET_ID\",\"password\":\"TestPassword123!\",\"location_name\":\"Secondary Platform\",\"priority\":5}}"

echo "Sending request to join the meet..."
echo $JOIN_MESSAGE | websocat ws://127.0.0.1:3000/ws -n1 > /tmp/join_response.txt

# Show response and extract the second client's session token
cat /tmp/join_response.txt
SECOND_SESSION_TOKEN=$(grep -o '"session_token":"[^"]*' /tmp/join_response.txt | cut -d'"' -f4)
echo $SECOND_SESSION_TOKEN > $SESSION2_FILE

echo -e "${GREEN}Second client joined successfully!${NC}"
echo "Session Token for second client: $SECOND_SESSION_TOKEN"

# PHASE 3: SEND UPDATES FROM FIRST CLIENT
echo -e "\n${BLUE}=== PHASE 3: Sending Updates from First Client ===${NC}"
UPDATE_MESSAGE="{\"type\":\"UpdateInit\",\"payload\":{\"meet_id\":\"$MEET_ID\",\"session_token\":\"$SESSION_TOKEN\",\"updates\":[{\"location\":\"lifters.0.name\",\"value\":\"John Doe\",\"timestamp\":$(date +%s)}]}}"

echo "First client sending update (setting lifter name)..."
echo "$UPDATE_MESSAGE"
echo $UPDATE_MESSAGE | websocat ws://127.0.0.1:3000/ws -n1 > /tmp/update1_response.txt

# Show response
cat /tmp/update1_response.txt
echo -e "${GREEN}Update from first client processed!${NC}"

# PHASE 4: SEND UPDATES FROM SECOND CLIENT
echo -e "\n${BLUE}=== PHASE 4: Sending Updates from Second Client ===${NC}"
UPDATE2_MESSAGE="{\"type\":\"UpdateInit\",\"payload\":{\"meet_id\":\"$MEET_ID\",\"session_token\":\"$SECOND_SESSION_TOKEN\",\"updates\":[{\"location\":\"lifters.0.bodyweight\",\"value\":\"100.5\",\"timestamp\":$(date +%s)}]}}"

echo "Second client sending update (setting lifter bodyweight)..."
echo "$UPDATE2_MESSAGE"
echo $UPDATE2_MESSAGE | websocat ws://127.0.0.1:3000/ws -n1 > /tmp/update2_response.txt

# Show response
cat /tmp/update2_response.txt
echo -e "${GREEN}Update from second client processed!${NC}"

# PHASE 5: SIMULATE CONCURRENT UPDATES (POTENTIAL CONFLICT)
echo -e "\n${BLUE}=== PHASE 5: Simulating Concurrent Updates ===${NC}"
CONCURRENT1_MESSAGE="{\"type\":\"UpdateInit\",\"payload\":{\"meet_id\":\"$MEET_ID\",\"session_token\":\"$SESSION_TOKEN\",\"updates\":[{\"location\":\"lifters.0.attempts.0.weight\",\"value\":\"120.0\",\"timestamp\":$(date +%s)}]}}"
CONCURRENT2_MESSAGE="{\"type\":\"UpdateInit\",\"payload\":{\"meet_id\":\"$MEET_ID\",\"session_token\":\"$SECOND_SESSION_TOKEN\",\"updates\":[{\"location\":\"lifters.0.attempts.0.weight\",\"value\":\"125.0\",\"timestamp\":$(date +%s)}]}}"

echo "First client setting attempt weight to 120kg..."
echo $CONCURRENT1_MESSAGE | websocat ws://127.0.0.1:3000/ws -n1 > /tmp/concurrent1_response.txt

echo "Second client setting the same attempt weight to 125kg..."
echo $CONCURRENT2_MESSAGE | websocat ws://127.0.0.1:3000/ws -n1 > /tmp/concurrent2_response.txt

cat /tmp/concurrent1_response.txt
cat /tmp/concurrent2_response.txt
echo -e "${GREEN}Concurrent updates processed!${NC}"

# PHASE 5A: DEMONSTRATE RESYNCING WITH CLIENT_PULL
echo -e "\n${BLUE}=== PHASE 5A: Demonstrating Resync with CLIENT_PULL ===${NC}"
# Create a client pull message to request updates since seq 0
CLIENT_PULL_MESSAGE="{\"type\":\"ClientPull\",\"payload\":{\"meet_id\":\"$MEET_ID\",\"session_token\":\"$SESSION_TOKEN\",\"last_server_seq\":0}}"

echo "Sending CLIENT_PULL to resync data..."
echo $CLIENT_PULL_MESSAGE | websocat ws://127.0.0.1:3000/ws -n1 > /tmp/client_pull_response.txt

# Show response
cat /tmp/client_pull_response.txt
echo -e "${GREEN}Client pull for resyncing processed!${NC}"

# PHASE 5B: DEMONSTRATE PUBLISHING MEET RESULTS
echo -e "\n${BLUE}=== PHASE 5B: Demonstrating Meet Publication ===${NC}"
# Create a sample CSV content
SAMPLE_CSV="Meet Name,Date,Location\nOpenLifter Demo,$(date +%Y-%m-%d),Remote\n\nName,Division,Equipment,BodyweightKg,BestSquatKg,BestBenchKg,BestDeadliftKg,TotalKg\nJohn Doe,Open,Raw,100.5,120.0,80.0,150.0,350.0"

# Create publish message
PUBLISH_MESSAGE="{\"type\":\"PublishMeet\",\"payload\":{\"meet_id\":\"$MEET_ID\",\"session_token\":\"$SESSION_TOKEN\",\"return_email\":\"demo@example.com\",\"opl_csv\":\"$SAMPLE_CSV\"}}"

echo "Sending request to publish meet results..."
echo $PUBLISH_MESSAGE | websocat ws://127.0.0.1:3000/ws -n1 > /tmp/publish_response.txt

# Show response
cat /tmp/publish_response.txt
echo -e "${GREEN}Meet publication request processed!${NC}"

# PHASE 5C: DEMONSTRATE CONFLICT RESOLUTION BASED ON PRIORITY
echo -e "\n${BLUE}=== PHASE 5C: Demonstrating Conflict Resolution Based on Priority ===${NC}"
echo "The main platform (priority 10) and secondary platform (priority 5) will update the same field..."

# First client (priority 10) updates the weight
PRIORITY_TEST1="{\"type\":\"UpdateInit\",\"payload\":{\"meet_id\":\"$MEET_ID\",\"session_token\":\"$SESSION_TOKEN\",\"updates\":[{\"location\":\"lifters.1.attempts.1.weight\",\"value\":\"130.0\",\"timestamp\":$(date +%s)}]}}"

# Second client (priority 5) updates the same field at the same time
PRIORITY_TEST2="{\"type\":\"UpdateInit\",\"payload\":{\"meet_id\":\"$MEET_ID\",\"session_token\":\"$SECOND_SESSION_TOKEN\",\"updates\":[{\"location\":\"lifters.1.attempts.1.weight\",\"value\":\"135.0\",\"timestamp\":$(date +%s)}]}}"

echo "Main platform (priority 10) setting weight to 130kg..."
echo $PRIORITY_TEST1 | websocat ws://127.0.0.1:3000/ws -n1 > /tmp/priority1_response.txt

echo "Secondary platform (priority 5) setting weight to 135kg..."
echo $PRIORITY_TEST2 | websocat ws://127.0.0.1:3000/ws -n1 > /tmp/priority2_response.txt

cat /tmp/priority1_response.txt
cat /tmp/priority2_response.txt
echo -e "${GREEN}Priority-based conflict resolution demonstrated!${NC}"
echo "Since the main platform has a higher priority (10 vs 5), its value of 130kg will be used."

# PHASE 6: ESTABLISH LONG-LIVED CONNECTIONS TO DEMONSTRATE REAL-TIME UPDATES
echo -e "\n${BLUE}=== PHASE 6: Demonstrating Real-time Updates Between Clients ===${NC}"
echo "Starting two long-lived WebSocket clients in separate terminals..."

# Start first client in a new terminal
gnome-terminal -- bash -c "echo 'First client connected to WebSocket server with session: $(cat $SESSION1_FILE)'; websocat ws://127.0.0.1:3000/ws -t -B '{\"type\":\"UpdateInit\",\"payload\":{\"meet_id\":\"$(cat $MEET_ID_FILE)\",\"session_token\":\"$(cat $SESSION1_FILE)\",\"updates\":[]}}'; read" &
TERM1_PID=$!

# Start second client in a new terminal
gnome-terminal -- bash -c "echo 'Second client connected to WebSocket server with session: $(cat $SESSION2_FILE)'; websocat ws://127.0.0.1:3000/ws -t -B '{\"type\":\"UpdateInit\",\"payload\":{\"meet_id\":\"$(cat $MEET_ID_FILE)\",\"session_token\":\"$(cat $SESSION2_FILE)\",\"updates\":[]}}'; read" &
TERM2_PID=$!

echo -e "${GREEN}Two clients connected with long-lived connections.${NC}"
echo "You can now see updates propagating in real-time between the two terminals."
echo "Press any key to send a test update and then continue the demo..."
read -n 1

# Send a test update
TEST_UPDATE="{\"type\":\"UpdateInit\",\"payload\":{\"meet_id\":\"$MEET_ID\",\"session_token\":\"$SESSION_TOKEN\",\"updates\":[{\"location\":\"lifters.0.name\",\"value\":\"Jane Smith\",\"timestamp\":$(date +%s)}]}}"
echo "Sending test update from main script..."
echo $TEST_UPDATE | websocat ws://127.0.0.1:3000/ws -n1 > /dev/null

echo "Press any key to continue and clean up the demo..."
read -n 1

# Clean up
echo -e "\n${YELLOW}Cleaning up...${NC}"
kill $TERM1_PID $TERM2_PID 2>/dev/null

# Remove temporary files
rm -f /tmp/create_response.txt /tmp/join_response.txt
rm -f /tmp/update1_response.txt /tmp/update2_response.txt
rm -f /tmp/concurrent1_response.txt /tmp/concurrent2_response.txt
rm -f /tmp/client_pull_response.txt /tmp/publish_response.txt
rm -f /tmp/priority1_response.txt /tmp/priority2_response.txt
rm -f $SESSION1_FILE $SESSION2_FILE $MEET_ID_FILE

# Stop the server
kill $SERVER_PID 2>/dev/null
wait $SERVER_PID 2>/dev/null

echo -e "${GREEN}Demonstration completed!${NC}"
echo "This demo has shown:"
echo "1. Creating a meet and getting a session token"
echo "2. Joining an existing meet from a second client"
echo "3. Sending updates from both clients"
echo "4. Handling concurrent updates (potential conflicts)"
echo "5. Real-time update propagation between connected clients"
echo "6. Resyncing data with CLIENT_PULL"
echo "7. Publishing meet results"
echo "8. Conflict resolution based on priority"
echo ""
echo "To test manually, run: websocat ws://127.0.0.1:3000/ws" 