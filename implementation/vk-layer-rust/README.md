# Vulkan layer Rust port

Rust port of the current explicit Vulkan post-process frame-generation layer MVP.

## Current goal

Reach feature parity with the C++ MVP while building a safer foundation for future work.

Current Rust capability set:
- explicit Vulkan layer ABI exports
- instance / device / swapchain / present interception
- parity runtime modes:
  - `passthrough`
  - `clear`
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
- blend/adaptive-blend/search-blend/search-adaptive-blend/reproject-blend/reproject-adaptive-blend/multi-blend/adaptive-multi-blend policy semantics
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

## Design notes

The Rust port intentionally separates:
- **pure policy logic** in `src/config.rs` and `src/planner.rs`
- **unsafe Vulkan ABI/runtime glue** in `src/lib.rs`
- **loader-specific structs** in `src/layer_defs.rs`
- **precompiled test shaders** in `shaders/`

The current `blend` mode uses a simple fullscreen graphics pass to synthesize a midpoint placeholder from the previous and current frames.
The `adaptive-blend` mode builds on that by biasing the blend toward the current frame in higher-difference regions.
The `search-blend` mode adds a small neighborhood search on the previous frame to approximate motion-aware reprojection before blending.
The `search-adaptive-blend` mode combines the small neighborhood search with adaptive current-frame weighting.
The `reproject-blend` mode adds a stronger **symmetric patch-search reprojection** step, searching for a midpoint half-motion offset between the previous and current frames and blending confidence-weighted reprojected samples.
The `reproject-adaptive-blend` mode combines that stronger reprojection path with adaptive current-frame weighting.
The `multi-blend` mode is the first Rust **multi-FG** step, emitting two synthetic frames between real frames using temporal blend positions.
The `adaptive-multi-blend` mode combines both ideas: multi-FG plus adaptive current-frame weighting, and now also includes an initial present-interval-based frame-count controller.

These are still not fully optical-flow or ML interpolation backends, but they are real shader-based generated-frame steps beyond placeholder copying and simple same-pixel blending.

That split should make it much easier to grow the test suite as frame generation gets more complex.
