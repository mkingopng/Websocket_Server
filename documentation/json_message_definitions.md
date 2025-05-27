# JSON schema for OpenLifter WebSocket Messages
- All messages are UTF-8 encoded JSON over WebSocket
- All messages must conform to the following schema
- Clarify token rules:
    - Is token optional for some operations and required for others? 
    - Does an empty string count as “unauthenticated”?

## Token Semantics
- The `token` field is optional for all requests.
- If present, it should be a non-empty string.
- An empty or missing token is treated as unauthenticated.
- In the current implementation:
    - `CreateMeet`, `UpdateMeet`, and `JoinMeet` accept a token but do not yet enforce auth.
    - Future versions may validate tokens using a signed scheme.


# ClientMessage (JSON), Sent via WebSocket to server
```json
{
  "type": "CreateMeet" | "JoinMeet" | "UpdateMeet" | "ClientPull",
  "data": {
    // Varies based on type
  },
  "token": "optional-authentication-token-string"
}
```

## CreateMeet
```json
{
  "type": "CreateMeet",
  "data": {
    "meet": {
      "id": "meet-001",
      "name": "State Championships",
      "platforms": [
        {
          "id": "platform-1",
          "name": "Platform A",
          "sessions": []
        }
      ]
    }
  },
  "token": "abc123"
}
```

## JoinMeet
```json
{
  "type": "JoinMeet",
  "data": {
    "meet_id": "meet-001"
  },
  "token": "abc123"
}
```

## UpdateMeet
```json
{
  "type": "UpdateMeet",
  "data": {
    "meet_id": "meet-001",
    "meet": {
      // Full or partial meet object update
    }
  },
  "token": "abc123"
}
```

## ClientPull
```json
{
  "type": "ClientPull",
  "data": {
    "meet_id": "meet-001"
  },
  "token": "abc123"
}
```

# ServerMessage Format (Sent from WebSocket Server to OpenLifter)

ServerMessage (JSON), sent by server to client over WebSocket
```json
{
  "type": "Success" | "Error" | "Pong",
  "data": {
    // Depends on type
  }
}
```

# type specific payloads

## Success (used for all successful operations)
```json
{
  "type": "Success",
  "data": {
    "meet": {
      "id": "meet-001",
      "name": "State Championships",
      "platforms": [
        {
          "id": "platform-1",
          "name": "Platform A",
          "sessions": []
        }
      ]
    }
  }
}
```

## Error
```json
{
  "type": "Error",
  "data": {
    "error": "Missing meet_id in JoinMeet request"
  }
}
```

## Pong (for heartbeat)
```json
{
  "type": "Pong",
  "data": {}
}
```
