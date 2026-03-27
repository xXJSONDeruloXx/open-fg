#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
: "${PPFG_LAYER_IMPL:=rust}"
# shellcheck disable=SC1091
source "${ROOT_DIR}/scripts/_ppfg_layer_impl.sh"

VKCUBE_COUNT="${PPFG_BENCHMARK_VKCUBE_COUNT:-120}"
TIMEOUT_SEC="${PPFG_BENCHMARK_TIMEOUT_SEC:-30}"
RUN_ID="${PPFG_BENCHMARK_RUN_ID:-$(date +%Y%m%d-%H%M%S)}"
RESULTS_DIR="${ROOT_DIR}/${PPFG_LAYER_ARTIFACT_ROOT_REL}/benchmark/${RUN_ID}"
CSV_PATH="${RESULTS_DIR}/results.csv"

mkdir -p "${RESULTS_DIR}"
python3 "${ROOT_DIR}/scripts/summarize-benchmark-log.py" --header > "${CSV_PATH}"

run_case() {
  local name="$1"
  shift

  (
    export PPFG_LAYER_IMPL="${PPFG_LAYER_IMPL}"
    export PPFG_BENCHMARK=1
    export PPFG_BENCHMARK_LABEL="${name}"
    export PPFG_VKCUBE_COUNT="${VKCUBE_COUNT}"
    export PPFG_VKCUBE_TIMEOUT_SEC="${TIMEOUT_SEC}"
    export PPFG_VKCUBE_ARTIFACT_SUFFIX="benchmark-${name}"
    export PPFG_VISUAL_HOLD_MS=
    export PPFG_BFI_HOLD_MS=
    export PPFG_BFI_PERIOD=
    for kv in "$@"; do
      export "$kv"
    done
    "${ROOT_DIR}/scripts/test-steamdeck-vkcube.sh"
  )

  local mode=""
  for kv in "$@"; do
    if [[ "${kv}" == PPFG_LAYER_MODE=* ]]; then
      mode="${kv#PPFG_LAYER_MODE=}"
      break
    fi
  done
  if [[ -z "${mode}" ]]; then
    echo "run_case requires PPFG_LAYER_MODE" >&2
    return 1
  fi

  local log_path="${ROOT_DIR}/${PPFG_LAYER_ARTIFACT_ROOT_REL}/vkcube/${mode}-benchmark-${name}/ppfg-vkcube.log"
  python3 "${ROOT_DIR}/scripts/summarize-benchmark-log.py" "${log_path}" | tee -a "${RESULTS_DIR}/summary.txt"
  python3 "${ROOT_DIR}/scripts/summarize-benchmark-log.py" --csv "${log_path}" >> "${CSV_PATH}"
}

run_case blend PPFG_LAYER_MODE=blend
run_case adaptive-blend PPFG_LAYER_MODE=adaptive-blend
run_case search-blend-r1 PPFG_LAYER_MODE=search-blend PPFG_SEARCH_BLEND_RADIUS=1
run_case reproject-blend-default PPFG_LAYER_MODE=reproject-blend PPFG_REPROJECT_SEARCH_RADIUS=2 PPFG_REPROJECT_PATCH_RADIUS=1 PPFG_REPROJECT_CONFIDENCE_SCALE=4.0
run_case multi-blend-count2 PPFG_LAYER_MODE=multi-blend PPFG_MULTI_BLEND_COUNT=2
run_case multi-blend-count3 PPFG_LAYER_MODE=multi-blend PPFG_MULTI_BLEND_COUNT=3

echo "Benchmark summaries saved under ${RESULTS_DIR}"
echo "CSV: ${CSV_PATH}"
