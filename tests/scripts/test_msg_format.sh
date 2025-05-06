#!/bin/bash

echo "Testing WebSocket message format..."

# Create a test message
cat > test_msg.json << EOF
{
  "msgType": "CreateMeet",
  "meet_id": "test-meet-1",
  "password": "TestPassword123!",
  "location_name": "Test Location",
  "priority": 10
}
EOF

echo "Test message content:"
cat test_msg.json

# Send the message
echo "Sending message..."
cat test_msg.json | websocat -v ws://127.0.0.1:3000/ws

echo "Test completed." 