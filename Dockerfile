# Multi-stage build for COINjecture blockchain node
FROM rust:1.85-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /build

# Copy Cargo files first for better caching
COPY Cargo.toml Cargo.lock ./
COPY core/Cargo.toml ./core/
COPY consensus/Cargo.toml ./consensus/
COPY network/Cargo.toml ./network/
COPY state/Cargo.toml ./state/
COPY mempool/Cargo.toml ./mempool/
COPY rpc/Cargo.toml ./rpc/
COPY tokenomics/Cargo.toml ./tokenomics/
COPY node/Cargo.toml ./node/
COPY wallet/Cargo.toml ./wallet/
COPY marketplace-export/Cargo.toml ./marketplace-export/
COPY huggingface/Cargo.toml ./huggingface/

# Copy source code
COPY core ./core
COPY consensus ./consensus
COPY network ./network
COPY state ./state
COPY mempool ./mempool
COPY rpc ./rpc
COPY tokenomics ./tokenomics
COPY node ./node
COPY wallet ./wallet
COPY marketplace-export ./marketplace-export
COPY huggingface ./huggingface

# Build release binary
RUN cargo build --release --bin coinject

# Runtime stage - use slim Debian for smaller image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /build/target/release/coinject /usr/local/bin/coinject

# Create data directory
RUN mkdir -p /data

# Expose ports
# P2P port
EXPOSE 30333
# RPC port
EXPOSE 9933

# Set working directory
WORKDIR /data

# Default command - can be overridden
ENTRYPOINT ["/usr/local/bin/coinject"]
CMD ["--help"]
