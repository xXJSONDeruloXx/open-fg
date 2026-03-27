#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
: "${PPFG_LAYER_IMPL:=rust}"
# shellcheck disable=SC1091
source "${ROOT_DIR}/scripts/_ppfg_layer_impl.sh"

VKCUBE_COUNT="${PPFG_BENCHMARK_VKCUBE_COUNT:-120}"
TIMEOUT_SEC="${PPFG_BENCHMARK_TIMEOUT_SEC:-30}"
RUN_ID="${PPFG_BENCHMARK_RUN_ID:-$(date +%Y%m%d-%H%M%S)}"
PRESET="${PPFG_BENCHMARK_PRESET:-full}"
CASE_FILTER="${PPFG_BENCHMARK_CASES:-}"
ARTIFACT_PREFIX="${PPFG_BENCHMARK_ARTIFACT_PREFIX:-}"
RESULTS_DIR="${ROOT_DIR}/${PPFG_LAYER_ARTIFACT_ROOT_REL}/benchmark/${RUN_ID}"
CSV_PATH="${RESULTS_DIR}/results.csv"

mkdir -p "${RESULTS_DIR}"
python3 "${ROOT_DIR}/scripts/summarize-benchmark-log.py" --header > "${CSV_PATH}"

run_case() {
  local name="$1"
  shift

  local artifact_suffix="benchmark-${name}"
  if [[ -n "${ARTIFACT_PREFIX}" ]]; then
    artifact_suffix="${ARTIFACT_PREFIX}-${name}"
  fi

  (
    export PPFG_LAYER_IMPL="${PPFG_LAYER_IMPL}"
    export PPFG_BENCHMARK=1
    export PPFG_BENCHMARK_LABEL="${name}"
    export PPFG_VKCUBE_COUNT="${VKCUBE_COUNT}"
    export PPFG_VKCUBE_TIMEOUT_SEC="${TIMEOUT_SEC}"
    export PPFG_VKCUBE_ARTIFACT_SUFFIX="${artifact_suffix}"
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

  local log_path="${ROOT_DIR}/${PPFG_LAYER_ARTIFACT_ROOT_REL}/vkcube/${mode}-${artifact_suffix}/ppfg-vkcube.log"
  python3 "${ROOT_DIR}/scripts/summarize-benchmark-log.py" "${log_path}" | tee -a "${RESULTS_DIR}/summary.txt"
  python3 "${ROOT_DIR}/scripts/summarize-benchmark-log.py" --csv "${log_path}" >> "${CSV_PATH}"
}

should_run_case() {
  local label="$1"
  if [[ -z "${CASE_FILTER}" ]]; then
    return 0
  fi

  IFS=',' read -r -a filters <<< "${CASE_FILTER}"
  for filter in "${filters[@]}"; do
    if [[ "${filter}" == "${label}" ]]; then
      return 0
    fi
  done
  return 1
}

add_case() {
  local name="$1"
  shift
  if should_run_case "${name}"; then
    run_case "${name}" "$@"
  fi
}

run_preset_full() {
  add_case blend PPFG_LAYER_MODE=blend
  add_case adaptive-blend PPFG_LAYER_MODE=adaptive-blend
  add_case search-blend-r1 PPFG_LAYER_MODE=search-blend PPFG_SEARCH_BLEND_RADIUS=1
  add_case search-blend-r2 PPFG_LAYER_MODE=search-blend PPFG_SEARCH_BLEND_RADIUS=2
  add_case search-adaptive-blend-r1 PPFG_LAYER_MODE=search-adaptive-blend PPFG_SEARCH_BLEND_RADIUS=1
  add_case reproject-blend-default PPFG_LAYER_MODE=reproject-blend PPFG_REPROJECT_SEARCH_RADIUS=2 PPFG_REPROJECT_PATCH_RADIUS=1 PPFG_REPROJECT_CONFIDENCE_SCALE=4.0
  add_case reproject-blend-wide PPFG_LAYER_MODE=reproject-blend PPFG_REPROJECT_SEARCH_RADIUS=3 PPFG_REPROJECT_PATCH_RADIUS=2 PPFG_REPROJECT_CONFIDENCE_SCALE=4.0
  add_case reproject-adaptive-blend-default PPFG_LAYER_MODE=reproject-adaptive-blend PPFG_REPROJECT_SEARCH_RADIUS=2 PPFG_REPROJECT_PATCH_RADIUS=1 PPFG_REPROJECT_CONFIDENCE_SCALE=4.0
  add_case multi-blend-count2 PPFG_LAYER_MODE=multi-blend PPFG_MULTI_BLEND_COUNT=2
  add_case multi-blend-count3 PPFG_LAYER_MODE=multi-blend PPFG_MULTI_BLEND_COUNT=3
  add_case adaptive-multi-default PPFG_LAYER_MODE=adaptive-multi-blend PPFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=1 PPFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=2 PPFG_ADAPTIVE_MULTI_INTERVAL_THRESHOLD_MS=5.0
  add_case adaptive-multi-target120 PPFG_LAYER_MODE=adaptive-multi-blend PPFG_ADAPTIVE_MULTI_TARGET_FPS=120 PPFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=0 PPFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=2
  add_case adaptive-multi-target150 PPFG_LAYER_MODE=adaptive-multi-blend PPFG_ADAPTIVE_MULTI_TARGET_FPS=150 PPFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=0 PPFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=2
  add_case adaptive-multi-target180 PPFG_LAYER_MODE=adaptive-multi-blend PPFG_ADAPTIVE_MULTI_TARGET_FPS=180 PPFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=0 PPFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=2
}

run_preset_decision() {
  add_case blend PPFG_LAYER_MODE=blend
  add_case reproject-blend-default PPFG_LAYER_MODE=reproject-blend PPFG_REPROJECT_SEARCH_RADIUS=2 PPFG_REPROJECT_PATCH_RADIUS=1 PPFG_REPROJECT_CONFIDENCE_SCALE=4.0
  add_case multi-blend-count3 PPFG_LAYER_MODE=multi-blend PPFG_MULTI_BLEND_COUNT=3
  add_case adaptive-multi-target180 PPFG_LAYER_MODE=adaptive-multi-blend PPFG_ADAPTIVE_MULTI_TARGET_FPS=180 PPFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=0 PPFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=2
}

echo "Running benchmark preset=${PRESET} run_id=${RUN_ID} results_dir=${RESULTS_DIR}"
if [[ -n "${CASE_FILTER}" ]]; then
  echo "Filtering cases to: ${CASE_FILTER}"
fi

case "${PRESET}" in
  full)
    run_preset_full
    ;;
  decision)
    run_preset_decision
    ;;
  *)
    echo "Unknown benchmark preset: ${PRESET}" >&2
    exit 1
    ;;
esac

echo "Benchmark summaries saved under ${RESULTS_DIR}"
echo "CSV: ${CSV_PATH}"
