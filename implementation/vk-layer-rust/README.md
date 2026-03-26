# Vulkan layer Rust port

Rust port of the current explicit Vulkan post-process frame-generation layer MVP.

## Current goal

Reach feature parity with the C++ MVP while building a safer foundation for future work:
- explicit Vulkan layer ABI exports
- instance / device / swapchain / present interception
- runtime modes:
  - `passthrough`
  - `clear`
  - `copy`
  - `history-copy`
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
```

## Design notes

The Rust port intentionally separates:
- **pure policy logic** in `src/config.rs` and `src/planner.rs`
- **unsafe Vulkan ABI/runtime glue** in `src/lib.rs`
- **loader-specific structs** in `src/layer_defs.rs`

That split should make it much easier to grow the test suite as frame generation gets more complex.
