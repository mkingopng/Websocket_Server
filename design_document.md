# Design Specification

## Rationale
- Lots of meet directors like OpenLifter but are frustrated by its limitations associated with its client-only operation
    - Awkward for livestreaming
    - Awkward for running registrations and weigh-ins from multiple locations
    - No auto submission of results to OPL
        - Also adds difficulty on the OPL end

## Features
- Multiple browsers running the same meet simultaneously
    - Multiple updating browsers
    - Multiple display-only browsers
- Browsers can make meets live or join already live meets
    - They can do this from within OpenLifter
    - Auth via meet ID and password shared
- Updating browsers can continue to operate without contact to server
    - Conflicting updates while offline are resolved by server
- Server can recover lost state from clients
- Livestream overlay

## Limitations
- No public scoreboard
- No public lifter self-registration
- No monetisation

## Auth
- When making meet live
    - Browser sets password
        - Min 10 characters
    - Browser supplies list of endpoints with their conflict resolution priority
    - Server responds with random live meet ID not already used in current or finished meets dir
        - 9 digits, presented in groups of 3
- Password salted, hashed with scrypt on server
- When logging into existing meet
    - Supply meet ID, location name, and password
- Authenticated clients issued crypto-random session tokens expiring in 7? Days
    - Server authenticates client comms via these session tokens

## Technology recommendations

### Network transport
- Websocket
- Messages serde with JSON

### Backend data storage
- Flat files
    - {current-meets,finished-meets}/[meetID]/{updates.json.log,auth.json}
    - diag.log

### Language / framework
- Rust
    - Tokio-tungstenite
    - serde_json

## Integration with OpenLifter

### UI
- A navigational tab labelled something like “Live (Advanced)”
- Links to a component with all live-related parameters and forms
    - Go live form
        - TODO
    - Display live params
        - TODO

### Code
- TODO
    - Is there a react websocket component that helps somehow?
        - Yes - see email from otf
    - Update local pending log and send via websocket from reducers
        - May involve slight refactoring
    - Invoke reducers from incoming websocket handler

## Client/Server Protocol

### General schema

```json
{
	“msgType”: “MESSAGE_TYPE”,
	…
}
```

### Meet creation
```rust
CREATE_MEET, client->server
{
“thisLocationName”: “foo”,
“password”: “AStrongerPasswordThanThisHopefully”,
[ {“locationName”: “foo”, “priority”: n},...]
}
```

```rust
MEET_CREATED, server->client
{
“meetId”: “nnnnnn”,
“sessionToken”: “AAAAAAAAAAAAAaaaaaaaaaAAAAAA”
}
```

### Joining
```rust
JOIN_MEET, client->server
{
	“meetId”: “nnnnnn”,
	“Password”: “AStrongerPasswordThanThisHopefully”,
	“locationName”: “foo”,
}
```

```rust
MEET_JOINED, server->client
{
	“sessionToken”: “BBBBBBBBBBBBBbbbbbbbbbbbbbBBBB”
}
```

```rust
JOIN_REJECTED, server->client
{
	“reason”: “invalid meetId/Password” // (or invalid locationName)
}
```

### Update

```rust
UPDATE_INIT, client->server
	{
		“sessionToken”: “asdfsapdfojwpeofijwpeojfiwe”,
		“updates”: [
			{
			“updateKey”: “fooState.property.array[n]”,
			“updateValue”: “fooValue”,
			“localSeqNum”: n,
			“afterServerSeqNum”: m,
		},
		…,
  ]
}
```

```rust
UPDATE_ACK, server->client
	{
		“updateAcks: [
			{
“localSeqNum”: n,
			“serverSeqNum”: x,
		},
		…,
		],
	}
```

```rust
UPDATE_REJECTED, server->client
	{
		“updatesRejected: [
			{
		“localSeqNum”: n,
		“conflict”: true,
		“reason”: “blah blah”,
},
…,
		],
}
```

```rust
UPDATE_RELAY, server->client
	{
		“updatesRelayed”: [
			{
		“updateKey”: “fooState.property.array[n]”,
		“updateValue”: “fooValue”,
		“serverSeqNum”: x,
},
…,
],
	}
```

### Publish
```rust
PUBLISH_MEET, client->server
{
		“sessionToken”: “asdfsapdfojwpeofijwpeojfiwe”,
	“returnEmail”: “meetdirector@example.com”,
	“oplCsv”: “[opl.csv export file content]”
}
```

```rust
PUBLISH_ACK, server->client
```	

### Resync
```rust
CLIENT_PULL, client->server
{
	“sessionToken”: “asdfsapdfojwpeofijwpeojfiwe”,
	“lastServerSeq”: x,
}
```

```rust
SERVER_PULL, server->client
	{
		“lastServerSeq”: x,
}
```

### General error messages
```rust
MALFORMED_MESSAGE
{
	“errMsg”: “[message from JSON parser]”,
}
```

```rust
UNKNOWN_MESSAGE_TYPE
	{
		“msgType”: “[MESSAGE_TYPE]”,
}
```

```rust
INVALID_SESSION, server->client
{
“sessionToken”: “asdfsapdfojwpeofijwpeojfiwe”,
} 
```

## Sketchy ramblings

### Features

- Either create new or join existing live meet
    - Initiator sets priority levels for locations
        - If conflict, prefer higher prio
            - Conflicts should be rare - this is not only where one location modifies data set by another, but where event ordering is ambiguous due to loss of sync with backend as well
    - Initiator can finalise results and then request backend publish to GL
        - Or any client?
        - Client may as well render csv and send to backend
            - No point duplicating results code
- Full state recovery in either direction if client or server goes down
- In Live tab, link to overlay page to open in new browser tab
    - Initially not configurable

### Auth
- Initiator sets password as part of live setup
    - Min 10 char
    - Salted and hashed
        - Scrypt with appropriate parameters
- Backend creates random meet ID
    - Something easily enterable
        - 6 digits?
- Backend responds with random session ID
    - Expires 7 days?
- Subsequent clients join with meet ID and password
    - Backend responds with random session ID

## Backend storage
- Could probably almost go without, but this means backend and writer clients can both go down and restore state
- Entire state is tens of KB per meet
    - Probably could just rewrite entire state to flat file per meet on each state update
        - Or append a journal?
            - Backend assigns sequence numbers in order it receives events and includes them when sending to other clients
                - This is safe if server is single threaded async
            - When meet is finalised, we could render CSV on backend and attach to issue
                - Or let client do it since it already does that
    - Implementing some kind of more granular k:v store or other random access system would probably cost more in complication than saving in performance
        - Unless we were to operate on a massively higher scale with thousands of meets simultaneously
    - Or, if we are tracking and replaying events, maybe an event bus, journal, queue service, etc?
        - Something that allows reconsumption of events from t onwards?
            - Rabbit? Redis? Celery?  DIY?
                - should be a simple data structure, DIY probably better
- Each meet gets a dir named with meet ID
    - Updates log
    - Auth file
        - Password hash
    - Sessions file
        - Client ID, key, expiry time
    - When meet finalised, meet dir moves to another dir for finalised meets
        - Cron job clears it after n days?

## Language / framework
- TypeScript/Node
    - TS used in OpenLifter
- Rust
    - Rocket
        - Web, has ws support, async support
    - Used in OPL backend
    - Sean prefers Rust
    - tokio-tungstenite
        - WS only
        - https://crates.io/crates/tokio-tungstenite
    - Loosely couple from specific transport framework

## Network transport
- Websocket
    - Preferred as full support for bidirectional comms
- SSE over HTTP 2
    - Clients can’t send, would have to use separate HTTP requests

## Normal, fault-free scenarios

### One updating client A, one read-only client B
- Client A sends update with local seq and last seen server seq
- Server applies updates, allocating server seqs
- Server sends updates to client B
- Client B applies updates and updates its local canonical log
- Server sends local seq->server seq mappings to client A
- Client A moves local pending updates into local canonical log, with seq mappings applied

### Two updating clients A and B, overlapping temporally but not overlapping state values
- Client A sends update with local seq 1..m and last seen server seq s
- Client B sends update with local seq 1..n and last seen server seq s
- Server applies updates from client A, assigning server seq s+1..s+m
- Server sends local seq->server seq mappings to client A
- Client A moves updates from pending log to its canonical log, with server seq numbers
- Server sends updates that came from client A, to client B
- Client B applies updates from client A, updating its canonical log
- Server applies updates from client B, assigning server seq s+m+1..s+m+n
- Server sends local seq->server seq mappings to client B
- Client B moves updates from pending log to its canonical log, with server seq numbers
- Server sends updates that can from client B, to client A
- Client A applies updates from client B, updating its canonical log

## What is a conflict?
- Multiple clients each send an update to the same object
- The value of the update is not the same across all clients
- At least one client’s update’s last seen server sequence number is before that of at least one other client’s update of that same value ie : this updating client did not see the other client’s update beforehand
- Conflicts should be rare
    - In most cases with multiple updating clients, they will be registering independent sets of lifters, updating weigh in data for independent sets of lifters, updating lifting data on different platforms etc
        - Even if there are overlapping updates, there still has to be a race condition
            - Unlikely to happen without latency/reachability problems as well
    - We should keep the algo simple and not be tied up in 2PCs etc even if it means more traffic than optimal in order to resolve
    - Resolution
        - If priority of updating client allows it to overrule client that provided the update currently in the event list, overrule it
            - This implies we need to store the priority of the client that provided each update
        - Push updates from earliest conflict seq onward to all connected clients
            - All clients receiving overwrite their own local logs in the course of applying the updates to their local states

### Fault tolerance / conflict resolution scenarios
- Consider using FSM pattern - state transitions

### Essentially read-only client unable to reach server
- Client shows pop-up notification indicating out of sync
- Client sets {need_resync} flag
- When able to reach server again, client send “pull events after {seq}”
    - This implies clients need to be told by server how to map their updates to server’s sequence numbers
        - Always assign local sequence numbers
        - Server can respond with the mapping
            - Or is it better/simpler to simply send the events as the server has them to all clients, including the originator?
                - No, providing the mapping allows the originator to easily remove from the local pending update log after copying into the local canonical log
- Server sends all events where {seq} > {after_seq}
- Client clears {need_resync}
- Client applies updates in order
- Client clears pop-up

### Updating client unable to reach server, no conflicting updates
- Client shows pop-up notification indicating out of sync
- Client sets {need_resync} flag
- Client continues applying updates locally and assigning local sequence numbers
- When able to reach server again,
- Client sends locally buffered updates with local sequence numbers
- Server applies updates
    - Conflict resolution would happen here
- Server sends local:global sequence number mappings
- client send “pull events after {seq}”
    - Do we want to do this yet?  We don’t know about potential conflicts yet
- Server sends all events where {seq} > {after_seq}
    - Including the ones the client sent
- Client clears {need_resync}
- Client applies updates from server in order, starting from {after_seq+1}
    - Overwrites any local-only entries
    - This assumes that none of our updates are read-and-updates, otherwise we would have to actually undo our local only entries to get back to the correct state
        - Eg: increments, decrements, appends, etc.

### Two updating clients A, B unable to reach server, one client C still able to reach, mutually conflicting updates
- Clients A and B show pop-up notification indicating out of sync
- Clients A and B set {need_resync} flag
- Clients A and B continue applying updates locally and assigning sequence numbers
- Client C continues sending updates to server
- When Client A is able to reach server again, it sends locally buffered updates with local sequence numbers
- Server applies updates from Client C
- Server pushes new updates relative to last seq it saw, to client A
- Server pushes local->global seq mappings to Client C
- Server applies updates from Client A
    - Conflict!
- Server resolves conflict
- Server pushes resolved conflicted updates to Client A
- Client A applies updates from server
- Server pushes resolved conflicted updates to Client C
- Client C applies updates from server
- When Client B is able to reach server again, it sends locally buffered updates with local sequence numbers
- Server applies updates from Client B
    - Conflict!
- Server resolves conflict
- Server pushes resolved conflicted updates to Clients A, B, and C
- Clients A, B, and C apply updates from server


### Two updating clients unable to reach server, one client A reaches server significantly later than the other client B, conflicting updates
- TODO - is this still a special case though, if we aren’t waiting for other clients before applying updates?

### Two updating clients A and B unable to reach server, when they reach server, server has lost all state
- When clients reach server again, they send a request to pull missing updates after last seen server seq num
- If server hasn’t seen that seq num, something is wrong
- Server requests to pull all updates from each client in descending priority order
- Clients respond with updates
- Server applies updates, resolves conflicts, and sends updates out

------