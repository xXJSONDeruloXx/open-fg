#!/usr/bin/env bash
set -euo pipefail

# Canonical OMFG Steam Deck wrapper.
#
# Purpose:
#   Export the Vulkan-layer environment OMFG needs, then exec the original
#   Steam/Proton launch command unchanged.
#
# Typical usage on Deck:
#   cp scripts/steamdeck-omfg-wrapper.sh /home/deck/omfg.sh
#   chmod +x /home/deck/omfg.sh
#
# Optional:
#   export OMFG_WRAPPER_ENV_FILE=/home/deck/omfg.env
#   # put OMFG_* exports in that file and this wrapper will source it first

if [[ -n "${OMFG_WRAPPER_ENV_FILE:-}" && -f "${OMFG_WRAPPER_ENV_FILE}" ]]; then
  # shellcheck disable=SC1090
  source "${OMFG_WRAPPER_ENV_FILE}"
fi

BASE="${OMFG_BASE_DIR:-/home/deck/post-proc-fg-research}"
LAYER_IMPL="${OMFG_LAYER_IMPL:-rust}"
LAYER_DIR="${OMFG_LAYER_DIR:-${BASE}/deploy/vk-layer-${LAYER_IMPL}}"
LAYER_NAME="${OMFG_LAYER_NAME:-VK_LAYER_OMFG_${LAYER_IMPL}}"
LOG_DIR="${OMFG_LOG_DIR:-${BASE}/logs}"
DISABLE_LAYER="${OMFG_DISABLE_LAYER:-0}"
DISABLE_LAYER_IMPL_ENV="DISABLE_OMFG_${LAYER_IMPL^^}"

if [[ -n "${PRESSURE_VESSEL_FILESYSTEMS_RW:-}" ]]; then
  export PRESSURE_VESSEL_FILESYSTEMS_RW="${BASE}:${PRESSURE_VESSEL_FILESYSTEMS_RW}"
else
  export PRESSURE_VESSEL_FILESYSTEMS_RW="${BASE}"
fi

layer_enabled=1
if [[ "${DISABLE_LAYER}" == "1" ]]; then
  layer_enabled=0
fi
if [[ "${!DISABLE_LAYER_IMPL_ENV:-0}" == "1" ]]; then
  layer_enabled=0
fi

if [[ "${layer_enabled}" == "1" ]]; then
  export VK_LAYER_PATH="${LAYER_DIR}${VK_LAYER_PATH:+:${VK_LAYER_PATH}}"
  export VK_INSTANCE_LAYERS="${LAYER_NAME}"
  export ENABLE_OMFG_RUST="${ENABLE_OMFG_RUST:-1}"
  export OMFG_LAYER_MODE="${OMFG_LAYER_MODE:-reproject-blend}"

  # Optional tuning knobs. These are read by the Rust layer if present.
  export OMFG_REPROJECT_SEARCH_RADIUS="${OMFG_REPROJECT_SEARCH_RADIUS:-2}"
  export OMFG_REPROJECT_PATCH_RADIUS="${OMFG_REPROJECT_PATCH_RADIUS:-1}"
  export OMFG_REPROJECT_CONFIDENCE_SCALE="${OMFG_REPROJECT_CONFIDENCE_SCALE:-4.0}"
  export OMFG_REPROJECT_DISOCCLUSION_CURRENT_BIAS="${OMFG_REPROJECT_DISOCCLUSION_CURRENT_BIAS:-0.75}"
  export OMFG_SWAPCHAIN_IMAGE_BUMP_OVERRIDE="${OMFG_SWAPCHAIN_IMAGE_BUMP_OVERRIDE:-}"
  export OMFG_BFI_PERIOD="${OMFG_BFI_PERIOD:-}"
  export OMFG_BFI_HOLD_MS="${OMFG_BFI_HOLD_MS:-}"
  export OMFG_HISTORY_COPY_FREEZE_HISTORY="${OMFG_HISTORY_COPY_FREEZE_HISTORY:-}"
  export OMFG_COPY_ORIGINAL_PRESENT_FIRST="${OMFG_COPY_ORIGINAL_PRESENT_FIRST:-}"
  export OMFG_BLEND_ORIGINAL_PRESENT_FIRST="${OMFG_BLEND_ORIGINAL_PRESENT_FIRST:-}"
  export OMFG_GENERATED_ACQUIRE_TIMEOUT_NS="${OMFG_GENERATED_ACQUIRE_TIMEOUT_NS:-}"

  mkdir -p "${LOG_DIR}"
  export OMFG_LAYER_LOG_FILE="${OMFG_LAYER_LOG_FILE:-${LOG_DIR}/omfg.log}"
  if [[ "${OMFG_TRUNCATE_LAYER_LOG:-1}" == "1" ]]; then
    rm -f "${OMFG_LAYER_LOG_FILE}"
  fi
else
  unset VK_LAYER_PATH VK_INSTANCE_LAYERS ENABLE_OMFG_RUST OMFG_LAYER_MODE OMFG_LAYER_LOG_FILE || true
fi

WRAPPER_LOG_FILE="${OMFG_WRAPPER_LOG_FILE:-}"
if [[ -n "${WRAPPER_LOG_FILE}" ]]; then
  mkdir -p "$(dirname "${WRAPPER_LOG_FILE}")"
  {
    printf '[%s] wrapper-exec layerEnabled=%s mode=%s cwd=%s argc=%s\n' "$(date +%s)" "${layer_enabled}" "${OMFG_LAYER_MODE:-}" "$(pwd)" "$#"
    printf 'argv=%s\n' "$*"
    printf 'VK_LAYER_PATH=%s\n' "${VK_LAYER_PATH:-}"
    printf 'VK_INSTANCE_LAYERS=%s ENABLE_OMFG_RUST=%s OMFG_LAYER_LOG_FILE=%s\n' \
      "${VK_INSTANCE_LAYERS:-}" "${ENABLE_OMFG_RUST:-}" "${OMFG_LAYER_LOG_FILE:-}"
  } >> "${WRAPPER_LOG_FILE}" 2>/dev/null || true
fi

exec "$@"
