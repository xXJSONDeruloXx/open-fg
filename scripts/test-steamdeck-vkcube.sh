#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
MODE="${PPFG_LAYER_MODE:-passthrough}"
VKCUBE_COUNT="${PPFG_VKCUBE_COUNT:-120}"
VKCUBE_TIMEOUT_SEC="${PPFG_VKCUBE_TIMEOUT_SEC:-20}"
VKCUBE_PRESENT_MODE="${PPFG_VKCUBE_PRESENT_MODE:-}"
VKCUBE_EXTRA_ARGS="${PPFG_VKCUBE_EXTRA_ARGS:-}"
ARTIFACT_SUFFIX="${PPFG_VKCUBE_ARTIFACT_SUFFIX:-}"

# shellcheck disable=SC1091
source "${ROOT_DIR}/scripts/_ppfg_layer_impl.sh"

REMOTE_BASE="${1:-${PPFG_LAYER_REMOTE_BASE_DEFAULT}}"
RUN_NAME="${MODE}"
if [[ -n "${ARTIFACT_SUFFIX}" ]]; then
  RUN_NAME="${MODE}-${ARTIFACT_SUFFIX}"
fi
ARTIFACT_DIR="${ROOT_DIR}/${PPFG_LAYER_ARTIFACT_ROOT_REL}/vkcube/${RUN_NAME}"

mkdir -p "${ARTIFACT_DIR}"

PRESENT_MODE_ARG=""
if [[ -n "${VKCUBE_PRESENT_MODE}" ]]; then
  PRESENT_MODE_ARG="--present_mode ${VKCUBE_PRESENT_MODE}"
fi

REMOTE_SCRIPT=$(cat <<EOF
set -euo pipefail
mkdir -p ${REMOTE_BASE}
rm -f ${REMOTE_BASE}/ppfg-vkcube.log ${REMOTE_BASE}/vkcube.stdout
set -a
source /run/user/1000/gamescope-environment
set +a
export DISPLAY=:0
export XDG_RUNTIME_DIR=/run/user/1000
export XAUTHORITY=\$(ls -1 /run/user/1000/xauth_* | head -1)
export DISABLE_GAMESCOPE_WSI=1
export ${PPFG_LAYER_ENABLE_ENV}=1
export PPFG_LAYER_MODE=${MODE}
export PPFG_LAYER_LOG_FILE=${REMOTE_BASE}/ppfg-vkcube.log
export PPFG_BLEND_ADAPTIVE_STRENGTH=${PPFG_BLEND_ADAPTIVE_STRENGTH:-}
export PPFG_BLEND_ADAPTIVE_BIAS=${PPFG_BLEND_ADAPTIVE_BIAS:-}
export PPFG_SEARCH_BLEND_RADIUS=${PPFG_SEARCH_BLEND_RADIUS:-}
export PPFG_REPROJECT_SEARCH_RADIUS=${PPFG_REPROJECT_SEARCH_RADIUS:-}
export PPFG_REPROJECT_PATCH_RADIUS=${PPFG_REPROJECT_PATCH_RADIUS:-}
export PPFG_REPROJECT_CONFIDENCE_SCALE=${PPFG_REPROJECT_CONFIDENCE_SCALE:-}
export PPFG_MULTI_BLEND_COUNT=${PPFG_MULTI_BLEND_COUNT:-}
export PPFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=${PPFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES:-}
export PPFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=${PPFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES:-}
export PPFG_ADAPTIVE_MULTI_INTERVAL_THRESHOLD_MS=${PPFG_ADAPTIVE_MULTI_INTERVAL_THRESHOLD_MS:-}
export PPFG_ADAPTIVE_MULTI_TARGET_FPS=${PPFG_ADAPTIVE_MULTI_TARGET_FPS:-}
export PPFG_ADAPTIVE_MULTI_INTERVAL_SMOOTHING_ALPHA=${PPFG_ADAPTIVE_MULTI_INTERVAL_SMOOTHING_ALPHA:-}
export PPFG_BFI_PERIOD=${PPFG_BFI_PERIOD:-}
export PPFG_BFI_HOLD_MS=${PPFG_BFI_HOLD_MS:-}
export PPFG_VISUAL_HOLD_MS=${PPFG_VISUAL_HOLD_MS:-}
export PPFG_BENCHMARK=${PPFG_BENCHMARK:-}
export PPFG_BENCHMARK_LABEL=${PPFG_BENCHMARK_LABEL:-}
export VK_LAYER_PATH=${REMOTE_BASE}
export VK_INSTANCE_LAYERS=${PPFG_LAYER_NAME}
printf 'RUN impl=%s display=%s xauthority=%s mode=%s layer=%s count=%s present_mode=%s extra=%s target_fps=%s min_generated=%s max_generated=%s bfi_period=%s bfi_hold_ms=%s visual_hold_ms=%s benchmark=%s benchmark_label=%s\n' "${PPFG_LAYER_IMPL}" "\$DISPLAY" "\$XAUTHORITY" "\$PPFG_LAYER_MODE" "\$VK_INSTANCE_LAYERS" "${VKCUBE_COUNT}" "${VKCUBE_PRESENT_MODE:-default}" "${VKCUBE_EXTRA_ARGS:-none}" "${PPFG_ADAPTIVE_MULTI_TARGET_FPS:-default}" "${PPFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES:-default}" "${PPFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES:-default}" "${PPFG_BFI_PERIOD:-default}" "${PPFG_BFI_HOLD_MS:-default}" "${PPFG_VISUAL_HOLD_MS:-default}" "${PPFG_BENCHMARK:-default}" "${PPFG_BENCHMARK_LABEL:-default}"
timeout ${VKCUBE_TIMEOUT_SEC}s vkcube --c ${VKCUBE_COUNT} --suppress_popups --wsi xcb ${PRESENT_MODE_ARG} ${VKCUBE_EXTRA_ARGS} > ${REMOTE_BASE}/vkcube.stdout 2>&1 || status=\$?
printf 'VCUBE_STATUS=%s\n' "\${status:-0}"
ls -lah ${REMOTE_BASE}
printf '\n--- vkcube.stdout ---\n'
sed -n '1,200p' ${REMOTE_BASE}/vkcube.stdout || true
printf '\n--- ppfg-vkcube.log ---\n'
sed -n '1,260p' ${REMOTE_BASE}/ppfg-vkcube.log || true
EOF
)

"${ROOT_DIR}/scripts/steamdeck-run.sh" "${REMOTE_SCRIPT}"
"${ROOT_DIR}/scripts/steamdeck-scp-from.sh" "${REMOTE_BASE}/vkcube.stdout" "${ARTIFACT_DIR}/vkcube.stdout"
"${ROOT_DIR}/scripts/steamdeck-scp-from.sh" "${REMOTE_BASE}/ppfg-vkcube.log" "${ARTIFACT_DIR}/ppfg-vkcube.log"

echo "Artifacts saved under ${ARTIFACT_DIR}"
