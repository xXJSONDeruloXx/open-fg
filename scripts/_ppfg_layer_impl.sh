#!/usr/bin/env bash
set -euo pipefail

: "${PPFG_LAYER_IMPL:=mvp}"

case "${PPFG_LAYER_IMPL}" in
  mvp|cpp)
    export PPFG_LAYER_IMPL="mvp"
    export PPFG_LAYER_BUILD_SUBDIR="vk-layer-mvp"
    export PPFG_LAYER_NAME="VK_LAYER_PPFG_mvp"
    export PPFG_LAYER_ENABLE_ENV="ENABLE_PPFG_MVP"
    export PPFG_LAYER_DISABLE_ENV="DISABLE_PPFG_MVP"
    export PPFG_LAYER_LIB_BASENAME="libVkLayer_PPFG_mvp.so"
    export PPFG_LAYER_MANIFEST_BASENAME="VkLayer_PPFG_mvp.json"
    export PPFG_LAYER_REMOTE_BASE_DEFAULT="/home/deck/post-proc-fg-research/deploy/vk-layer-mvp"
    export PPFG_LAYER_ARTIFACT_ROOT_REL="artifacts/steamdeck"
    export PPFG_LAYER_SOURCE_DIR="implementation/vk-layer-mvp"
    export PPFG_LAYER_BUILD_SYSTEM="cmake"
    ;;
  rust)
    export PPFG_LAYER_IMPL="rust"
    export PPFG_LAYER_BUILD_SUBDIR="vk-layer-rust"
    export PPFG_LAYER_NAME="VK_LAYER_PPFG_rust"
    export PPFG_LAYER_ENABLE_ENV="ENABLE_PPFG_RUST"
    export PPFG_LAYER_DISABLE_ENV="DISABLE_PPFG_RUST"
    export PPFG_LAYER_LIB_BASENAME="libVkLayer_PPFG_rust.so"
    export PPFG_LAYER_MANIFEST_BASENAME="VkLayer_PPFG_rust.json"
    export PPFG_LAYER_REMOTE_BASE_DEFAULT="/home/deck/post-proc-fg-research/deploy/vk-layer-rust"
    export PPFG_LAYER_ARTIFACT_ROOT_REL="artifacts/steamdeck/rust"
    export PPFG_LAYER_SOURCE_DIR="implementation/vk-layer-rust"
    export PPFG_LAYER_BUILD_SYSTEM="cargo"
    ;;
  *)
    echo "Unsupported PPFG_LAYER_IMPL=${PPFG_LAYER_IMPL}. Expected one of: mvp, rust" >&2
    exit 1
    ;;
esac
