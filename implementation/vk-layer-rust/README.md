# Vulkan layer Rust port

Rust port of the current explicit Vulkan post-process frame-generation layer MVP.

## Current goal

Reach feature parity with the C++ MVP while building a safer foundation for future work.

Current Rust capability set:
- explicit Vulkan layer ABI exports
- instance / device / swapchain / present interception
- parity/runtime utility modes:
  - `passthrough`
  - `clear`
  - `bfi`
  - `copy`
  - `history-copy`
- shader-based generated backend modes:
  - `blend`
  - `adaptive-blend`
  - `search-blend`
  - `search-adaptive-blend`
  - `reproject-blend`
  - `reproject-adaptive-blend`
  - `multi-blend`
  - `adaptive-multi-blend`
  - `reproject-multi-blend`
  - `reproject-adaptive-multi-blend`
- LSFG-style target-FPS adaptive controller for `adaptive-multi-blend` and `reproject-adaptive-multi-blend`
- testable swapchain mutation + present sequencing logic
- expandable regression harness for future interpolation work

## Test strategy

### Fast local tests
From this directory:

```bash
cargo test
```

This currently covers:
- mode parsing aliases
- swapchain mutation policy
- present ordering semantics
- generated-frame accounting
- clear/bfi/copy/history and blend/adaptive-blend/search-blend/search-adaptive-blend/reproject-blend/reproject-adaptive-blend/multi-blend/adaptive-multi-blend/reproject-multi-blend/reproject-adaptive-multi-blend policy semantics
- target-FPS adaptive multi-FG controller logic
- pure Rust motion-search / reprojection heuristic tests
- dispatch-key extraction helper
- exported layer enumeration/proc-address plumbing
- loader negotiation ABI

### Linux/x86_64 build + test in Docker
From project root:

```bash
OMFG_LAYER_IMPL=rust ./scripts/build-linux-amd64.sh
```

The Rust crate vendors its dependencies under `implementation/vk-layer-rust/vendor/`, so the Linux builder can run offline/reproducibly.
The Docker builder also recompiles the GLSL shaders via `scripts/compile-rust-shaders.sh`, so the SPIR-V artifacts are now reproducible as part of the Linux build path.

That runs Rust tests inside the Linux builder container and emits:

- `build/linux-amd64/vk-layer-rust/out/libVkLayer_OMFG_rust.so`
- `build/linux-amd64/vk-layer-rust/out/VkLayer_OMFG_rust.json`

## Steam Deck flow

### Deploy
```bash
export STEAMDECK_PASS='...'
OMFG_LAYER_IMPL=rust ./scripts/deploy-steamdeck-layer.sh
```

### Smoke test with vkcube
```bash
export STEAMDECK_PASS='...'
export OMFG_LAYER_IMPL=rust
export OMFG_LAYER_MODE=passthrough
./scripts/test-steamdeck-vkcube.sh

export OMFG_LAYER_MODE=history-copy
./scripts/test-steamdeck-vkcube.sh

export OMFG_LAYER_MODE=bfi
export OMFG_BFI_PERIOD=1
./scripts/test-steamdeck-vkcube.sh

export OMFG_LAYER_MODE=blend
./scripts/test-steamdeck-vkcube.sh

export OMFG_LAYER_MODE=adaptive-blend
./scripts/test-steamdeck-vkcube.sh

export OMFG_LAYER_MODE=search-blend
./scripts/test-steamdeck-vkcube.sh

export OMFG_LAYER_MODE=search-adaptive-blend
./scripts/test-steamdeck-vkcube.sh

export OMFG_LAYER_MODE=reproject-blend
./scripts/test-steamdeck-vkcube.sh

export OMFG_LAYER_MODE=reproject-adaptive-blend
./scripts/test-steamdeck-vkcube.sh

export OMFG_LAYER_MODE=reproject-multi-blend
./scripts/test-steamdeck-vkcube.sh

export OMFG_LAYER_MODE=reproject-adaptive-multi-blend
./scripts/test-steamdeck-vkcube.sh

export OMFG_LAYER_MODE=multi-blend
./scripts/test-steamdeck-vkcube.sh

# Optional higher-count multi-FG experiments.
# The layer now auto-expands swapchain image headroom for larger counts,
# capped by OMFG_MULTI_SWAPCHAIN_MAX_GENERATED_FRAMES (default: 32).
export OMFG_MULTI_BLEND_COUNT=10
./scripts/test-steamdeck-vkcube.sh

export OMFG_LAYER_MODE=reproject-multi-blend
export OMFG_MULTI_BLEND_COUNT=6
./scripts/test-steamdeck-vkcube.sh

export OMFG_LAYER_MODE=adaptive-multi-blend
./scripts/test-steamdeck-vkcube.sh

export OMFG_LAYER_MODE=reproject-adaptive-multi-blend
./scripts/test-steamdeck-vkcube.sh

# Optional higher-count adaptive multi-FG experiment.
export OMFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=1
export OMFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=6
export OMFG_ADAPTIVE_MULTI_INTERVAL_THRESHOLD_MS=1.0
./scripts/test-steamdeck-vkcube.sh

# Optional LSFG-style target-FPS controller for adaptive multi-FG modes.
export OMFG_ADAPTIVE_MULTI_TARGET_FPS=120
export OMFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=0
export OMFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=2
./scripts/test-steamdeck-vkcube.sh

# Optional present timing / pacing instrumentation
export OMFG_PRESENT_TIMING=1
export OMFG_PRESENT_WAIT=1
export OMFG_PRESENT_WAIT_TIMEOUT_NS=5000000000
./scripts/test-steamdeck-vkcube.sh

# Optional reprojection quality tuning
export OMFG_LAYER_MODE=reproject-multi-blend
export OMFG_REPROJECT_DISOCCLUSION_SCALE=2.0
export OMFG_REPROJECT_HOLE_FILL_STRENGTH=0.85
export OMFG_REPROJECT_HOLE_FILL_RADIUS=2
export OMFG_REPROJECT_AMBIGUITY_SCALE=6.0
./scripts/test-steamdeck-vkcube.sh
```

### Full regression suite
```bash
export STEAMDECK_PASS='...'
export OMFG_LAYER_IMPL=rust
./scripts/run-layer-regression-suite.sh
```

### Advanced Steam Deck validation
This extends the normal smoke suite with long FIFO and IMMEDIATE runs for the stronger motion-aware single-FG modes and the newer reprojection-backed multi-FG modes.

```bash
export STEAMDECK_PASS='...'
export OMFG_LAYER_IMPL=rust
./scripts/run-advanced-steamdeck-validation.sh
```

### Target-FPS adaptive multi-FG validation
This exercises the LSFG-style target-FPS controller on the Steam Deck for both `adaptive-multi-blend` and `reproject-adaptive-multi-blend`, including fractional target cases.

```bash
export STEAMDECK_PASS='...'
export OMFG_LAYER_IMPL=rust
./scripts/run-target-fps-steamdeck-validation.sh
```

### Present timing validation
This exercises the present-id / present-wait timing instrumentation on the Steam Deck.

```bash
export STEAMDECK_PASS='...'
export OMFG_LAYER_IMPL=rust
./scripts/run-present-timing-steamdeck-validation.sh
```

### BFI validation
This validates black-frame insertion behavior, including a reduced-cadence `OMFG_BFI_PERIOD=2` case.

```bash
export STEAMDECK_PASS='...'
export OMFG_LAYER_IMPL=rust
./scripts/run-bfi-steamdeck-validation.sh
```

### Display info capture
This records current panel/connector/display-capability information from the Linux target.

```bash
export STEAMDECK_PASS='...'
./scripts/collect-steamdeck-display-info.sh bfi-validation
```

### Benchmark suite
This runs the Steam Deck benchmark matrix and writes per-run CSV/summary artifacts.

```bash
export STEAMDECK_PASS='...'
export OMFG_LAYER_IMPL=rust
./scripts/run-steamdeck-benchmark-suite.sh

# Fast decision subset only
OMFG_BENCHMARK_PRESET=decision ./scripts/run-steamdeck-benchmark-suite.sh

# Focused reprojection-quality ablation preset
OMFG_BENCHMARK_PRESET=reproject-quality ./scripts/run-steamdeck-benchmark-suite.sh
```

### Multi-count sweep
This runs a `multi-blend` multiplier sweep and records how far the current architecture scales.

```bash
export STEAMDECK_PASS='...'
export OMFG_LAYER_IMPL=rust
./scripts/run-steamdeck-multi-count-sweep.sh
```

### Autoperf loop
This repeatedly runs the fast decision subset, aggregates the results, compares them against a baseline, and can optionally promote winners to the full benchmark suite.

```bash
export STEAMDECK_PASS='...'
export OMFG_LAYER_IMPL=rust
./scripts/run-autoperf-loop.sh

# Optional full-suite promotion on acceptance
OMFG_AUTOPERF_RUN_FULL_ON_ACCEPT=1 ./scripts/run-autoperf-loop.sh

# Focused reprojection-quality loop
OMFG_AUTOPERF_BENCHMARK_PRESET=reproject-quality \
OMFG_AUTOPERF_COMPARE_PRESET=reproject-quality \
./scripts/run-autoperf-loop.sh
```

See `experiments/program.md` for the current subset, weights, and accept/reject rules.

### Future backend planning
For the broader roadmap beyond the current Rust mainline, see:
- `docs/future-backends.md`
- `docs/rust-feature-roadmap.md`
- `docs/debug-observability-plan.md`
- `docs/optical-flow-v0-plan.md`

Those documents record how:
- FSR3-style analytical FG concepts fit the current Linux post-process mainline
- the next recommended implementation order is debug views first, then hardware-agnostic optical flow, then broader quality improvements and pacing follow-through
- the first debug / observability landing should be structured and validated
- the first hardware-agnostic optical-flow landing should be benchmarked against the current reprojection baseline
- `RIFE` / `rife-ncnn-vulkan` fit as a quality oracle and later optional experimental backend
- NVIDIA Optical Flow / FRUC fit as a vendor-specific acceleration branch
- FSR4-style ML fits only later or behind an architecture pivot

## Design notes

The Rust port intentionally separates:
- **pure policy logic** in `src/config.rs` and `src/planner.rs`
- **unsafe Vulkan ABI/runtime glue** in `src/lib.rs`
- **loader-specific structs** in `src/layer_defs.rs`
- **precompiled test shaders** in `shaders/`

The `clear` mode is the original generated-placeholder path using a visible debug clear color.
The `bfi` mode reuses that insertion machinery but clears the generated image to solid black, providing a simple software black-frame-insertion path with configurable cadence via `OMFG_BFI_PERIOD`.
The current `blend` mode uses a simple fullscreen graphics pass to synthesize a midpoint placeholder from the previous and current frames.
The `adaptive-blend` mode builds on that by biasing the blend toward the current frame in higher-difference regions.
The `search-blend` mode adds a small neighborhood search on the previous frame to approximate motion-aware reprojection before blending.
The `search-adaptive-blend` mode combines the small neighborhood search with adaptive current-frame weighting.
The `reproject-blend` mode adds a stronger **symmetric patch-search reprojection** step, searching for a midpoint half-motion offset between the previous and current frames and blending confidence-weighted reprojected samples.
It now also exposes tunable quality controls via:
- `OMFG_REPROJECT_DISOCCLUSION_SCALE`
- `OMFG_REPROJECT_HOLE_FILL_STRENGTH`
- `OMFG_REPROJECT_HOLE_FILL_RADIUS`
- `OMFG_REPROJECT_GRADIENT_CONFIDENCE_WEIGHT` (reduces confidence in flat regions where motion estimation is unreliable; default `8.0`)
- `OMFG_REPROJECT_CHROMA_WEIGHT` (blends between luma-only and full RGB patch matching; default `0.3`, range `0.0-1.0`)
- `OMFG_REPROJECT_AMBIGUITY_SCALE` (suppresses confidence when multiple reprojection candidates are nearly tied; default `6.0`)
The `reproject-adaptive-blend` mode combines that stronger reprojection path with adaptive current-frame weighting.
The `multi-blend` mode is the first Rust **multi-FG** step, emitting multiple synthetic frames between real frames using temporal blend positions.
It now auto-expands swapchain image headroom for larger requested multipliers, controlled by `OMFG_MULTI_SWAPCHAIN_MAX_GENERATED_FRAMES` (default `32`).
A Steam Deck sweep has now validated successful `multi-blend` counts from `1..20` with full generated-frame success once that dynamic headroom expansion is enabled.
The `adaptive-multi-blend` mode combines both ideas: multi-FG plus adaptive current-frame weighting, and now includes both:
- the older present-interval heuristic path
- a newer **target-FPS adaptive controller** (`OMFG_ADAPTIVE_MULTI_TARGET_FPS`) that accumulates fractional generated-frame credit so the effective FG multiplier can fluctuate over time.
The `reproject-multi-blend` mode now propagates the stronger symmetric reprojection + confidence/disocclusion path into multi-FG generation.
That path now includes a small neighborhood hole-fill fallback for higher-disocclusion regions, using the same reprojection quality knobs above.
That higher-quality reprojection-backed multi-FG path is now validated on Deck through smoke, long, IMMEDIATE, and a higher-count `OMFG_MULTI_BLEND_COUNT=6` run.
The `reproject-adaptive-multi-blend` mode combines reprojection, confidence/disocclusion-aware fallback, adaptive current-frame weighting, and adaptive multi-FG control in one backend.
That mode is also now validated on Deck through smoke, long, IMMEDIATE, and a forced higher-count adaptive run with `OMFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=6`.
A focused benchmark run at `artifacts/steamdeck/rust/benchmark/reproject-multi-20260327-002943/` shows the reprojection-backed multi-FG path costs about `~3.76–3.79 ms/generated` on the Deck GPU for the current default reprojection settings.

Today that target-FPS controller is already validated on the Deck, but it still observes the app's intercepted present cadence under the current synchronization model. On the current Deck vsync-like validation path, that may simply reflect correct display-paced behavior rather than a bug; future controller work should separate app cadence from FG cadence more cleanly only where it improves decisions or visible results.

These are still not fully optical-flow or ML interpolation backends, but they are real shader-based generated-frame steps beyond placeholder copying and simple same-pixel blending.

That split should make it much easier to grow the test suite as frame generation gets more complex.
