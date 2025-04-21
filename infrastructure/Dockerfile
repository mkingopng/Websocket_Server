# ---- Build Stage ----
FROM rust:1.75-slim as builder

# Create app directory
WORKDIR /usr/src/app

# Copy manifests and build dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release && rm -r src

# Copy source and build
COPY . .
RUN cargo build --release

# ---- Runtime Stage ----
FROM debian:bullseye-slim

# Install system dependencies
RUN apt-get update && apt-get install -y libssl1.1 ca-certificates && apt-get clean

# Copy the built binary from builder
COPY --from=builder /usr/src/app/target/release/ws-backend /usr/local/bin/ws-backend

# Set entrypoint
ENTRYPOINT ["ws-backend"]
