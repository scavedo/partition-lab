#!/usr/bin/env bash
# Build the site locally into www/, then serve it.
# Needs: rustup target add wasm32-unknown-unknown
#        cargo install wasm-bindgen-cli --version 0.2.130 --locked
set -euo pipefail
cargo test
cargo build --release --target wasm32-unknown-unknown --lib
wasm-bindgen --target web --no-typescript --out-dir www/pkg \
  target/wasm32-unknown-unknown/release/partition_lab.wasm
echo "Built. Serve it with:  python3 -m http.server -d www 8000"
