# OMFG debug / observability plan

This document defines the first debug-view architecture for the Rust OMFG layer.

## Goal

Make the reprojection-backed path inspectable enough to tune intentionally, not just by looking at the final blended frame.

The design goal is explicitly inspired by the way the FidelityFX SDK treats frame-generation debug output: intermediate decisions should be visible in-frame and easy to toggle while keeping the runtime path close to the real generation path.

## Scope for v0

Phase 1 only needs to support the reprojection-backed modes:
- `reproject-blend`
- `reproject-adaptive-blend`
- `reproject-multi-blend`
- `reproject-adaptive-multi-blend`

Non-reprojection modes can keep debug output disabled for now.

## Recommended architecture

### 1. Mode / env plumbing

Add a new debug-view selector in Rust config/env parsing.

Recommended envs:
- `OMFG_DEBUG_VIEW=off|motion|confidence|ambiguity|disocclusion|hole-fill|fallback`
- `OMFG_DEBUG_VIEW_OPACITY=1.0` (optional later if overlay mode is added)
- `OMFG_DEBUG_VIEW_SCALE=1.0` (optional later for vector magnitudes)

For v0, keep behavior simple:
- `off` = current normal rendering
- any other value = replace generated output with the selected diagnostic view

Using full replacement first is intentional:
- easiest to reason about
- easiest to benchmark against non-debug runs
- avoids cluttered overlay composition in the first landing

### 2. Shader output strategy

Keep the existing generation path and shader entrypoint, but refactor the reprojection block so it computes reusable diagnostic values.

Recommended internal data to expose from the reprojection section:
- selected half-offset / motion vector
- zero-motion error
- best-match error
- second-best error
- confidence after all weighting
- ambiguity suppression factor
- disocclusion estimate
- hole-fill contribution weight
- source-selection / fallback mix weight

Recommended shader-side struct pattern:
- `ReprojectDiagnostics` or equivalent
- filled once during reprojection search
- normal output path uses it for blending
- debug path maps it to a false-color or grayscale presentation

### 3. First debug views

#### `motion`
Show selected reprojection offset.

Recommended encoding:
- red = normalized X offset
- green = normalized Y offset
- blue = motion magnitude

This should make it obvious when the search locks onto flat-region junk or repeated texture patterns.

#### `confidence`
Show final confidence after all weighting.

Recommended encoding:
- grayscale or heatmap
- black = distrust reprojection
- white = strong trust

#### `ambiguity`
Show how tied the best and second-best candidates are.

Recommended encoding:
- black = clear winner
- bright = ambiguous / multiple candidates nearly tied

This is useful for understanding where the ambiguity suppression term is helping or overfiring.

#### `disocclusion`
Show the disocclusion estimate.

Recommended encoding:
- black = low disocclusion risk
- bright = likely uncover / occlusion boundary / mismatch zone

#### `hole-fill`
Show the hole-fill contribution weight.

Recommended encoding:
- black = no neighborhood fill used
- bright = heavy hole-fill fallback

#### `fallback`
Show where the final color came from.

Recommended encoding:
- red = mostly original-frame fallback
- green = mostly reprojection-driven result
- blue = meaningful hole-fill involvement

This should be the most immediately actionable “what actually happened here?” view.

## Multi-FG behavior

For multi-FG reprojection modes, the same debug selector should apply to each generated frame.

That means:
- no separate debug mode per generated frame in v0
- the chosen view is rendered for every generated frame in the sequence
- alpha/time position still matters when the view depends on blend position

This keeps the implementation small and makes per-generated-frame issues visible under the existing sequencing path.

## Logging and artifact plan

Phase 1 should reuse the current artifact pipeline:
- `scripts/test-steamdeck-vkcube.sh`
- per-run artifact suffixes
- `omfg-vkcube.log`
- benchmark CSV / summary output where applicable

Recommended run naming pattern:
- `reproject-blend-debug-motion`
- `reproject-blend-debug-confidence`
- `reproject-multi-debug-fallback`

Recommended script passthrough additions:
- export `OMFG_DEBUG_VIEW`
- export optional future knobs like `OMFG_DEBUG_VIEW_OPACITY`

For visual validation on Deck:
- use the existing generated-frame path directly
- combine with `OMFG_VISUAL_HOLD_MS` when manual inspection is useful
- treat screenshot automation as a later convenience, not a blocker for landing the first debug views

## Benchmark strategy

Debug views are not a shipping quality mode, so the benchmark question is narrowly:

> how much overhead does observability add relative to the same reprojection path with debug disabled?

Recommended benchmark approach:
- add a small `debug-overhead` or similar preset later
- compare:
  - `reproject-blend-default`
  - `reproject-blend-debug-motion`
  - `reproject-blend-debug-confidence`
  - `reproject-multi-count3-default`
  - `reproject-multi-count3-debug-fallback`

Success criteria for v0:
- debug view works in local / Docker / Deck validation
- logs clearly identify which debug view ran
- overhead is measured and documented
- no regressions when `OMFG_DEBUG_VIEW=off`

## Validation plan

### Local
- extend Rust tests for env parsing / push-constant propagation
- ensure non-debug behavior is unchanged when debug view is `off`

### Docker
- build shaders and full Rust layer normally
- confirm no shader compilation regressions

### Steam Deck
- smoke-test at least:
  - `reproject-blend`
  - `reproject-multi-blend`
- use artifact suffixes to separate each debug view
- use visual hold on targeted cases when manual inspection is needed

## Recommended landing order

1. env/config plumbing
2. push-constant / shader plumbing
3. `motion` view
4. `confidence` + `ambiguity` views
5. `disocclusion` + `hole-fill` + `fallback` views
6. script passthrough + docs + benchmark preset

## Non-goals for v0

Do not require all of these in the first landing:
- composited overlay UI
- per-pixel textual labels
- screenshot automation
- non-reprojection debug support
- pacing-marker bars equivalent to FSR right away

Those can come later once the core analytical views are useful.
