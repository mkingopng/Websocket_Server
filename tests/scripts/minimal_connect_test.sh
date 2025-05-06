#!/bin/bash

# Basic colors
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo "=== WebSocket Server Connection Test ==="

# Kill any existing servers
killall -q backend-bin 2>/dev/null
sleep 1

# Start the server 
echo "Starting WebSocket server..."
cargo run -p backend-bin &
SERVER_PID=$!

# Wait for the server to start
echo "Waiting for server to start (5 seconds)..."
sleep 5

# Check the health endpoint
echo "Testing health endpoint..."
HEALTH_RESPONSE=$(curl -s http://127.0.0.1:3000/health)
if [ "$HEALTH_RESPONSE" == "Healthy" ]; then
    echo -e "${GREEN}Health endpoint working!${NC}"
else
    echo -e "${RED}Health endpoint not working${NC}"
    kill $SERVER_PID 2>/dev/null
    exit 1
fi

# Test basic WebSocket connection establishment
echo "Testing WebSocket connection (3 seconds)..."
timeout 3 websocat ws://127.0.0.1:3000/ws
WEBSOCKET_EXIT=$?

# 124 is the timeout exit code, which is expected
if [ $WEBSOCKET_EXIT -eq 124 ] || [ $WEBSOCKET_EXIT -eq 0 ]; then
    echo -e "${GREEN}WebSocket connection successful!${NC}"
else
    echo -e "${RED}WebSocket connection failed with code $WEBSOCKET_EXIT${NC}"
fi

# Stop the server
echo "Cleaning up..."
kill $SERVER_PID 2>/dev/null
wait $SERVER_PID 2>/dev/null

echo "Test complete." 