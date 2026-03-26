#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# shellcheck disable=SC1091
source "${ROOT_DIR}/scripts/_ppfg_layer_impl.sh"

REMOTE_BASE="${1:-${PPFG_LAYER_REMOTE_BASE_DEFAULT}}"
LOCAL_OUT_DIR="${ROOT_DIR}/build/linux-amd64/${PPFG_LAYER_BUILD_SUBDIR}/out"

if [[ ! -f "${LOCAL_OUT_DIR}/${PPFG_LAYER_LIB_BASENAME}" ]]; then
  echo "Missing build output. Run PPFG_LAYER_IMPL=${PPFG_LAYER_IMPL} scripts/build-linux-amd64.sh first." >&2
  exit 1
fi

# shellcheck disable=SC1091
source "${ROOT_DIR}/scripts/_steamdeck_env.sh"

"${ROOT_DIR}/scripts/steamdeck-run.sh" "mkdir -p '${REMOTE_BASE}'"
"${ROOT_DIR}/scripts/steamdeck-scp-to.sh" "${LOCAL_OUT_DIR}/${PPFG_LAYER_LIB_BASENAME}" "${REMOTE_BASE}/${PPFG_LAYER_LIB_BASENAME}"
"${ROOT_DIR}/scripts/steamdeck-scp-to.sh" "${LOCAL_OUT_DIR}/${PPFG_LAYER_MANIFEST_BASENAME}" "${REMOTE_BASE}/${PPFG_LAYER_MANIFEST_BASENAME}"

echo "Deployed ${PPFG_LAYER_IMPL} layer to ${STEAMDECK_USER}@${STEAMDECK_HOST}:${REMOTE_BASE}"
