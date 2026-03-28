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
REMOTE_WRAPPER_LOG="${REMOTE_LOG_DIR}/${SLUG}-${MODE}-${RUN_ID}.wrapper.log"
REMOTE_WINDOW_TREE_PATH="${REMOTE_LOG_DIR}/${SLUG}-${MODE}-${RUN_ID}.windows.txt"
REMOTE_SCREENSHOT_PATH="${REMOTE_LOG_DIR}/${SLUG}-${MODE}-${RUN_ID}.png"
REMOTE_STEAM_LOG="/tmp/omfg-${SLUG}-${MODE}-${RUN_ID}.steam.log"
REMOTE_STEAM_CLIENT_LOG="/tmp/omfg-steam-client-${SLUG}-${MODE}-${RUN_ID}.log"
REMOTE_PROTON_LOG="/home/deck/steam-${APP_ID}.log"
WAIT_SEC="${OMFG_GAME_WAIT_SEC:-45}"
ARTIFACT_DIR="${ROOT_DIR}/artifacts/steamdeck/rust/real-games/${SLUG}/${MODE}"
CAPTURE_DISPLAY="${OMFG_CAPTURE_DISPLAY:-}"
WINDOW_NAME_PATTERN="${OMFG_WINDOW_NAME_PATTERN:-${TITLE}}"
AUTO_KEYS="${OMFG_AUTO_KEYS:-}"
AUTO_KEYS_EVERY_SEC="${OMFG_AUTO_KEYS_EVERY_SEC:-0}"
DISABLE_LAYER="${OMFG_DISABLE_LAYER:-0}"
CLEANUP_WAIT_SEC="${OMFG_CLEANUP_WAIT_SEC:-20}"
RESTART_STEAM_CLIENT="${OMFG_RESTART_STEAM_CLIENT:-0}"

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
if [[ "${DISABLE_LAYER}" != "1" ]]; then
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
  export OMFG_HOT_CONFIG_PATH="${OMFG_HOT_CONFIG_PATH:-}"
  export OMFG_LAYER_LOG_FILE="${REMOTE_LOG_PATH}"
  mkdir -p "\${BASE}/logs"
  rm -f "\${OMFG_LAYER_LOG_FILE}"
else
  unset VK_LAYER_PATH VK_INSTANCE_LAYERS ENABLE_OMFG_RUST OMFG_LAYER_MODE OMFG_LAYER_LOG_FILE || true
fi
{
  printf '[%s] wrapper-exec mode=%s cwd=%s argc=%s disableLayer=%s\n' "\$(date +%s)" "${MODE}" "\$(pwd)" "\$#" "${DISABLE_LAYER}"
  printf 'argv=%s\n' "\$*"
  printf 'VK_INSTANCE_LAYERS=%s ENABLE_OMFG_RUST=%s OMFG_LAYER_MODE=%s OMFG_LAYER_LOG_FILE=%s\n' \
    "\${VK_INSTANCE_LAYERS:-}" "\${ENABLE_OMFG_RUST:-}" "\${OMFG_LAYER_MODE:-}" "\${OMFG_LAYER_LOG_FILE:-}"
} >> "${REMOTE_WRAPPER_LOG}" 2>/dev/null || true
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
rm -f ${REMOTE_LOG_PATH} ${REMOTE_WRAPPER_LOG} ${REMOTE_WINDOW_TREE_PATH} ${REMOTE_SCREENSHOT_PATH} ${REMOTE_STEAM_LOG} ${REMOTE_STEAM_CLIENT_LOG} ${REMOTE_PROTON_LOG}
set -a
source /run/user/1000/gamescope-environment
set +a
export DISPLAY=:0
export XDG_RUNTIME_DIR=/run/user/1000
xauth=\$(ls -1 /run/user/1000/xauth_* 2>/dev/null | head -1 || true)
if [ -n "\$xauth" ]; then export XAUTHORITY="\$xauth"; fi
if [ -n "${OMFG_PROTON_LOG:-}" ]; then export PROTON_LOG="${OMFG_PROTON_LOG:-}"; fi
POLL_INTERVAL="${OMFG_PROCESS_POLL_INTERVAL_SEC:-15}"
kill_all_steam_games() {
  # Broad pre-kill: terminates any running Steam game processes regardless of AppID.
  # This prevents cross-game contamination when multiple probes run back-to-back.
  python3 - <<'PY'
import os, re, subprocess, signal
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
appid_re = re.compile(r'AppId=\d+')
proton_markers = ('SteamLaunch', 'waitforexitandrun', 'reaper steam', 'wineserver', 'wine64', 'wine ')
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
    if appid_re.search(args) or any(m in args for m in proton_markers):
        try:
            os.kill(pid, signal.SIGKILL)
            print(f'killed pid={pid} args={args}')
        except ProcessLookupError:
            pass
        except PermissionError:
            pass
PY
}
cleanup_matches() {
  APPID='${APP_ID}' EXE_REGEX='${EXE_REGEX}' python3 - <<'PY'
import os, re, subprocess, signal
appid = os.environ['APPID']
exe_regex = os.environ.get('EXE_REGEX', '')
exe_re = re.compile(exe_regex) if exe_regex else None
appid_marker = f'AppId={appid}'
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
    if appid_marker in args or (exe_re and exe_re.search(args)):
        try:
            os.kill(pid, signal.SIGKILL)
            print(f'killed pid={pid} args={args}')
        except ProcessLookupError:
            pass
        except PermissionError:
            pass
PY
}
wait_for_no_matches() {
  local remaining
  for _ in \$(seq 1 ${CLEANUP_WAIT_SEC}); do
    remaining=\$(ps -ef | grep -E "AppId=${APP_ID}${EXE_REGEX:+|${EXE_REGEX}}" | grep -v grep || true)
    if [ -z "\$remaining" ]; then
      echo "cleanup-settled"
      return 0
    fi
    sleep 1
  done
  echo "cleanup-timeout"
  ps -ef | grep -E "AppId=${APP_ID}${EXE_REGEX:+|${EXE_REGEX}}" | grep -v grep || true
  return 0
}
kill_all_steam_games || true
sleep 3
cleanup_matches || true
wait_for_no_matches || true
sleep 2
echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} run=${RUN_ID} paths ==="
echo "remote_log=${REMOTE_LOG_PATH}"
echo "remote_wrapper_log=${REMOTE_WRAPPER_LOG}"
echo "remote_steam_log=${REMOTE_STEAM_LOG}"
if [ "${RESTART_STEAM_CLIENT}" = "1" ]; then echo "remote_steam_client_log=${REMOTE_STEAM_CLIENT_LOG}"; fi
if [ -n "${OMFG_PROTON_LOG:-}" ]; then echo "remote_proton_log=${REMOTE_PROTON_LOG}"; fi
if [ "${RESTART_STEAM_CLIENT}" = "1" ]; then
  echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} restarting-steam-client ==="
  steam -shutdown || true
  sleep 8
  nohup steam >${REMOTE_STEAM_CLIENT_LOG} 2>&1 &
  for _ in \$(seq 1 30); do
    if pgrep -f 'steamwebhelper|steam -srt-logger-opened' >/dev/null 2>&1; then
      echo 'steam-client-ready'
      break
    fi
    sleep 1
  done
  ps -ef | grep -E 'steamwebhelper|steam -srt-logger-opened' | grep -v grep || true
fi
nohup steam steam://rungameid/${APP_ID} >${REMOTE_STEAM_LOG} 2>&1 &
elapsed=0
while [ "\$elapsed" -lt ${WAIT_SEC} ]; do
  sleep "\$POLL_INTERVAL"
  elapsed=\$((elapsed + POLL_INTERVAL))
  echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} process-snapshot t=\${elapsed}s ==="
  ps -ef | grep -E "AppId=${APP_ID}${EXE_REGEX:+|${EXE_REGEX}}" | grep -v grep || true
  if [ -f ${REMOTE_WRAPPER_LOG} ]; then
    echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} wrapper-log-tail t=\${elapsed}s ==="
    tail -40 ${REMOTE_WRAPPER_LOG}
  fi
  if [ -n "${AUTO_KEYS}" ] && [ "${AUTO_KEYS_EVERY_SEC}" -gt 0 ] && [ \$((elapsed % ${AUTO_KEYS_EVERY_SEC})) -eq 0 ]; then
    echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} auto-input t=\${elapsed}s keys=${AUTO_KEYS} window=${WINDOW_NAME_PATTERN} ==="
    win_id=\$(xdotool search --name "${WINDOW_NAME_PATTERN}" 2>/dev/null | head -1 || true)
    if [ -n "\$win_id" ]; then
      xdotool windowactivate --sync "\$win_id" || true
      # shellcheck disable=SC2086
      xdotool key --window "\$win_id" ${AUTO_KEYS} || true
      echo "sent-input win_id=\$win_id"
    else
      echo missing-window-for-input
    fi
  fi
done
echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} process ==="
ps -ef | grep -E "AppId=${APP_ID}${EXE_REGEX:+|${EXE_REGEX}}" | grep -v grep || true
echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} omfg log ==="
if [ -f ${REMOTE_LOG_PATH} ]; then tail -200 ${REMOTE_LOG_PATH}; else echo missing-log; fi
echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} wrapper log ==="
if [ -f ${REMOTE_WRAPPER_LOG} ]; then tail -120 ${REMOTE_WRAPPER_LOG}; else echo missing-wrapper-log; fi
echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} steam log ==="
if [ -f ${REMOTE_STEAM_LOG} ]; then tail -80 ${REMOTE_STEAM_LOG}; else echo missing-steam-log; fi
if [ "${RESTART_STEAM_CLIENT}" = "1" ]; then
  echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} steam client log ==="
  if [ -f ${REMOTE_STEAM_CLIENT_LOG} ]; then tail -120 ${REMOTE_STEAM_CLIENT_LOG}; else echo missing-steam-client-log; fi
fi
if [ -n "${OMFG_PROTON_LOG:-}" ]; then
  echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} proton log ==="
  if [ -f ${REMOTE_PROTON_LOG} ]; then tail -120 ${REMOTE_PROTON_LOG}; else echo missing-proton-log; fi
fi
if [ -n "${CAPTURE_DISPLAY}" ]; then
  echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} window tree ==="
  if xwininfo -root -tree > ${REMOTE_WINDOW_TREE_PATH} 2>/dev/null; then
    sed -n '1,200p' ${REMOTE_WINDOW_TREE_PATH}
  else
    echo missing-window-tree
  fi
  echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} screenshot capture ==="
  if ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -i :0 -frames:v 1 ${REMOTE_SCREENSHOT_PATH}; then
    ls -lh ${REMOTE_SCREENSHOT_PATH}
  else
    echo missing-screenshot
  fi
fi
cleanup_matches || true
echo "=== title=${TITLE} appid=${APP_ID} mode=${MODE} post-clean process check ==="
ps -ef | grep -E "AppId=${APP_ID}${EXE_REGEX:+|${EXE_REGEX}}" | grep -v grep || true
EOF
)

"${ROOT_DIR}/scripts/steamdeck-run.sh" "${REMOTE_CMD}" | tee "${ARTIFACT_DIR}/session.txt"

if "${ROOT_DIR}/scripts/steamdeck-scp-from.sh" "${REMOTE_LOG_PATH}" "${ARTIFACT_DIR}/omfg.log"; then
  :
fi
if "${ROOT_DIR}/scripts/steamdeck-scp-from.sh" "${REMOTE_WRAPPER_LOG}" "${ARTIFACT_DIR}/wrapper.log"; then
  :
fi
if "${ROOT_DIR}/scripts/steamdeck-scp-from.sh" "${REMOTE_STEAM_LOG}" "${ARTIFACT_DIR}/steam-launch.log"; then
  :
fi
if [[ "${RESTART_STEAM_CLIENT}" = "1" ]]; then
  if "${ROOT_DIR}/scripts/steamdeck-scp-from.sh" "${REMOTE_STEAM_CLIENT_LOG}" "${ARTIFACT_DIR}/steam-client.log"; then
    :
  fi
fi
if [[ -n "${OMFG_PROTON_LOG:-}" ]]; then
  if "${ROOT_DIR}/scripts/steamdeck-scp-from.sh" "${REMOTE_PROTON_LOG}" "${ARTIFACT_DIR}/proton.log"; then
    :
  fi
fi
if [[ -n "${CAPTURE_DISPLAY}" ]]; then
  if "${ROOT_DIR}/scripts/steamdeck-scp-from.sh" "${REMOTE_WINDOW_TREE_PATH}" "${ARTIFACT_DIR}/windows.txt"; then
    :
  fi
  if "${ROOT_DIR}/scripts/steamdeck-scp-from.sh" "${REMOTE_SCREENSHOT_PATH}" "${ARTIFACT_DIR}/screen.png"; then
    :
  fi
fi

restore_wrapper
trap cleanup EXIT

echo "Artifacts saved under ${ARTIFACT_DIR}"
