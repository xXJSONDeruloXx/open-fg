#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
MODE="${OMFG_LAYER_MODE:-reproject-blend}"
APP_ID="960990"
REMOTE_WRAPPER="/home/deck/omfg.sh"
REMOTE_LOG_DIR="/home/deck/post-proc-fg-research/logs"
REMOTE_LOG_PATH="${REMOTE_LOG_DIR}/beyond-${MODE}.log"
REMOTE_STEAM_LOG="/tmp/omfg-beyond-${MODE}.steam.log"
WAIT_SEC="${OMFG_BEYOND_WAIT_SEC:-45}"
ARTIFACT_DIR="${ROOT_DIR}/artifacts/steamdeck/rust/real-games/beyond-two-souls/${MODE}"

mkdir -p "${ARTIFACT_DIR}"

TMP_WRAPPER="$(mktemp)"
ORIG_WRAPPER="$(mktemp)"
cleanup() {
  rm -f "${TMP_WRAPPER}" "${ORIG_WRAPPER}"
}
trap cleanup EXIT

cat >"${TMP_WRAPPER}" <<EOF
#!/usr/bin/env bash
set -euo pipefail
BASE=/home/deck/post-proc-fg-research
if [[ -n "\${PRESSURE_VESSEL_FILESYSTEMS_RW:-}" ]]; then
  export PRESSURE_VESSEL_FILESYSTEMS_RW="\${BASE}:\${PRESSURE_VESSEL_FILESYSTEMS_RW}"
else
  export PRESSURE_VESSEL_FILESYSTEMS_RW="\${BASE}"
fi
export VK_LAYER_PATH="\${BASE}/deploy/vk-layer-rust"
export VK_INSTANCE_LAYERS="VK_LAYER_OMFG_rust"
export ENABLE_OMFG_RUST=1
export OMFG_LAYER_MODE="${MODE}"
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
export OMFG_LAYER_LOG_FILE="${REMOTE_LOG_PATH}"
mkdir -p "\${BASE}/logs"
rm -f "\${OMFG_LAYER_LOG_FILE}"
exec "\$@"
EOF

chmod +x "${TMP_WRAPPER}"
"${ROOT_DIR}/scripts/steamdeck-scp-from.sh" "${REMOTE_WRAPPER}" "${ORIG_WRAPPER}"
"${ROOT_DIR}/scripts/steamdeck-scp-to.sh" "${TMP_WRAPPER}" "${REMOTE_WRAPPER}"

restore_wrapper() {
  "${ROOT_DIR}/scripts/steamdeck-scp-to.sh" "${ORIG_WRAPPER}" "${REMOTE_WRAPPER}"
  "${ROOT_DIR}/scripts/steamdeck-run.sh" "chmod +x ${REMOTE_WRAPPER}" >/dev/null
}
trap 'restore_wrapper; cleanup' EXIT

REMOTE_CMD=$(cat <<EOF
set -euo pipefail
chmod +x ${REMOTE_WRAPPER}
mkdir -p ${REMOTE_LOG_DIR}
rm -f ${REMOTE_LOG_PATH} ${REMOTE_STEAM_LOG}
set -a
source /run/user/1000/gamescope-environment
set +a
export DISPLAY=:0
export XDG_RUNTIME_DIR=/run/user/1000
xauth=\$(ls -1 /run/user/1000/xauth_* 2>/dev/null | head -1 || true)
if [ -n "\$xauth" ]; then export XAUTHORITY="\$xauth"; fi
nohup steam steam://rungameid/${APP_ID} >/tmp/omfg-beyond-launch-${MODE}.log 2>&1 &
sleep ${WAIT_SEC}
echo "=== beyond mode=${MODE} process ==="
ps -ef | grep -E "BeyondTwoSouls|AppId=${APP_ID}|proton waitforexitandrun|proton run|wineserver" | grep -v grep || true
echo "=== beyond mode=${MODE} omfg log ==="
if [ -f ${REMOTE_LOG_PATH} ]; then tail -200 ${REMOTE_LOG_PATH}; else echo missing-log; fi
echo "=== beyond mode=${MODE} steam log ==="
if [ -f /tmp/omfg-beyond-launch-${MODE}.log ]; then tail -80 /tmp/omfg-beyond-launch-${MODE}.log; else echo missing-steam-log; fi
pkill -f BeyondTwoSouls_Steam.exe || true
pkill -f "AppId=${APP_ID}" || true
pkill -f "/proton waitforexitandrun .*BEYOND Two Souls" || true
pkill -f "/proton run .*BEYOND Two Souls" || true
EOF
)

"${ROOT_DIR}/scripts/steamdeck-run.sh" "${REMOTE_CMD}" | tee "${ARTIFACT_DIR}/session.txt"

if "${ROOT_DIR}/scripts/steamdeck-scp-from.sh" "${REMOTE_LOG_PATH}" "${ARTIFACT_DIR}/omfg.log"; then
  :
fi
if "${ROOT_DIR}/scripts/steamdeck-scp-from.sh" "/tmp/omfg-beyond-launch-${MODE}.log" "${ARTIFACT_DIR}/steam-launch.log"; then
  :
fi

restore_wrapper
trap cleanup EXIT

echo "Artifacts saved under ${ARTIFACT_DIR}"
