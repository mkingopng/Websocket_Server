# Build stage
FROM rust:1.75 as builder

ARG RUST_VERSION=1.75

# Create a new empty shell project
WORKDIR /usr/src/openlifter
COPY . .

# Build for release
RUN cargo build --release

# Run stage
FROM debian:bookworm-slim

# Install OpenSSL and CA certificates
RUN apt-get update && apt-get install -y \
    openssl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy the binary and config
COPY --from=builder /usr/src/openlifter/target/release/openlifter-backend-bin /app/openlifter-backend
COPY config.toml /app/config.toml

# Create data directory
RUN mkdir -p /app/data

# Set environment variables
ENV RUST_LOG=info
ENV OPENLIFTER_CONFIG=/app/config.toml

# Expose ports
EXPOSE 3000
EXPOSE 9091

# Run the binary
CMD ["/app/openlifter-backend"] 