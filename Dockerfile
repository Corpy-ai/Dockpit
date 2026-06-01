# Docker Manager v3.0 Container Image
# Multi-stage build for minimal final image

# Build stage
FROM rust:1.89-alpine AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev \
    openssl-libs-static

# Create app user
RUN addgroup -g 1000 dockermgr && \
    adduser -D -s /bin/sh -u 1000 -G dockermgr dockermgr

# Set working directory
WORKDIR /app

# Copy dependency files first (for better Docker layer caching)
COPY Cargo.toml Cargo.lock ./

# Create dummy main.rs to build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source code
COPY src ./src

# Build application
RUN cargo build --release --target x86_64-unknown-linux-musl

# Strip binary for smaller size
RUN strip target/x86_64-unknown-linux-musl/release/docker-manager

# Runtime stage - minimal Alpine image
FROM alpine:3.19 AS runtime

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    docker-cli \
    ncurses \
    && rm -rf /var/cache/apk/*

# Create app user
RUN addgroup -g 1000 dockermgr && \
    adduser -D -s /bin/sh -u 1000 -G dockermgr dockermgr

# Copy binary from builder
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/docker-manager /usr/local/bin/docker-manager

# Set ownership and permissions
RUN chown dockermgr:dockermgr /usr/local/bin/docker-manager && \
    chmod +x /usr/local/bin/docker-manager

# Switch to non-root user
USER dockermgr

# Set environment variables
ENV TERM=xterm-256color
ENV DOCKER_HOST=unix:///var/run/docker.sock

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD docker version >/dev/null || exit 1

# Default command
ENTRYPOINT ["/usr/local/bin/docker-manager"]
CMD []

# Metadata
LABEL org.opencontainers.image.title="Docker Manager" \
      org.opencontainers.image.description="Fast and efficient Docker container management TUI" \
      org.opencontainers.image.version="3.0.0" \
      org.opencontainers.image.source="https://github.com/unicommerce/docker-manager" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.authors="uniCommerce Team"