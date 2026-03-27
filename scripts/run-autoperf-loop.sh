#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
: "${PPFG_LAYER_IMPL:=rust}"
# shellcheck disable=SC1091
source "${ROOT_DIR}/scripts/_ppfg_layer_impl.sh"

RUN_ID="${PPFG_AUTOPERF_RUN_ID:-$(date +%Y%m%d-%H%M%S)}"
REPETITIONS="${PPFG_AUTOPERF_REPETITIONS:-3}"
BENCHMARK_PRESET="${PPFG_AUTOPERF_BENCHMARK_PRESET:-decision}"
COMPARE_PRESET="${PPFG_AUTOPERF_COMPARE_PRESET:-decision}"
RUN_FULL_ON_ACCEPT="${PPFG_AUTOPERF_RUN_FULL_ON_ACCEPT:-0}"
BASELINE_INPUT="${PPFG_AUTOPERF_BASELINE:-${ROOT_DIR}/${PPFG_LAYER_ARTIFACT_ROOT_REL}/benchmark/extended-20260326-204745}"
AUTOPERF_DIR="${ROOT_DIR}/${PPFG_LAYER_ARTIFACT_ROOT_REL}/autoperf/${RUN_ID}"
AGGREGATE_CSV="${AUTOPERF_DIR}/aggregate.csv"
AGGREGATE_SUMMARY="${AUTOPERF_DIR}/aggregate-summary.txt"
COMPARISON_TXT="${AUTOPERF_DIR}/comparison.txt"
COMPARISON_JSON="${AUTOPERF_DIR}/comparison.json"
FULL_COMPARISON_TXT="${AUTOPERF_DIR}/promoted-full-comparison.txt"
FULL_COMPARISON_JSON="${AUTOPERF_DIR}/promoted-full-comparison.json"
RUNS_LIST="${AUTOPERF_DIR}/runs.txt"
MANIFEST_TXT="${AUTOPERF_DIR}/manifest.txt"

mkdir -p "${AUTOPERF_DIR}"

{
  echo "runId=${RUN_ID}"
  echo "layerImpl=${PPFG_LAYER_IMPL}"
  echo "benchmarkPreset=${BENCHMARK_PRESET}"
  echo "comparePreset=${COMPARE_PRESET}"
  echo "repetitions=${REPETITIONS}"
  echo "baseline=${BASELINE_INPUT}"
  echo "runFullOnAccept=${RUN_FULL_ON_ACCEPT}"
  echo "benchmarkCount=${PPFG_BENCHMARK_VKCUBE_COUNT:-120}"
  echo "benchmarkTimeoutSec=${PPFG_BENCHMARK_TIMEOUT_SEC:-30}"
} > "${MANIFEST_TXT}"

run_dirs=()
for iteration in $(seq 1 "${REPETITIONS}"); do
  iter_name=$(printf 'iter%02d' "${iteration}")
  bench_run_id="autoperf-${RUN_ID}-${iter_name}"
  artifact_prefix="autoperf-${RUN_ID}-${iter_name}"
  bench_dir="${ROOT_DIR}/${PPFG_LAYER_ARTIFACT_ROOT_REL}/benchmark/${bench_run_id}"

  echo "=== autoperf iteration ${iteration}/${REPETITIONS}: ${bench_run_id} ==="
  PPFG_BENCHMARK_PRESET="${BENCHMARK_PRESET}" \
  PPFG_BENCHMARK_RUN_ID="${bench_run_id}" \
  PPFG_BENCHMARK_ARTIFACT_PREFIX="${artifact_prefix}" \
  "${ROOT_DIR}/scripts/run-steamdeck-benchmark-suite.sh"

  echo "${bench_dir}" >> "${RUNS_LIST}"
  run_dirs+=("${bench_dir}")
done

python3 "${ROOT_DIR}/scripts/aggregate-benchmark-results.py" \
  --csv-out "${AGGREGATE_CSV}" \
  --summary-out "${AGGREGATE_SUMMARY}" \
  "${run_dirs[@]}" | tee "${AUTOPERF_DIR}/aggregate.stdout"

compare_status=0
python3 "${ROOT_DIR}/scripts/compare-benchmark-results.py" \
  --preset "${COMPARE_PRESET}" \
  --json \
  "${BASELINE_INPUT}" \
  "${AGGREGATE_CSV}" > "${COMPARISON_JSON}" || compare_status=$?
python3 "${ROOT_DIR}/scripts/compare-benchmark-results.py" \
  --preset "${COMPARE_PRESET}" \
  "${BASELINE_INPUT}" \
  "${AGGREGATE_CSV}" | tee "${COMPARISON_TXT}" || compare_status=$?

if [[ "${compare_status}" -ne 0 && "${compare_status}" -ne 2 ]]; then
  exit "${compare_status}"
fi

accepted=0
if [[ "${compare_status}" -eq 0 ]]; then
  accepted=1
fi

echo "accepted=${accepted}" >> "${MANIFEST_TXT}"

if [[ "${accepted}" -eq 1 && "${RUN_FULL_ON_ACCEPT}" == "1" ]]; then
  full_run_id="autoperf-${RUN_ID}-full"
  full_run_dir="${ROOT_DIR}/${PPFG_LAYER_ARTIFACT_ROOT_REL}/benchmark/${full_run_id}"
  echo "=== autoperf promoted full benchmark: ${full_run_id} ==="
  PPFG_BENCHMARK_PRESET=full \
  PPFG_BENCHMARK_RUN_ID="${full_run_id}" \
  PPFG_BENCHMARK_ARTIFACT_PREFIX="autoperf-${RUN_ID}-full" \
  "${ROOT_DIR}/scripts/run-steamdeck-benchmark-suite.sh"
  echo "promotedFullRun=${full_run_dir}" >> "${MANIFEST_TXT}"

  full_compare_status=0
  python3 "${ROOT_DIR}/scripts/compare-benchmark-results.py" \
    --preset full \
    --json \
    "${BASELINE_INPUT}" \
    "${full_run_dir}" > "${FULL_COMPARISON_JSON}" || full_compare_status=$?
  python3 "${ROOT_DIR}/scripts/compare-benchmark-results.py" \
    --preset full \
    "${BASELINE_INPUT}" \
    "${full_run_dir}" | tee "${FULL_COMPARISON_TXT}" || full_compare_status=$?
  if [[ "${full_compare_status}" -eq 0 ]]; then
    echo "promotedFullAccepted=1" >> "${MANIFEST_TXT}"
  elif [[ "${full_compare_status}" -eq 2 ]]; then
    echo "promotedFullAccepted=0" >> "${MANIFEST_TXT}"
  else
    echo "promotedFullComparisonError=${full_compare_status}" >> "${MANIFEST_TXT}"
  fi
fi

echo "Autoperf artifacts saved under ${AUTOPERF_DIR}"
