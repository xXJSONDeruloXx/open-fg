# Autoperf Program

This repo now has a small **benchmark-driven autoperf loop** for Steam Deck validation.

## Goal

Make pacing / synchronization changes in the Rust Vulkan layer and only keep them when they show a repeatable improvement on real Linux hardware.

## Fast decision subset

The fast subset is intentionally small and representative:

1. `blend`
   - metric: `avgCpuTotalMs`
   - reason: cheap single-FG baseline / pacing sanity check
2. `reproject-blend-default`
   - metric: `avgCpuTotalMs`
   - reason: heavier single-FG path with meaningful GPU work
3. `multi-blend-count3`
   - metric: `avgCpuPerGeneratedFrameMs`
   - reason: preferred visible interpolation path and primary multi-FG focus
4. `adaptive-multi-target180`
   - metric: `avgCpuPerGeneratedFrameMs`
   - reason: LSFG-style target-FPS controller under aggressive output pressure

## Acceptance rule

Current default acceptance rule in `scripts/compare-benchmark-results.py`:

- compare candidate aggregate vs baseline on the decision subset
- reject if any tracked case regresses by more than **0.5%** on its primary metric
- accept only if weighted improvement is at least **0.25%** overall

Weights:

- `blend`: `1.0`
- `reproject-blend-default`: `1.5`
- `multi-blend-count3`: `3.0`
- `adaptive-multi-target180`: `3.0`

This intentionally favors wins on the multi-FG paths.

## Workflow

1. Make a candidate change.
2. Run the fast decision subset several times:
   ```bash
   ./scripts/run-autoperf-loop.sh
   ```
3. Inspect:
   - `artifacts/steamdeck/rust/autoperf/<run-id>/aggregate-summary.txt`
   - `artifacts/steamdeck/rust/autoperf/<run-id>/comparison.txt`
4. If accepted, optionally promote to the full benchmark suite:
   ```bash
   OMFG_AUTOPERF_RUN_FULL_ON_ACCEPT=1 \
   ./scripts/run-autoperf-loop.sh
   ```
5. Commit only accepted improvements.
6. Revert or discard rejected experiments.

## Supporting scripts

- `scripts/run-steamdeck-benchmark-suite.sh`
  - now supports `OMFG_BENCHMARK_PRESET=decision|full|reproject-quality|reproject-disocclusion|optflow-compare|optflow-quality`
  - supports `OMFG_BENCHMARK_ARTIFACT_PREFIX` so repeated runs do not clobber canonical benchmark case artifacts
- `scripts/aggregate-benchmark-results.py`
  - aggregates repeated benchmark runs into mean/stdev summaries
- `scripts/compare-benchmark-results.py`
  - compares baseline vs candidate and emits accept/reject
  - supports matching comparison presets: `decision`, `full`, `reproject-quality`, `optflow-quality`
- `scripts/run-autoperf-loop.sh`
  - orchestrates repeated decision runs, aggregation, comparison, and optional full-suite promotion

### Focused reprojection-quality loop

When tuning reprojection heuristics specifically, run the matching benchmark and compare presets together:

```bash
OMFG_AUTOPERF_BENCHMARK_PRESET=reproject-quality \
OMFG_AUTOPERF_COMPARE_PRESET=reproject-quality \
./scripts/run-autoperf-loop.sh
```

### Focused optflow-quality loop

When comparing optical-flow mode families against the reprojection baseline, or evaluating
the new `optflow-adaptive-blend` and `optflow-multi-blend` modes:

```bash
OMFG_AUTOPERF_BENCHMARK_PRESET=optflow-quality \
OMFG_AUTOPERF_COMPARE_PRESET=optflow-quality \
./scripts/run-autoperf-loop.sh
```

This preset includes:
- `reproject-blend-default`: reprojection baseline
- `optflow-blend-default`: optical-flow v0 with default (wider) search
- `optflow-blend-fast`: optical-flow v0 with tighter fast search (competitive with reprojection on Deck)
- `optflow-adaptive-blend-default`: new adaptive variant (optflow + adaptive current-frame bias)
- `optflow-multi-blend-count2`: new optical-flow-backed multi-FG (2 generated frames, fast profile)

Primary metrics:
- `avgCpuTotalMs` for single-FG cases
- `avgCpuPerGeneratedFrameMs` for multi-FG cases

## Baseline

Default baseline currently points at the pre-autoperf architectural benchmark run:

- `artifacts/steamdeck/rust/benchmark/extended-20260326-204745/`

Override with:

```bash
OMFG_AUTOPERF_BASELINE=/path/to/results-or-directory ./scripts/run-autoperf-loop.sh
```
