#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

REMOTE_SCRIPT=$(cat <<'EOF'
python3 - <<'PY'
from pathlib import Path
steamapps = Path.home()/'.local/share/Steam/steamapps'
cache = Path.home()/'.local/share/Steam/appcache/librarycache'
installed = {p.stem.split('_')[-1] for p in steamapps.glob('appmanifest_*.acf')}
owned = sorted({p.name for p in cache.glob('*') if p.is_dir() and p.name.isdigit()}, key=int)
print(f'owned={len(owned)} installed={len(installed)} uninstalled={len(set(owned)-installed)}')
for appid in owned:
    state = 'installed' if appid in installed else 'owned-uninstalled'
    print(f'{appid}\t{state}')
PY
EOF
)

"${SCRIPT_DIR}/steamdeck-run.sh" "$REMOTE_SCRIPT"
