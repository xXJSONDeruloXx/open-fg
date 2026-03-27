# post-proc-fg-research

Research and implementation work for **OMFG — Open Multi Frame Generation**, a Linux-native, real-time, fully post-process frame generation / interpolation stack.

## Goal

Build toward an open solution that can:

- generate intermediate frames from final presented frames
- avoid dependency on app-provided motion vectors / depth / engine hooks
- work in real time on Linux
- eventually approach the user experience of:
  - Lossless Scaling Frame Generation
  - AMD AFMF
  - NVIDIA Smooth Motion

## Running the regression suite

```bash
# One-time: copy the example and set STEAMDECK_PASS
cp .env.steamdeck.local.example .env.steamdeck.local
# edit .env.steamdeck.local

# Run unit tests + full Deck hardware smoke suite (all 19 modes)
OMFG_LAYER_IMPL=rust bash scripts/run-layer-regression-suite.sh
```

The script sources `.env.steamdeck.local` automatically. With credentials present it:
builds a `linux/amd64` `.so` via Docker, deploys to the Deck, runs `vkcube` for every
mode, and asserts log markers. Without credentials it runs unit tests only and exits cleanly.

See `docs/testing-strategy.md` for full details.

---

## Current recommendation

**Current implementation path:**
- maintain the working C++ Vulkan-layer MVP as the reference oracle
- grow a Rust parity port with a stronger regression harness
- keep validating both against the same Steam Deck smoke targets

**MVP path:**
- start with a **Vulkan layer / swapchain interception prototype**
- keep it **clean-room / permissively licensed**
- use `lsfg-vk` and old FFX Vulkan frame-generation code as **reference architecture**, not as code to directly fork
- treat `gamescope` as the likely **phase 2 / compositor path** once the interpolation core and present scheduler are proven

## Documentation map

### Research
- `research/post-process-frame-gen-linux-research.md`
  - full landscape writeup
  - what exists today
  - what is possible
  - what is missing
  - cloned repos and fetched sources

### Decisions / planning
- `docs/path-comparison.md`
  - Vulkan-layer path vs compositor/gamescope path
  - tradeoffs, licensing, and recommendation
- `docs/mvp-plan.md`
  - scoped MVP proposal
  - milestones and success criteria
- `docs/testing-strategy.md`
  - realistic testing plan
  - what can and cannot be validated from macOS Apple Silicon
- `docs/open-questions.md`
  - unresolved technical questions and Linux experiments to run next
- `docs/targets/steamdeck.md`
  - confirmed remote Linux target
  - Steam Deck environment details
  - remote helper usage
- `docs/implementation-status.md`
  - current build/deploy/runtime status
  - what now works on the Steam Deck
  - current BFI status and display-target observations
- `docs/rust-feature-roadmap.md`
  - Rust-first implementation ladder
  - parity goals and next capability targets
- `docs/future-backends.md`
  - how FSR3-style analytical FG, RIFE-style ML, and vendor optical-flow paths fit the roadmap
  - current recommendation for what stays on the mainline vs parallel research branches
- `experiments/program.md`
  - current autoperf benchmark subset
  - acceptance rules for pacing/synchronization changes

### Implementation
- `implementation/vk-layer-mvp/README.md`
  - current C++ Vulkan-layer MVP
  - modes, build, deploy, and smoke-test usage
- `implementation/vk-layer-rust/README.md`
  - Rust parity port
  - regression tests and extensible harness

## Local research assets

### Cloned repositories
Stored under `research/repos/`.

Key repos:
- `lsfg-vk`
- `gamescope`
- `FidelityFX-SDK`
- `FidelityFX-SDK-v1.1.4`
- `NVIDIAOpticalFlowSDK`
- `linux-fg`
- `lsfg-vk-afmf`
- `vkBasalt`
- `rife-ncnn-vulkan`

### Fetched vendor docs
Stored under `research/fetch/`.

## Important current constraint

Current host is:
- macOS
- Apple Silicon (`arm64`, Apple M4 Pro)

That means this machine is good for:
- research
- documentation
- scaffolding
- code authoring
- build-system work

It is **not** the right place to validate:
- Linux Vulkan layer behavior
- real present timing / pacing
- gamescope integration
- NVIDIA OFA / FRUC
- AFMF / Smooth Motion equivalents
- VRR / HDR / queue-family behavior

See `docs/testing-strategy.md` for details.
