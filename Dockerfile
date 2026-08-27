# syntax=docker/dockerfile:1

# 1. Rust: wasm sim + server binary
FROM rust:1-slim AS rust
RUN rustup target add wasm32-unknown-unknown
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p tick-sim-wasm --target wasm32-unknown-unknown \
 && cargo build --release -p tick-server

# 2. Node: build client (needs the wasm artifact)
FROM node:22-slim AS client
WORKDIR /client
COPY client/package.json client/package-lock.json ./
RUN npm ci
COPY client ./
COPY --from=rust /src/target/wasm32-unknown-unknown/release/tick_sim_wasm.wasm ./public/tick_sim.wasm
RUN npm run build

# 3. Runtime
FROM debian:bookworm-slim
WORKDIR /app
COPY --from=rust /src/target/release/tick-server /app/tick-server
COPY --from=client /client/dist /app/client/dist
ENV TICK_STATIC=/app/client/dist
EXPOSE 8080
CMD ["/app/tick-server"]
