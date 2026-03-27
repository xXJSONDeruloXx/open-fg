#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
: "${OMFG_LAYER_IMPL:=rust}"

ENV_FILE="${ROOT_DIR}/.env.steamdeck.local"
if [[ -f "${ENV_FILE}" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "${ENV_FILE}"
  set +a
fi

# shellcheck disable=SC1091
source "${ROOT_DIR}/scripts/_omfg_layer_impl.sh"

if [[ "${OMFG_LAYER_IMPL}" == "rust" ]]; then
  "${ROOT_DIR}/scripts/test-rust-layer.sh"
fi

OMFG_LAYER_IMPL="${OMFG_LAYER_IMPL}" "${ROOT_DIR}/scripts/build-linux-amd64.sh"

if [[ -z "${STEAMDECK_PASS:-}" ]]; then
  echo "STEAMDECK_PASS not set; skipped Steam Deck smoke suite." >&2
  exit 0
fi

OMFG_LAYER_IMPL="${OMFG_LAYER_IMPL}" "${ROOT_DIR}/scripts/deploy-steamdeck-layer.sh"

modes=(passthrough clear copy history-copy blend adaptive-blend search-blend search-adaptive-blend reproject-blend reproject-adaptive-blend optflow-blend multi-blend adaptive-multi-blend reproject-multi-blend reproject-adaptive-multi-blend)
if [[ "${OMFG_LAYER_IMPL}" == "rust" ]]; then
  modes+=(bfi)
fi

for mode in "${modes[@]}"; do
  OMFG_LAYER_IMPL="${OMFG_LAYER_IMPL}" OMFG_LAYER_MODE="${mode}" "${ROOT_DIR}/scripts/test-steamdeck-vkcube.sh"
  python3 "${ROOT_DIR}/scripts/assert-vkcube-log.py" \
    --mode "${mode}" \
    --log "${ROOT_DIR}/${OMFG_LAYER_ARTIFACT_ROOT_REL}/vkcube/${mode}/omfg-vkcube.log"
done

echo "Regression suite passed for ${OMFG_LAYER_IMPL}"
