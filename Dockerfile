FROM rustlang/rust:nightly-slim as builder

WORKDIR /usr/src/app

RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev sqlite3 libsqlite3-dev && \
    rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo test && cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y ca-certificates libssl3 libsqlite3-0 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /usr/src/app/target/release/wibot /app/wibot

RUN mkdir -p /app/data

CMD ["./wibot"]