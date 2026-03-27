#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
: "${OMFG_LAYER_IMPL:=rust}"
# shellcheck disable=SC1091
source "${ROOT_DIR}/scripts/_omfg_layer_impl.sh"

VKCUBE_COUNT="${OMFG_BENCHMARK_VKCUBE_COUNT:-120}"
TIMEOUT_SEC="${OMFG_BENCHMARK_TIMEOUT_SEC:-30}"
RUN_ID="${OMFG_BENCHMARK_RUN_ID:-$(date +%Y%m%d-%H%M%S)}"
PRESET="${OMFG_BENCHMARK_PRESET:-full}"
CASE_FILTER="${OMFG_BENCHMARK_CASES:-}"
ARTIFACT_PREFIX="${OMFG_BENCHMARK_ARTIFACT_PREFIX:-}"
RESULTS_DIR="${ROOT_DIR}/${OMFG_LAYER_ARTIFACT_ROOT_REL}/benchmark/${RUN_ID}"
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
    export OMFG_LAYER_IMPL="${OMFG_LAYER_IMPL}"
    export OMFG_BENCHMARK=1
    export OMFG_BENCHMARK_LABEL="${name}"
    export OMFG_VKCUBE_COUNT="${VKCUBE_COUNT}"
    export OMFG_VKCUBE_TIMEOUT_SEC="${TIMEOUT_SEC}"
    export OMFG_VKCUBE_ARTIFACT_SUFFIX="${artifact_suffix}"
    export OMFG_VISUAL_HOLD_MS=
    export OMFG_BFI_HOLD_MS=
    export OMFG_BFI_PERIOD=
    for kv in "$@"; do
      export "$kv"
    done
    "${ROOT_DIR}/scripts/test-steamdeck-vkcube.sh"
  )

  local mode=""
  for kv in "$@"; do
    if [[ "${kv}" == OMFG_LAYER_MODE=* ]]; then
      mode="${kv#OMFG_LAYER_MODE=}"
      break
    fi
  done
  if [[ -z "${mode}" ]]; then
    echo "run_case requires OMFG_LAYER_MODE" >&2
    return 1
  fi

  local log_path="${ROOT_DIR}/${OMFG_LAYER_ARTIFACT_ROOT_REL}/vkcube/${mode}-${artifact_suffix}/omfg-vkcube.log"
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
  add_case blend OMFG_LAYER_MODE=blend
  add_case adaptive-blend OMFG_LAYER_MODE=adaptive-blend
  add_case search-blend-r1 OMFG_LAYER_MODE=search-blend OMFG_SEARCH_BLEND_RADIUS=1
  add_case search-blend-r2 OMFG_LAYER_MODE=search-blend OMFG_SEARCH_BLEND_RADIUS=2
  add_case search-adaptive-blend-r1 OMFG_LAYER_MODE=search-adaptive-blend OMFG_SEARCH_BLEND_RADIUS=1
  add_case reproject-blend-default OMFG_LAYER_MODE=reproject-blend OMFG_REPROJECT_SEARCH_RADIUS=2 OMFG_REPROJECT_PATCH_RADIUS=1 OMFG_REPROJECT_CONFIDENCE_SCALE=4.0
  add_case reproject-blend-wide OMFG_LAYER_MODE=reproject-blend OMFG_REPROJECT_SEARCH_RADIUS=3 OMFG_REPROJECT_PATCH_RADIUS=2 OMFG_REPROJECT_CONFIDENCE_SCALE=4.0
  add_case reproject-adaptive-blend-default OMFG_LAYER_MODE=reproject-adaptive-blend OMFG_REPROJECT_SEARCH_RADIUS=2 OMFG_REPROJECT_PATCH_RADIUS=1 OMFG_REPROJECT_CONFIDENCE_SCALE=4.0
  add_case multi-blend-count2 OMFG_LAYER_MODE=multi-blend OMFG_MULTI_BLEND_COUNT=2
  add_case multi-blend-count3 OMFG_LAYER_MODE=multi-blend OMFG_MULTI_BLEND_COUNT=3
  add_case reproject-multi-count2 OMFG_LAYER_MODE=reproject-multi-blend OMFG_MULTI_BLEND_COUNT=2 OMFG_REPROJECT_SEARCH_RADIUS=2 OMFG_REPROJECT_PATCH_RADIUS=1 OMFG_REPROJECT_CONFIDENCE_SCALE=4.0
  add_case reproject-multi-count3 OMFG_LAYER_MODE=reproject-multi-blend OMFG_MULTI_BLEND_COUNT=3 OMFG_REPROJECT_SEARCH_RADIUS=2 OMFG_REPROJECT_PATCH_RADIUS=1 OMFG_REPROJECT_CONFIDENCE_SCALE=4.0
  add_case adaptive-multi-default OMFG_LAYER_MODE=adaptive-multi-blend OMFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=1 OMFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=2 OMFG_ADAPTIVE_MULTI_INTERVAL_THRESHOLD_MS=5.0
  add_case adaptive-multi-target120 OMFG_LAYER_MODE=adaptive-multi-blend OMFG_ADAPTIVE_MULTI_TARGET_FPS=120 OMFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=0 OMFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=2
  add_case adaptive-multi-target150 OMFG_LAYER_MODE=adaptive-multi-blend OMFG_ADAPTIVE_MULTI_TARGET_FPS=150 OMFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=0 OMFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=2
  add_case adaptive-multi-target180 OMFG_LAYER_MODE=adaptive-multi-blend OMFG_ADAPTIVE_MULTI_TARGET_FPS=180 OMFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=0 OMFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=2
  add_case reproject-adaptive-multi-default OMFG_LAYER_MODE=reproject-adaptive-multi-blend OMFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=1 OMFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=2 OMFG_ADAPTIVE_MULTI_INTERVAL_THRESHOLD_MS=5.0 OMFG_REPROJECT_SEARCH_RADIUS=2 OMFG_REPROJECT_PATCH_RADIUS=1 OMFG_REPROJECT_CONFIDENCE_SCALE=4.0
  add_case reproject-adaptive-multi-target180 OMFG_LAYER_MODE=reproject-adaptive-multi-blend OMFG_ADAPTIVE_MULTI_TARGET_FPS=180 OMFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=0 OMFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=2 OMFG_REPROJECT_SEARCH_RADIUS=2 OMFG_REPROJECT_PATCH_RADIUS=1 OMFG_REPROJECT_CONFIDENCE_SCALE=4.0
}

run_preset_decision() {
  add_case blend OMFG_LAYER_MODE=blend
  add_case reproject-blend-default OMFG_LAYER_MODE=reproject-blend OMFG_REPROJECT_SEARCH_RADIUS=2 OMFG_REPROJECT_PATCH_RADIUS=1 OMFG_REPROJECT_CONFIDENCE_SCALE=4.0
  add_case multi-blend-count3 OMFG_LAYER_MODE=multi-blend OMFG_MULTI_BLEND_COUNT=3
  add_case adaptive-multi-target180 OMFG_LAYER_MODE=adaptive-multi-blend OMFG_ADAPTIVE_MULTI_TARGET_FPS=180 OMFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=0 OMFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=2
}

run_preset_reproject_quality() {
  add_case reproject-blend-default OMFG_LAYER_MODE=reproject-blend OMFG_REPROJECT_SEARCH_RADIUS=2 OMFG_REPROJECT_PATCH_RADIUS=1 OMFG_REPROJECT_CONFIDENCE_SCALE=4.0
  add_case reproject-blend-no-gradient OMFG_LAYER_MODE=reproject-blend OMFG_REPROJECT_SEARCH_RADIUS=2 OMFG_REPROJECT_PATCH_RADIUS=1 OMFG_REPROJECT_CONFIDENCE_SCALE=4.0 OMFG_REPROJECT_GRADIENT_CONFIDENCE_WEIGHT=0.0
  add_case reproject-blend-no-chroma OMFG_LAYER_MODE=reproject-blend OMFG_REPROJECT_SEARCH_RADIUS=2 OMFG_REPROJECT_PATCH_RADIUS=1 OMFG_REPROJECT_CONFIDENCE_SCALE=4.0 OMFG_REPROJECT_CHROMA_WEIGHT=0.0
  add_case reproject-blend-no-ambiguity OMFG_LAYER_MODE=reproject-blend OMFG_REPROJECT_SEARCH_RADIUS=2 OMFG_REPROJECT_PATCH_RADIUS=1 OMFG_REPROJECT_CONFIDENCE_SCALE=4.0 OMFG_REPROJECT_AMBIGUITY_SCALE=0.0
  add_case reproject-multi-count3-default OMFG_LAYER_MODE=reproject-multi-blend OMFG_MULTI_BLEND_COUNT=3 OMFG_REPROJECT_SEARCH_RADIUS=2 OMFG_REPROJECT_PATCH_RADIUS=1 OMFG_REPROJECT_CONFIDENCE_SCALE=4.0
  add_case reproject-multi-count3-no-ambiguity OMFG_LAYER_MODE=reproject-multi-blend OMFG_MULTI_BLEND_COUNT=3 OMFG_REPROJECT_SEARCH_RADIUS=2 OMFG_REPROJECT_PATCH_RADIUS=1 OMFG_REPROJECT_CONFIDENCE_SCALE=4.0 OMFG_REPROJECT_AMBIGUITY_SCALE=0.0
  add_case reproject-adaptive-multi-target180-default OMFG_LAYER_MODE=reproject-adaptive-multi-blend OMFG_ADAPTIVE_MULTI_TARGET_FPS=180 OMFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=0 OMFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=2 OMFG_REPROJECT_SEARCH_RADIUS=2 OMFG_REPROJECT_PATCH_RADIUS=1 OMFG_REPROJECT_CONFIDENCE_SCALE=4.0
  add_case reproject-adaptive-multi-target180-no-ambiguity OMFG_LAYER_MODE=reproject-adaptive-multi-blend OMFG_ADAPTIVE_MULTI_TARGET_FPS=180 OMFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=0 OMFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=2 OMFG_REPROJECT_SEARCH_RADIUS=2 OMFG_REPROJECT_PATCH_RADIUS=1 OMFG_REPROJECT_CONFIDENCE_SCALE=4.0 OMFG_REPROJECT_AMBIGUITY_SCALE=0.0
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
  reproject-quality)
    run_preset_reproject_quality
    ;;
  *)
    echo "Unknown benchmark preset: ${PRESET}" >&2
    exit 1
    ;;
esac

echo "Benchmark summaries saved under ${RESULTS_DIR}"
echo "CSV: ${CSV_PATH}"
