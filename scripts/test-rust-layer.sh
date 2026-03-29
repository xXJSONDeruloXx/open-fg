#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

"${ROOT_DIR}/scripts/compile-rust-shaders.sh"
cargo test --locked --manifest-path "${ROOT_DIR}/Cargo.toml"
