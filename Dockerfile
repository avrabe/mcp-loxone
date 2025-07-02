# Multi-stage build for Loxone MCP Rust Server
# This creates a minimal container with just the binary and required runtime dependencies

# Stage 1: Build the application
FROM rust:1.75-alpine AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    pkgconfig \
    ca-certificates

# Create app directory
WORKDIR /app

# Copy manifest files
COPY Cargo.toml Cargo.lock ./

# Create dummy main.rs to cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    echo "fn main() {}" > src/lib.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src ./src
COPY build.rs ./

# Build the actual application
RUN touch src/main.rs src/lib.rs && \
    cargo build --release --bin loxone-mcp-server

# Stage 2: Create minimal runtime image
FROM alpine:3.19

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    libgcc \
    libssl3 \
    tzdata && \
    adduser -D -u 1000 loxone

# Copy binary from builder
COPY --from=builder /app/target/release/loxone-mcp-server /usr/local/bin/

# Create necessary directories
RUN mkdir -p /var/log/loxone-mcp && \
    chown -R loxone:loxone /var/log/loxone-mcp

# Switch to non-root user
USER loxone

# Set working directory
WORKDIR /home/loxone

# Expose ports
# 3001 - HTTP/SSE transport (for n8n and web clients)
EXPOSE 3001

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/loxone-mcp-server", "http", "--port", "3001", "--api-key", "health-check"] || exit 1

# Default command (stdio mode for Claude Desktop)
# Override with "http" for HTTP/SSE mode
ENTRYPOINT ["/usr/local/bin/loxone-mcp-server"]
CMD ["stdio"]

# Environment variables (can be overridden)
ENV RUST_LOG=info,loxone_mcp_rust=info
ENV LOXONE_LOG_FILE=/var/log/loxone-mcp/server.log

# Labels
LABEL org.opencontainers.image.title="Loxone MCP Rust Server"
LABEL org.opencontainers.image.description="Model Context Protocol server for Loxone Generation 1 home automation"
LABEL org.opencontainers.image.vendor="Ralf Anton Beier"
LABEL org.opencontainers.image.licenses="MIT"
LABEL org.opencontainers.image.source="https://github.com/avrabe/mcp-loxone"