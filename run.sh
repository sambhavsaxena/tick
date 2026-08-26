#!/usr/bin/env bash
# Build everything and serve the game on http://localhost:8080
set -euo pipefail
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"

cargo build --release -p tick-sim-wasm --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/tick_sim_wasm.wasm client/public/tick_sim.wasm
npm --prefix client install --silent
npm --prefix client run build
cargo build --release -p tick-server
exec ./target/release/tick-server
