#!/bin/bash

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${YELLOW}WebSocket Server Basic Test${NC}"
echo "============================="

# Kill any existing servers
echo "Cleaning up existing processes..."
killall -q backend-bin 2>/dev/null
sleep 1

# Start the server with debug logging
echo -e "${YELLOW}Starting WebSocket server with debug logging...${NC}"
RUST_LOG=debug,axum=debug,tower_http=debug,backend_lib=trace cargo run -p backend-bin &
SERVER_PID=$!

# Wait for the server to start
echo "Waiting for server to start (10 seconds)..."
sleep 10

# Basic HTTP health check
echo -e "\n${BLUE}=== Testing HTTP health endpoint ===${NC}"
curl -v http://127.0.0.1:3000/health

# Test basic WebSocket connection - only establish connection, don't send any message
echo -e "\n${BLUE}=== Testing basic WebSocket connection establishment ===${NC}"
echo "Attempting to connect using websocat..."

# Connect but don't send a message, just close after 3 seconds
timeout 3 websocat --binary -v ws://127.0.0.1:3000/ws

# Check exit status of the websocat command
WS_EXIT=$?
# 124 means timeout, which is expected
if [ $WS_EXIT -ne 124 ] && [ $WS_EXIT -ne 0 ]; then
    echo -e "${RED}WebSocket connection failed with exit code $WS_EXIT!${NC}"
else
    echo -e "${GREEN}WebSocket connection established successfully!${NC}"
fi

# Stop the server
echo -e "\n${YELLOW}Cleaning up...${NC}"
kill $SERVER_PID 2>/dev/null
wait $SERVER_PID 2>/dev/null

echo -e "${GREEN}Test completed.${NC}" 