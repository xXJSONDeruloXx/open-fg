#!/usr/bin/env bash
set -euo pipefail

: "${OMFG_LAYER_IMPL:=rust}"

if [[ "${OMFG_LAYER_IMPL}" != "rust" ]]; then
  echo "Unsupported OMFG_LAYER_IMPL=${OMFG_LAYER_IMPL}. This repository now supports only: rust" >&2
  exit 1
fi

export OMFG_LAYER_IMPL="rust"
export OMFG_LAYER_BUILD_SUBDIR="vk-layer-rust"
export OMFG_LAYER_NAME="VK_LAYER_OMFG_rust"
export OMFG_LAYER_ENABLE_ENV="ENABLE_OMFG_RUST"
export OMFG_LAYER_DISABLE_ENV="DISABLE_OMFG_RUST"
export OMFG_LAYER_LIB_BASENAME="libVkLayer_OMFG_rust.so"
export OMFG_LAYER_MANIFEST_BASENAME="VkLayer_OMFG_rust.json"
export OMFG_LAYER_REMOTE_BASE_DEFAULT="/home/deck/post-proc-fg-research/deploy/vk-layer-rust"
export OMFG_LAYER_ARTIFACT_ROOT_REL="artifacts/steamdeck/rust"
export OMFG_LAYER_SOURCE_DIR="."
export OMFG_LAYER_BUILD_SYSTEM="cargo"
