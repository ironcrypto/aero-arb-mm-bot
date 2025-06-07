# Use latest Rust nightly for edition 2024 support
FROM rustlang/rust:nightly-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy dependency files
COPY Cargo.toml Cargo.lock ./

# Create dummy main.rs for dependency caching
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies
RUN cargo build --release && rm -rf src

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash appuser

# Copy the binary
COPY --from=builder /app/target/release/aero-arb-mm-bot /usr/local/bin/aero-arb-mm-bot

# Create necessary directories
RUN mkdir -p /app/output && chown -R appuser:appuser /app

# Switch to non-root user
USER appuser
WORKDIR /app

# Environment variables
ENV RUST_LOG=info \
    NETWORK=mainnet \
    ENABLE_MARKET_MAKING=true \
    ENABLE_TRADE_EXECUTION=false

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD test -f /app/output/logs/aerodrome-bot.log || exit 1

# Run the bot
CMD ["aero-arb-mm-bot"]