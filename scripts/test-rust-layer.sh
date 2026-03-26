#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

cargo test --locked --manifest-path "${ROOT_DIR}/implementation/vk-layer-rust/Cargo.toml"
