# WebSocket Server Test Coverage

This document tracks the current test coverage of the WebSocket Server API and outlines areas that need additional testing.

## Test Directory Structure
We've created a new test directory structure to better organize tests:
```
tests/
├── e2e/               # End-to-end tests (shell scripts)
│   ├── websocket_test.sh
│   └── network_resilience_test.sh
├── integration/       # Integration tests
├── performance/       # Performance tests
├── unit/              # Unit tests
└── README.md          # Documentation
```
**Note:** The test structure is set up, but more work is needed to properly integrate it with the build system. 
Currently, unit tests still exist in their respective modules, and the new test files are skeletons that need to be adapted to the actual API.

## Current Coverage (as of latest tarpaulin report)
Overall coverage: **51.79%** (837/1616 lines covered)

### Coverage by File

| File                         | Coverage | Lines Covered | Total Lines |
|------------------------------|----------|---------------|-------------|
| `auth/token_generator.rs`    | 100.00%  | 6             | 6           |
| `auth/password.rs`           | 48.00%   | 12            | 25          |
| `auth/persistent_session.rs` | 68.48%   | 113           | 165         |
| `auth/rate_limit.rs`         | 71.11%   | 32            | 45          |
| `auth/service_impl.rs`       | 55.00%   | 11            | 20          |
| `auth/session.rs`            | 76.57%   | 134           | 175         |
| `config.rs`                  | 46.15%   | 12            | 26          |
| `error.rs`                   | 62.50%   | 30            | 48          |
| `handlers/live.rs`           | 25.32%   | 20            | 79          |
| `lib.rs`                     | 80.00%   | 20            | 25          |
| `meet.rs`                    | 0.00%    | 0             | 19          |
| `meet_actor.rs`              | 64.68%   | 130           | 201         |
| `messages.rs`                | 0.00%    | 0             | 2           |
| `middleware/rate_limit.rs`   | 7.14%    | 2             | 28          |
| `storage.rs`                 | 76.81%   | 53            | 69          |
| `validation/mod.rs`          | 71.79%   | 84            | 117         |
| `websocket.rs`               | 42.74%   | 203           | 475         |
| `ws_router.rs`               | 71.64%   | 48            | 67          |
| `main.rs`                    | 0.00%    | 0             | 23          |

## Unit Tests
- [x] Validation module tests (71.79% coverage)
- [x] Auth service implementation tests (55.00% coverage)
- [x] WebSocket handler tests (42.74% coverage) - Significantly improved
- [x] WebSocket router tests (71.64% coverage) - Major improvement, now passing
  - [x] Test malformed message handling
  - [x] Test session validation
  - [x] Test router creation
  - [x] Test handler process message
  - [x] Test error serialization
  - [x] Test validation errors
  - [x] Test message handling workflow
  - [x] Test router with middleware and logging
  - [ ] Test disconnection scenarios
  - [ ] Test more complex error handling
- [x] Error handling tests (62.50% coverage)
  - [x] Test error creation for each error type
  - [x] Test error serialization
  - [x] Test error responses
- [x] Password authentication tests (48.00% coverage)
  - [x] Test password validation
  - [x] Test password hashing
  - [x] Test password verification
- [x] Session management tests (76.57% coverage)
  - [x] Test session creation
  - [x] Test session validation
  - [x] Test session expiration
  - [x] Test CSRF token validation
  - [x] Test session rotation
- [x] Meet operations tests (Implemented but coverage pending)
  - [x] Test meet creation
  - [x] Test meet joining
  - [x] Test meet publication
- [x] Config loading tests (46.15% coverage)
  - [x] Test environment variable loading
  - [x] Test config file loading
  - [x] Test default values
  - [ ] Test config file loading from custom paths
  - [ ] Test environment variable overrides
- [x] Storage tests (76.81% coverage)
  - [x] Test basic file operations
  - [ ] Test concurrent file operations
  - [ ] Test error handling during file operations
  - [ ] Test edge cases like file system full
- [x] Rate limiting tests (71.11% coverage)
  - [x] Test rate limit enforcement
  - [x] Test rate limit window sliding
  - [x] Test rate limit bypass for certain operations
- [x] Middleware tests (7.14% coverage)
  - [x] Test basic router

## Integration Tests
- [x] Basic WebSocket flow tests (connection, messaging, disconnection)
- [x] Conflict resolution tests
- [x] Reconnection and retry logic tests
- [x] Authentication flow tests
  - [x] Test session creation and validation
  - [ ] Test login flow
  - [ ] Test session refreshing
- [ ] Client broadcast tests with multiple concurrent clients
- [ ] API endpoint integration tests
  - [ ] Test `/ws` endpoint
  - [ ] Test health check endpoints
  - [ ] Test admin endpoints
- [ ] Error handling integration tests
  - [ ] Test invalid message handling
  - [ ] Test connection interruption handling
  - [ ] Test server-side errors
- [ ] Storage integration tests
  - [ ] Test data persistence across server restarts
  - [ ] Test data migration
  - [ ] Test backup/restore functionality

## End-to-End Tests
- [x] Basic WebSocket communication script (`tests/e2e/websocket_test.sh`)
- [x] Network resilience test script (`tests/e2e/network_resilience_test.sh`)
- [ ] Load testing
  - [ ] Test with multiple concurrent clients (10, 100, 1000)
  - [ ] Test message throughput
  - [ ] Test under heavy load
- [ ] Real-world scenario tests
  - [ ] Test complete competition flow
  - [ ] Test multi-device synchronization
  - [ ] Test long-running sessions
- [ ] Security testing
  - [ ] Test authentication bypass attempts
  - [ ] Test authorization checks
  - [ ] Test input validation
  - [ ] Test rate limiting effectiveness
- [ ] Cross-platform testing
  - [ ] Test with different browsers
  - [ ] Test with mobile clients
  - [ ] Test with desktop clients

## Performance Benchmarks
- [x] Message throughput benchmarks (skeleton implemented)
  - [ ] Measure messages per second
  - [ ] Measure latency
  - [ ] Measure under different loads
- [ ] Connection handling capacity
  - [ ] Measure max connections
  - [ ] Measure connection establishment time
  - [ ] Measure memory usage per connection
- [ ] Storage performance
  - [ ] Measure read/write speeds
  - [ ] Measure query performance
  - [ ] Measure under concurrent access

## Test Improvement Notes
- The WebSocket handler has seen significant improvement in test coverage, now at 42.74% (up from 37.05%).
- WebSocket router tests have dramatically improved from 5.97% to 71.64% after fixing session directory issues.
- **All 58 tests in backend-lib crate and all 33 tests in tests crate are now passing successfully.**
- Added tests for handlers/live.rs, increasing coverage from 0% to 25.32%.
- We've addressed the root causes of testing failures:
  - **Session Directory Structure**: Most test failures were caused by missing session directories
  - **Version Compatibility**: Some failures were due to incompatibility with the current axum API
  - **Configuration Issues**: Tests were using default settings rather than test-specific configurations
- The overall test coverage has increased from 50.71% to 51.79%.

## Remaining Focus Areas
1. Continue improving coverage for `handlers/live.rs` (currently at 25.32%)
2. Add tests for `meet.rs` (currently at 0% coverage)
3. Improve middleware test coverage (currently at 7.14%)
4. Add tests for `messages.rs` (currently at 0% coverage)

## Next Steps
- Add more test cases for handlers/live.rs
- Create test utilities for common test setup code
- Implement tests for meet.rs
- Continue building out unit tests
- Add integration tests for middleware components
- Set up pre-commit hooks to run tests automatically
- Ensure test environments properly initialize session directories

## Action Items
- Continue building out unit tests for files with low or zero coverage
- Focus next on handlers/live.rs tests
- Create a test utilities module to share common test setup logic
- Set up pre-commit hooks to run tests automatically
- Ensure test environments properly initialize session directories

## Development Notes
When writing tests, remember:
- Fix one test at a time
- Deal with the root cause of the test failure, don't work around the problem
- We have a lot of tests filtered out. We need to deal with that
- Create a proper test environment with temporary directories
- Always ensure the sessions directory exists in test environment
- Configure test-specific Settings objects 
- Don't use workarounds - address root causes of test failures
- Test session handling is critical for most components