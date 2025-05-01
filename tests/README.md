# WebSocket Server Test Suite

This directory contains the test suite for the WebSocket Server application.

## Directory Structure

- `unit/`: Unit tests for individual components
- `integration/`: Integration tests for component interactions
- `e2e/`: End-to-end tests that verify complete system functionality
- `performance/`: Benchmarks and performance tests

## Running Tests

### Unit Tests
```bash
cargo test
```

### End-to-End Tests
```bash
cd tests/e2e
./websocket_test.sh
./network_resilience_test.sh
```

### Performance Tests
Performance tests will be added in future iterations.

## Test Coverage

For detailed test coverage information, see [../documentation/test_coverage.md](../documentation/test_coverage.md).

## Priority Test Areas

Based on current coverage, these components need additional testing:
- Error handling (`error.rs` - 0% coverage)
- Password authentication (`auth/password.rs` - 0% coverage)
- Live handlers (`handlers/live.rs` - 0% coverage)
- Meet operations (`meet.rs` - 0% coverage)
- WebSocket routing (`ws_router.rs` - 4.82% coverage)
- Rate limiting (`middleware/rate_limit.rs` - 7.14% coverage) 