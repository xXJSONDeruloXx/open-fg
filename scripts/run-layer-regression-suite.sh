#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
: "${PPFG_LAYER_IMPL:=rust}"

# shellcheck disable=SC1091
source "${ROOT_DIR}/scripts/_ppfg_layer_impl.sh"

if [[ "${PPFG_LAYER_IMPL}" == "rust" ]]; then
  "${ROOT_DIR}/scripts/test-rust-layer.sh"
fi

PPFG_LAYER_IMPL="${PPFG_LAYER_IMPL}" "${ROOT_DIR}/scripts/build-linux-amd64.sh"

if [[ -z "${STEAMDECK_PASS:-}" ]]; then
  echo "STEAMDECK_PASS not set; skipped Steam Deck smoke suite." >&2
  exit 0
fi

PPFG_LAYER_IMPL="${PPFG_LAYER_IMPL}" "${ROOT_DIR}/scripts/deploy-steamdeck-layer.sh"

for mode in passthrough clear copy history-copy blend adaptive-blend search-blend multi-blend adaptive-multi-blend; do
  PPFG_LAYER_IMPL="${PPFG_LAYER_IMPL}" PPFG_LAYER_MODE="${mode}" "${ROOT_DIR}/scripts/test-steamdeck-vkcube.sh"
  python3 "${ROOT_DIR}/scripts/assert-vkcube-log.py" \
    --mode "${mode}" \
    --log "${ROOT_DIR}/${PPFG_LAYER_ARTIFACT_ROOT_REL}/vkcube/${mode}/ppfg-vkcube.log"
done

echo "Regression suite passed for ${PPFG_LAYER_IMPL}"
