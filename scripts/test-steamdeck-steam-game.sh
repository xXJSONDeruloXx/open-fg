#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 3 || $# -gt 4 ]]; then
  echo "Usage: $0 <appid> <slug> <title> [exe-regex]" >&2
  echo "Example: $0 362890 black-mesa 'Black Mesa' 'bms.exe|hl2_linux'" >&2
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
APP_ID="$1"
SLUG="$2"
TITLE="$3"
EXE_REGEX="${4:-}"
MODE="${OMFG_LAYER_MODE:-reproject-blend}"
REMOTE_WRAPPER="/home/deck/omfg.sh"
REMOTE_LOG_DIR="/home/deck/post-proc-fg-research/logs"
RUN_ID="$(date +%Y%m%d-%H%M%S)-$$"
REMOTE_LOG_PATH="${REMOTE_LOG_DIR}/${SLUG}-${MODE}-${RUN_ID}.log"
REMOTE_STEAM_LOG="/tmp/omfg-${SLUG}-${MODE}-${RUN_ID}.steam.log"
WAIT_SEC="${OMFG_GAME_WAIT_SEC:-45}"
ARTIFACT_DIR="${ROOT_DIR}/artifacts/steamdeck/rust/real-games/${SLUG}/${MODE}"

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
cleanup_matches() {
  APPID='${APP_ID}' EXE_REGEX='${EXE_REGEX}' python3 - <<'PY'
import os, subprocess, signal
appid = os.environ['APPID']
exe_regex = os.environ.get('EXE_REGEX', '')
patterns = [f'AppId={appid}']
if exe_regex:
    patterns.append(exe_regex)
self_pid = os.getpid()
ancestors = {self_pid}
try:
    pid = self_pid
    while True:
        with open(f'/proc/{pid}/status', 'r', encoding='utf-8', errors='ignore') as f:
            ppid = None
            for line in f:
                if line.startswith('PPid:'):
                    ppid = int(line.split()[1])
                    break
        if not ppid or ppid in ancestors:
            break
        ancestors.add(ppid)
        pid = ppid
except Exception:
    pass
out = subprocess.check_output(['ps', '-eo', 'pid=,args='], text=True, errors='ignore')
for line in out.splitlines():
    s = line.strip()
    if not s:
        continue
    parts = s.split(None, 1)
    if len(parts) != 2:
        continue
    pid = int(parts[0])
    args = parts[1]
    if pid in ancestors:
        continue
    if any(p and p in args for p in patterns):
        try:
            os.kill(pid, signal.SIGKILL)
            print(f'killed pid={pid} args={args}')
        except ProcessLookupError:
            pass
        except PermissionError:
            pass
PY
}
cleanup_matches || true
sleep 2
echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} run=${RUN_ID} paths ==="
echo "remote_log=${REMOTE_LOG_PATH}"
echo "remote_steam_log=${REMOTE_STEAM_LOG}"
nohup steam steam://rungameid/${APP_ID} >${REMOTE_STEAM_LOG} 2>&1 &
sleep ${WAIT_SEC}
echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} process ==="
ps -ef | grep -E "AppId=${APP_ID}${EXE_REGEX:+|${EXE_REGEX}}" | grep -v grep || true
echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} omfg log ==="
if [ -f ${REMOTE_LOG_PATH} ]; then tail -200 ${REMOTE_LOG_PATH}; else echo missing-log; fi
echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} steam log ==="
if [ -f ${REMOTE_STEAM_LOG} ]; then tail -80 ${REMOTE_STEAM_LOG}; else echo missing-steam-log; fi
cleanup_matches || true
echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} post-clean process check ==="
ps -ef | grep -E "AppId=${APP_ID}${EXE_REGEX:+|${EXE_REGEX}}" | grep -v grep || true
EOF
)

"${ROOT_DIR}/scripts/steamdeck-run.sh" "${REMOTE_CMD}" | tee "${ARTIFACT_DIR}/session.txt"

if "${ROOT_DIR}/scripts/steamdeck-scp-from.sh" "${REMOTE_LOG_PATH}" "${ARTIFACT_DIR}/omfg.log"; then
  :
fi
if "${ROOT_DIR}/scripts/steamdeck-scp-from.sh" "${REMOTE_STEAM_LOG}" "${ARTIFACT_DIR}/steam-launch.log"; then
  :
fi

restore_wrapper
trap cleanup EXIT

echo "Artifacts saved under ${ARTIFACT_DIR}"
