# auth/

Implements authentication, session management, and rate limiting for the backend server.

## Key Files
- `session.rs`: Session creation, validation, and rotation logic
- `persistent_session.rs`: Persistent session storage and management
- `rate_limit.rs`: Rate limiting logic for client requests
- `password.rs`: Password hashing and verification
- `service.rs`/`service_impl.rs`: Auth service traits and implementations
- `token_generator.rs`: Secure token generation utilities
- `mod.rs`: Module declarations 