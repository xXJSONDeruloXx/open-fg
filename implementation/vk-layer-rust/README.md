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
- first Rust-only generated backend mode:
  - `blend`
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
- blend-mode policy semantics
- dispatch-key extraction helper
- exported layer enumeration/proc-address plumbing
- loader negotiation ABI

### Linux/x86_64 build + test in Docker
From project root:

```bash
PPFG_LAYER_IMPL=rust ./scripts/build-linux-amd64.sh
```

The Rust crate vendors its dependencies under `implementation/vk-layer-rust/vendor/`, so the Linux builder can run offline/reproducibly.

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
```

### Full regression suite
```bash
export STEAMDECK_PASS='...'
export PPFG_LAYER_IMPL=rust
./scripts/run-layer-regression-suite.sh
```

## Design notes

The Rust port intentionally separates:
- **pure policy logic** in `src/config.rs` and `src/planner.rs`
- **unsafe Vulkan ABI/runtime glue** in `src/lib.rs`
- **loader-specific structs** in `src/layer_defs.rs`
- **precompiled test shaders** in `shaders/`

The current `blend` mode uses a simple fullscreen graphics pass to synthesize a midpoint placeholder from the previous and current frames.
That is still not motion-aware interpolation, but it is the first real shader-based generated-frame backend in this repo.

That split should make it much easier to grow the test suite as frame generation gets more complex.
