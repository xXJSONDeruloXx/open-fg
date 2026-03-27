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
- LSFG-style target-FPS adaptive controller for `adaptive-multi-blend`
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
- clear/bfi/copy/history and blend/adaptive-blend/search-blend/search-adaptive-blend/reproject-blend/reproject-adaptive-blend/multi-blend/adaptive-multi-blend policy semantics
- target-FPS adaptive multi-FG controller logic
- pure Rust motion-search / reprojection heuristic tests
- dispatch-key extraction helper
- exported layer enumeration/proc-address plumbing
- loader negotiation ABI

### Linux/x86_64 build + test in Docker
From project root:

```bash
PPFG_LAYER_IMPL=rust ./scripts/build-linux-amd64.sh
```

The Rust crate vendors its dependencies under `implementation/vk-layer-rust/vendor/`, so the Linux builder can run offline/reproducibly.
The Docker builder also recompiles the GLSL shaders via `scripts/compile-rust-shaders.sh`, so the SPIR-V artifacts are now reproducible as part of the Linux build path.

That runs Rust tests inside the Linux builder container and emits:

- `build/linux-amd64/vk-layer-rust/out/libVkLayer_PPFG_rust.so`
- `build/linux-amd64/vk-layer-rust/out/VkLayer_PPFG_rust.json`

## Steam Deck flow

### Deploy
```bash
export STEAMDECK_PASS='...'
PPFG_LAYER_IMPL=rust ./scripts/deploy-steamdeck-layer.sh
```

### Smoke test with vkcube
```bash
export STEAMDECK_PASS='...'
export PPFG_LAYER_IMPL=rust
export PPFG_LAYER_MODE=passthrough
./scripts/test-steamdeck-vkcube.sh

export PPFG_LAYER_MODE=history-copy
./scripts/test-steamdeck-vkcube.sh

export PPFG_LAYER_MODE=bfi
export PPFG_BFI_PERIOD=1
./scripts/test-steamdeck-vkcube.sh

export PPFG_LAYER_MODE=blend
./scripts/test-steamdeck-vkcube.sh

export PPFG_LAYER_MODE=adaptive-blend
./scripts/test-steamdeck-vkcube.sh

export PPFG_LAYER_MODE=search-blend
./scripts/test-steamdeck-vkcube.sh

export PPFG_LAYER_MODE=search-adaptive-blend
./scripts/test-steamdeck-vkcube.sh

export PPFG_LAYER_MODE=reproject-blend
./scripts/test-steamdeck-vkcube.sh

export PPFG_LAYER_MODE=reproject-adaptive-blend
./scripts/test-steamdeck-vkcube.sh

export PPFG_LAYER_MODE=multi-blend
./scripts/test-steamdeck-vkcube.sh

export PPFG_LAYER_MODE=adaptive-multi-blend
./scripts/test-steamdeck-vkcube.sh

# Optional LSFG-style target-FPS controller for adaptive-multi-blend
export PPFG_ADAPTIVE_MULTI_TARGET_FPS=120
export PPFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=0
export PPFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=2
./scripts/test-steamdeck-vkcube.sh
```

### Full regression suite
```bash
export STEAMDECK_PASS='...'
export PPFG_LAYER_IMPL=rust
./scripts/run-layer-regression-suite.sh
```

### Advanced Steam Deck validation
This extends the normal smoke suite with long FIFO and IMMEDIATE runs for the stronger motion-aware single-FG modes.

```bash
export STEAMDECK_PASS='...'
export PPFG_LAYER_IMPL=rust
./scripts/run-advanced-steamdeck-validation.sh
```

### Target-FPS adaptive multi-FG validation
This exercises the LSFG-style target-FPS controller on the Steam Deck, including fractional target cases.

```bash
export STEAMDECK_PASS='...'
export PPFG_LAYER_IMPL=rust
./scripts/run-target-fps-steamdeck-validation.sh
```

### BFI validation
This validates black-frame insertion behavior, including a reduced-cadence `PPFG_BFI_PERIOD=2` case.

```bash
export STEAMDECK_PASS='...'
export PPFG_LAYER_IMPL=rust
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
export PPFG_LAYER_IMPL=rust
./scripts/run-steamdeck-benchmark-suite.sh

# Fast decision subset only
PPFG_BENCHMARK_PRESET=decision ./scripts/run-steamdeck-benchmark-suite.sh
```

### Autoperf loop
This repeatedly runs the fast decision subset, aggregates the results, compares them against a baseline, and can optionally promote winners to the full benchmark suite.

```bash
export STEAMDECK_PASS='...'
export PPFG_LAYER_IMPL=rust
./scripts/run-autoperf-loop.sh

# Optional full-suite promotion on acceptance
PPFG_AUTOPERF_RUN_FULL_ON_ACCEPT=1 ./scripts/run-autoperf-loop.sh
```

See `experiments/program.md` for the current subset, weights, and accept/reject rules.

## Design notes

The Rust port intentionally separates:
- **pure policy logic** in `src/config.rs` and `src/planner.rs`
- **unsafe Vulkan ABI/runtime glue** in `src/lib.rs`
- **loader-specific structs** in `src/layer_defs.rs`
- **precompiled test shaders** in `shaders/`

The `clear` mode is the original generated-placeholder path using a visible debug clear color.
The `bfi` mode reuses that insertion machinery but clears the generated image to solid black, providing a simple software black-frame-insertion path with configurable cadence via `PPFG_BFI_PERIOD`.
The current `blend` mode uses a simple fullscreen graphics pass to synthesize a midpoint placeholder from the previous and current frames.
The `adaptive-blend` mode builds on that by biasing the blend toward the current frame in higher-difference regions.
The `search-blend` mode adds a small neighborhood search on the previous frame to approximate motion-aware reprojection before blending.
The `search-adaptive-blend` mode combines the small neighborhood search with adaptive current-frame weighting.
The `reproject-blend` mode adds a stronger **symmetric patch-search reprojection** step, searching for a midpoint half-motion offset between the previous and current frames and blending confidence-weighted reprojected samples.
The `reproject-adaptive-blend` mode combines that stronger reprojection path with adaptive current-frame weighting.
The `multi-blend` mode is the first Rust **multi-FG** step, emitting two synthetic frames between real frames using temporal blend positions.
The `adaptive-multi-blend` mode combines both ideas: multi-FG plus adaptive current-frame weighting, and now includes both:
- the older present-interval heuristic path
- a newer **target-FPS adaptive controller** (`PPFG_ADAPTIVE_MULTI_TARGET_FPS`) that accumulates fractional generated-frame credit so the effective FG multiplier can fluctuate over time.

Today that target-FPS controller is already validated on the Deck, but it still observes the app's intercepted present cadence under the current conservative synchronization model, so its decisions are still coupled to current pacing overhead.

These are still not fully optical-flow or ML interpolation backends, but they are real shader-based generated-frame steps beyond placeholder copying and simple same-pixel blending.

That split should make it much easier to grow the test suite as frame generation gets more complex.
