#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
MODE="${OMFG_LAYER_MODE:-reproject-blend}"
APP_ID="960990"
REMOTE_WRAPPER="/home/deck/omfg.sh"
REMOTE_LOG_DIR="/home/deck/post-proc-fg-research/logs"
REMOTE_LOG_PATH="${REMOTE_LOG_DIR}/beyond-${MODE}.log"
REMOTE_STEAM_LOG="/tmp/omfg-beyond-${MODE}.steam.log"
REMOTE_WINDOW_TREE_PATH="${REMOTE_LOG_DIR}/beyond-${MODE}.windows.txt"
REMOTE_SCREENSHOT_PATH="${REMOTE_LOG_DIR}/beyond-${MODE}.png"
WAIT_SEC="${OMFG_BEYOND_WAIT_SEC:-45}"
POLL_INTERVAL_SEC="${OMFG_PROCESS_POLL_INTERVAL_SEC:-15}"
ARTIFACT_DIR="${ROOT_DIR}/artifacts/steamdeck/rust/real-games/beyond-two-souls/${MODE}"
WINDOW_NAME_PATTERN="${OMFG_WINDOW_NAME_PATTERN:-Beyond: Two Souls}"
AUTO_KEYS="${OMFG_AUTO_KEYS:-}"
AUTO_KEYS_EVERY_SEC="${OMFG_AUTO_KEYS_EVERY_SEC:-0}"
DISABLE_LAYER="${OMFG_DISABLE_LAYER:-0}"

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
rm -f ${REMOTE_LOG_PATH} ${REMOTE_STEAM_LOG} ${REMOTE_WINDOW_TREE_PATH} ${REMOTE_SCREENSHOT_PATH}
set -a
source /run/user/1000/gamescope-environment
set +a
export DISPLAY=:0
export XDG_RUNTIME_DIR=/run/user/1000
xauth=\$(ls -1 /run/user/1000/xauth_* 2>/dev/null | head -1 || true)
if [ -n "\$xauth" ]; then export XAUTHORITY="\$xauth"; fi
kill_all_steam_games() {
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
  APPID='${APP_ID}' python3 - <<'PY'
import os, re, subprocess, signal
appid = os.environ['APPID']
patterns = [
    f'AppId={appid}',
    'BeyondTwoSouls_Steam.exe',
    '/proton waitforexitandrun /home/deck/.local/share/Steam/steamapps/common/BEYOND Two Souls/BeyondTwoSouls_Steam.exe',
    '/proton run /home/deck/.local/share/Steam/steamapps/common/BEYOND Two Souls/BeyondTwoSouls_Steam.exe',
]
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
kill_all_steam_games || true
sleep 3
cleanup_matches || true
sleep 2
nohup steam steam://rungameid/${APP_ID} >/tmp/omfg-beyond-launch-${MODE}.log 2>&1 &
elapsed=0
while [ "\$elapsed" -lt ${WAIT_SEC} ]; do
  sleep ${POLL_INTERVAL_SEC}
  elapsed=\$((elapsed + ${POLL_INTERVAL_SEC}))
  echo "=== beyond mode=${MODE} process-snapshot t=\${elapsed}s ==="
  ps -ef | grep -E "BeyondTwoSouls|AppId=${APP_ID}|proton waitforexitandrun|proton run|wineserver" | grep -v grep || true
  if [ -n "${AUTO_KEYS}" ] && [ "${AUTO_KEYS_EVERY_SEC}" -gt 0 ] && [ \$((elapsed % ${AUTO_KEYS_EVERY_SEC})) -eq 0 ]; then
    echo "=== beyond mode=${MODE} auto-input t=\${elapsed}s keys=${AUTO_KEYS} window=${WINDOW_NAME_PATTERN} ==="
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
echo "=== beyond mode=${MODE} process ==="
ps -ef | grep -E "BeyondTwoSouls|AppId=${APP_ID}|proton waitforexitandrun|proton run|wineserver" | grep -v grep || true
echo "=== beyond mode=${MODE} omfg log ==="
if [ -f ${REMOTE_LOG_PATH} ]; then tail -200 ${REMOTE_LOG_PATH}; else echo missing-log; fi
echo "=== beyond mode=${MODE} steam log ==="
if [ -f /tmp/omfg-beyond-launch-${MODE}.log ]; then tail -80 /tmp/omfg-beyond-launch-${MODE}.log; else echo missing-steam-log; fi
if [ -n "${OMFG_CAPTURE_DISPLAY:-}" ]; then
  echo "=== beyond mode=${MODE} window tree ==="
  if xwininfo -root -tree > ${REMOTE_WINDOW_TREE_PATH} 2>/dev/null; then
    sed -n '1,200p' ${REMOTE_WINDOW_TREE_PATH}
  else
    echo missing-window-tree
  fi
  echo "=== beyond mode=${MODE} screenshot capture ==="
  if ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -i :0 -frames:v 1 ${REMOTE_SCREENSHOT_PATH}; then
    ls -lh ${REMOTE_SCREENSHOT_PATH}
  else
    echo missing-screenshot
  fi
fi
cleanup_matches || true
EOF
)

"${ROOT_DIR}/scripts/steamdeck-run.sh" "${REMOTE_CMD}" | tee "${ARTIFACT_DIR}/session.txt"

if "${ROOT_DIR}/scripts/steamdeck-scp-from.sh" "${REMOTE_LOG_PATH}" "${ARTIFACT_DIR}/omfg.log"; then
  :
fi
if "${ROOT_DIR}/scripts/steamdeck-scp-from.sh" "/tmp/omfg-beyond-launch-${MODE}.log" "${ARTIFACT_DIR}/steam-launch.log"; then
  :
fi
if [[ -n "${OMFG_CAPTURE_DISPLAY:-}" ]]; then
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
