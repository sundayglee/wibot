FROM rustlang/rust:nightly-slim as builder

# Accept build arguments
ARG VERSION
ARG BUILD_TIMESTAMP
ARG GITHUB_SHA

# Set build-time variables that will bust cache
ENV BUILD_VERSION=${VERSION}
ENV BUILD_TIMESTAMP=${BUILD_TIMESTAMP}
ENV BUILD_COMMIT=${GITHUB_SHA}

WORKDIR /usr/src/app

# Install dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev sqlite3 libsqlite3-dev && \
    rm -rf /var/lib/apt/lists/*

# Copy manifests first for better caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && \
    echo "fn main() {println!(\"dummy\")}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Now copy the actual source code
COPY src ./src

# Touch main.rs to ensure it's newer than the cached deps and rebuild with build info
RUN touch src/main.rs && \
    cargo test && \
    cargo build --release

# Final stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates libssl3 libsqlite3-0 && \
    rm -rf /var/lib/apt/lists/*

# Copy the build artifact and prepare data directory
COPY --from=builder /usr/src/app/target/release/wibot /app/wibot
RUN mkdir -p /app/data

CMD ["./wibot"]