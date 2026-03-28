#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 4 ]]; then
  cat >&2 <<'EOF'
Usage:
  run-steamdeck-real-game-mode-sweep.sh <preset>
  run-steamdeck-real-game-mode-sweep.sh <appid> <slug> <title> [exe-regex]

Presets:
  re-village | resident-evil-village | 1196590
  stellar-blade | 3489700
  beyond | beyond-two-souls | 960990

Environment:
  OMFG_LAYER_IMPL=rust                     # default: rust
  OMFG_BUILD_FIRST=1                      # run test/build before sweep (default: 1)
  OMFG_DEPLOY_FIRST=1                     # deploy to Deck before sweep (default: 1)
  OMFG_REAL_GAME_MODES="..."              # optional space-separated mode override
  OMFG_GAME_WAIT_SEC=...                  # generic wait override
  OMFG_BEYOND_WAIT_SEC=...                # Beyond-specific wait override
EOF
  exit 1
fi

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
: "${OMFG_LAYER_IMPL:=rust}"
: "${OMFG_BUILD_FIRST:=1}"
: "${OMFG_DEPLOY_FIRST:=1}"

ENV_FILE="${ROOT_DIR}/.env.steamdeck.local"
if [[ -f "${ENV_FILE}" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "${ENV_FILE}"
  set +a
fi

# shellcheck disable=SC1091
source "${ROOT_DIR}/scripts/_omfg_layer_impl.sh"

if [[ -z "${STEAMDECK_PASS:-}" ]]; then
  echo "STEAMDECK_PASS not set; cannot run Deck real-game sweep." >&2
  exit 1
fi

APP_ID=""
SLUG=""
TITLE=""
EXE_REGEX=""
GAME_KIND="generic"
DEFAULT_WAIT_SEC="60"

if [[ $# -eq 1 ]]; then
  case "$1" in
    re-village|resident-evil-village|1196590)
      APP_ID="1196590"
      SLUG="resident-evil-village"
      TITLE="Resident Evil Village"
      EXE_REGEX='re8.exe'
      DEFAULT_WAIT_SEC="90"
      ;;
    stellar-blade|3489700)
      APP_ID="3489700"
      SLUG="stellar-blade"
      TITLE="Stellar Blade™"
      EXE_REGEX='SB.exe|SB-Win64-Shipping.exe'
      DEFAULT_WAIT_SEC="90"
      ;;
    beyond|beyond-two-souls|960990)
      APP_ID="960990"
      SLUG="beyond-two-souls"
      TITLE="Beyond: Two Souls"
      EXE_REGEX='BeyondTwoSouls_Steam.exe'
      GAME_KIND="beyond"
      DEFAULT_WAIT_SEC="60"
      ;;
    *)
      echo "Unknown preset: $1" >&2
      exit 1
      ;;
  esac
else
  APP_ID="$1"
  SLUG="$2"
  TITLE="$3"
  EXE_REGEX="${4:-}"
  if [[ "${APP_ID}" == "960990" || "${SLUG}" == "beyond-two-souls" || "${SLUG}" == "beyond" ]]; then
    GAME_KIND="beyond"
    DEFAULT_WAIT_SEC="60"
  elif [[ "${APP_ID}" == "3489700" || "${SLUG}" == "stellar-blade" ]]; then
    DEFAULT_WAIT_SEC="90"
  elif [[ "${APP_ID}" == "1196590" || "${SLUG}" == "resident-evil-village" || "${SLUG}" == "re-village" ]]; then
    DEFAULT_WAIT_SEC="90"
  fi
fi

if [[ -n "${OMFG_REAL_GAME_MODES:-}" ]]; then
  # shellcheck disable=SC2206
  modes=( ${OMFG_REAL_GAME_MODES} )
else
  modes=(
    passthrough
    clear
    bfi
    copy
    history-copy
    blend
    adaptive-blend
    search-blend
    search-adaptive-blend
    reproject-blend
    reproject-adaptive-blend
    optflow-blend
    optflow-adaptive-blend
    multi-blend
    adaptive-multi-blend
    reproject-multi-blend
    reproject-adaptive-multi-blend
    optflow-multi-blend
    optflow-adaptive-multi-blend
  )
fi

if [[ "${OMFG_BUILD_FIRST}" == "1" ]]; then
  if [[ "${OMFG_LAYER_IMPL}" == "rust" ]]; then
    "${ROOT_DIR}/scripts/test-rust-layer.sh"
  fi
  OMFG_LAYER_IMPL="${OMFG_LAYER_IMPL}" "${ROOT_DIR}/scripts/build-linux-amd64.sh"
fi

if [[ "${OMFG_DEPLOY_FIRST}" == "1" ]]; then
  OMFG_LAYER_IMPL="${OMFG_LAYER_IMPL}" "${ROOT_DIR}/scripts/deploy-steamdeck-layer.sh"
fi

RUN_ID="$(date +%Y%m%d-%H%M%S)"
SWEEP_DIR="${ROOT_DIR}/${OMFG_LAYER_ARTIFACT_ROOT_REL}/real-games/${SLUG}/_sweeps/${RUN_ID}"
SUMMARY_TSV="${SWEEP_DIR}/summary.tsv"
mkdir -p "${SWEEP_DIR}"

printf 'run_id\tappid\tslug\ttitle\tmode\tstatus\tdevice_ok\tswapchain_ok\tfirst_generated\tsustained_progress\tlog_path\n' > "${SUMMARY_TSV}"

summarize_log() {
  local log_path="$1"
  local status="missing-log"
  local device_ok=0
  local swapchain_ok=0
  local first_generated=0
  local sustained_progress=0

  if [[ -f "${log_path}" ]]; then
    grep -q "vkCreateDevice ok" "${log_path}" && device_ok=1 || true
    grep -q "vkCreateSwapchainKHR ok" "${log_path}" && swapchain_ok=1 || true
    grep -Eq "first .*present succeeded" "${log_path}" && first_generated=1 || true
    grep -Eq "frame=60|present=60" "${log_path}" && sustained_progress=1 || true

    if [[ "${sustained_progress}" == "1" ]]; then
      status="sustained"
    elif [[ "${first_generated}" == "1" ]]; then
      status="generated-started"
    elif [[ "${swapchain_ok}" == "1" ]]; then
      status="swapchain-ok"
    elif [[ "${device_ok}" == "1" ]]; then
      status="device-ok"
    else
      status="log-present"
    fi
  fi

  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "${RUN_ID}" "${APP_ID}" "${SLUG}" "${TITLE}" "${CURRENT_MODE}" "${status}" \
    "${device_ok}" "${swapchain_ok}" "${first_generated}" "${sustained_progress}" "${log_path}" >> "${SUMMARY_TSV}"

  echo "mode=${CURRENT_MODE} status=${status} log=${log_path}"
}

for CURRENT_MODE in "${modes[@]}"; do
  echo "=== sweep title=${TITLE} appid=${APP_ID} mode=${CURRENT_MODE} ==="

  if [[ "${GAME_KIND}" == "beyond" ]]; then
    OMFG_LAYER_IMPL="${OMFG_LAYER_IMPL}" \
    OMFG_LAYER_MODE="${CURRENT_MODE}" \
    OMFG_BEYOND_WAIT_SEC="${OMFG_BEYOND_WAIT_SEC:-${DEFAULT_WAIT_SEC}}" \
    "${ROOT_DIR}/scripts/test-steamdeck-beyond-two-souls.sh"
    LOG_PATH="${ROOT_DIR}/${OMFG_LAYER_ARTIFACT_ROOT_REL}/real-games/${SLUG}/${CURRENT_MODE}/omfg.log"
  else
    OMFG_LAYER_IMPL="${OMFG_LAYER_IMPL}" \
    OMFG_LAYER_MODE="${CURRENT_MODE}" \
    OMFG_GAME_WAIT_SEC="${OMFG_GAME_WAIT_SEC:-${DEFAULT_WAIT_SEC}}" \
    "${ROOT_DIR}/scripts/test-steamdeck-steam-game.sh" "${APP_ID}" "${SLUG}" "${TITLE}" "${EXE_REGEX}"
    LOG_PATH="${ROOT_DIR}/${OMFG_LAYER_ARTIFACT_ROOT_REL}/real-games/${SLUG}/${CURRENT_MODE}/omfg.log"
  fi

  summarize_log "${LOG_PATH}"
done

echo
echo "Sweep complete: ${SUMMARY_TSV}"
column -t -s $'\t' "${SUMMARY_TSV}" || cat "${SUMMARY_TSV}"
