# Dockerfile for COINjecture CPP Network (libp2p removed)
# Multi-stage build for optimized image size

FROM rust:1.88-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy Cargo files for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY adzdb/Cargo.toml ./adzdb/
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
COPY mobile-sdk/Cargo.toml ./mobile-sdk/

# Copy source code
COPY adzdb ./adzdb
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
COPY mobile-sdk ./mobile-sdk

# Build release binary
RUN cargo build --release --bin coinject

# Runtime stage
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

# Expose CPP network ports
# CPP P2P port (default: 707)
EXPOSE 707
# CPP WebSocket RPC port (default: 8080)
EXPOSE 8080
# JSON-RPC port (default: 9933)
EXPOSE 9933
# Metrics port (default: 9090)
EXPOSE 9090

WORKDIR /data

# Default command
ENTRYPOINT ["/usr/local/bin/coinject"]
CMD ["--help"]

