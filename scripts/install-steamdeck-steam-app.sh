#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
  echo "Usage: $0 <appid> [timeout-seconds]" >&2
  exit 1
fi

APPID="$1"
TIMEOUT_SECS="${2:-120}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

REMOTE_SCRIPT=$(cat <<'EOF'
set -euo pipefail
APPID="__APPID__"
TIMEOUT_SECS="__TIMEOUT__"

set -a
source /run/user/1000/gamescope-environment
set +a
export DISPLAY=:0
export XDG_RUNTIME_DIR=/run/user/1000
xauth=$(ls -1 /run/user/1000/xauth_* 2>/dev/null | head -1 || true)
if [ -n "$xauth" ]; then export XAUTHORITY="$xauth"; fi

MANIFEST="/home/deck/.local/share/Steam/steamapps/appmanifest_${APPID}.acf"
CONTENT_LOG="/home/deck/.local/share/Steam/logs/content_log.txt"

steam steam://open/console >/tmp/steam-open-console-${APPID}.log 2>&1 &
sleep 3
wid=$(xdotool search --name '^Steam$' | tail -1 || true)
if [ -z "$wid" ]; then
  echo "could not find Steam window" >&2
  exit 1
fi

xdotool type --window "$wid" --delay 40 "app_install ${APPID}"
xdotool key --window "$wid" Return

end_ts=$(( $(date +%s) + TIMEOUT_SECS ))
while [ "$(date +%s)" -lt "$end_ts" ]; do
  if [ -f "$MANIFEST" ]; then
    echo "manifest-present"
    grep -E '"(appid|name|StateFlags|SizeOnDisk|BytesToDownload|BytesDownloaded|buildid)"' "$MANIFEST" || true
    echo "--- content-log tail ---"
    if [ -f "$CONTENT_LOG" ]; then
      tail -40 "$CONTENT_LOG" | grep "AppID ${APPID}\|Dependency added:\|update started\|finished update\|state changed" || true
    fi
    exit 0
  fi
  sleep 2
done

echo "install-timeout"
if [ -f "$CONTENT_LOG" ]; then
  tail -80 "$CONTENT_LOG" | grep "AppID ${APPID}\|Dependency added:\|update started\|finished update\|state changed" || true
fi
exit 2
EOF
)
REMOTE_SCRIPT="${REMOTE_SCRIPT/__APPID__/${APPID}}"
REMOTE_SCRIPT="${REMOTE_SCRIPT/__TIMEOUT__/${TIMEOUT_SECS}}"

"${SCRIPT_DIR}/steamdeck-run.sh" "$REMOTE_SCRIPT"
