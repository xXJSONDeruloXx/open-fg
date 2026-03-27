# Implementation status

## Summary

We now have:
- a **working Linux Vulkan layer MVP** in C++
- a **working Rust parity port** for the current MVP scope
- multiple **Rust shader-based generated-frame backends**
- a simple **software BFI / black-frame-insertion mode** in Rust
- a first **adaptive frame-count control path** in Rust multi-FG
- a repeatable local + Linux + Steam Deck regression harness

This is beyond paper architecture at this point.

## Current status

### Working
- explicit Vulkan layer negotiation / loading
- instance/device/swapchain/present interception
- queue tracking
- swapchain mutation for extra image capacity
- remote build + deploy loop
- remote smoke test loop on Steam Deck
- log capture back into local artifacts
- Rust unit tests for mode parsing, swapchain mutation, sequencing, exports, and loader negotiation
- generic regression harness scripts:
  - `scripts/run-layer-regression-suite.sh`
  - `scripts/run-advanced-steamdeck-validation.sh`
  - `scripts/run-target-fps-steamdeck-validation.sh`
  - `scripts/run-bfi-steamdeck-validation.sh`
  - `scripts/collect-steamdeck-display-info.sh`
  - `scripts/run-steamdeck-benchmark-suite.sh`
  - `scripts/run-autoperf-loop.sh`
  - `scripts/aggregate-benchmark-results.py`
  - `scripts/compare-benchmark-results.py`
  - `scripts/assert-vkcube-log.py`
  - `scripts/compile-rust-shaders.sh`

### Rust parity status

Rust implementation location:
- `implementation/vk-layer-rust/`

Rust implementation currently has verified parity for the existing MVP feature set:
- `passthrough`
- `clear`
- `bfi`
- `copy`
- `history-copy`

Rust also now has additional next-step backend modes:
- `blend`
- `adaptive-blend`
- `search-blend`
- `search-adaptive-blend`
- `multi-blend`
- `adaptive-multi-blend`

Validated via:
- local `cargo test --locked`
- Linux/x86_64 Docker build + test via `PPFG_LAYER_IMPL=rust ./scripts/build-linux-amd64.sh`
- full Deck smoke suite via `PPFG_LAYER_IMPL=rust ./scripts/run-layer-regression-suite.sh`

### Verified runtime modes on Steam Deck

The modes below are proven on the Steam Deck in the original C++ implementation, and now also in the Rust parity port for the main `vkcube` smoke path.

#### 1. `passthrough`
Working.

Validated with:
- `vkcube --c 120`

Observed:
- 120 real presents completed cleanly
- swapchain creation and present logging correct
- no crashes / no hangs

#### 2. `clear`
Working.

Validated with:
- `vkcube --c 120`

Observed:
- 120 real presents completed cleanly
- 120 generated placeholder presents completed cleanly
- extra frame insertion proven on real Linux hardware

#### 2a. `bfi` (Rust)
Working as a simple software black-frame-insertion path.

Validated with Rust layer on Steam Deck:
- `vkcube --c 120`
- `vkcube --c 600`
- `vkcube --c 120 --present_mode 0`
- `PPFG_BFI_PERIOD=2 vkcube --c 120`
- full Rust regression suite including `bfi`
- dedicated BFI validation via `scripts/run-bfi-steamdeck-validation.sh`

Observed:
- every inserted generated image is cleared to solid black
- default `PPFG_BFI_PERIOD=1` inserts one black frame after every intercepted real present
- `PPFG_BFI_PERIOD=2` reduces insertion cadence and was validated on Deck by ending at `black frame present=60` for a `120`-frame app run
- swapchain image count was increased from `3 -> 4` for the validated Deck path
- stable on Deck through smoke, long, and IMMEDIATE-mode runs

#### 3. `copy`
Working.

Validated with:
- `vkcube --c 120`

Observed:
- 120 real presents completed cleanly
- 120 duplicated generated-frame presents completed cleanly
- swapchain image count bumped from 3 -> 5
- usage flags bumped to include `TRANSFER_SRC` + `TRANSFER_DST`
- per-frame copy from source app image into generated swapchain image succeeded across the full run

#### 4. `history-copy`
Working and now the best placeholder mode.

Validated with:
- `vkcube --c 120`

Observed:
- first frame primes private history
- subsequent frames present a generated placeholder frame derived from the **previous** real frame
- generated frame is presented **before** the current real frame
- 120 real presents completed cleanly
- 119+ generated placeholder presents completed cleanly after priming, with stable operation through the run
- additional stress run completed cleanly through **600 real frames** on `vkcube`
- `vkcube` also completed successfully in **IMMEDIATE** present mode with `history-copy`
- `vkcube` also completed successfully in **MAILBOX** present mode with `history-copy`
- private history image allocation and reuse works on the Steam Deck

This is the current strongest proof that the project direction is viable, because it demonstrates:
- persistent frame history
- timing-aligned placeholder insertion
- present ordering closer to actual frame generation

#### 5. `blend` (Rust)
Working as the first shader-based generated backend.

Validated with Rust layer on Steam Deck:
- `vkcube --c 120`
- `vkcube --c 600`
- `vkcube --c 120 --present_mode 0`

Observed:
- first frame primes history
- subsequent frames render a generated frame from a **50/50 blend** of previous and current frames
- generated frame is presented before the current real frame
- stable through 120-frame and 600-frame runs on Deck
- stable in **IMMEDIATE** present mode on Deck
- uses a real graphics pipeline + shader pass, not just transfer-copy placeholders

#### 6. `adaptive-blend` (Rust)
Working as the next shader synthesis step.

Validated with Rust layer on Steam Deck:
- `vkcube --c 120`
- `vkcube --c 600`
- `vkcube --c 120 --present_mode 0`
- full Rust regression suite including `adaptive-blend`

Observed:
- first frame primes history
- subsequent generated frames use adaptive current-frame bias based on previous/current frame difference
- generated frame is still presented before the current real frame
- stable on Deck through the 120-frame smoke path and an additional **600-frame** run
- stable in **IMMEDIATE** present mode on Deck
- keeps the same present/interception infrastructure while improving synthesis behavior over fixed 50/50 blending

#### 7. `search-blend` (Rust)
Working as the first motion-search synthesis heuristic.

Validated with Rust layer on Steam Deck:
- `vkcube --c 120`
- full Rust regression suite including `search-blend`

Observed:
- first frame primes history
- subsequent generated frames search a small neighborhood in the previous frame before blending with current
- stable on the Deck 120-frame smoke path
- demonstrates the first explicit motion-search style heuristic in the Rust implementation

#### 8. `search-adaptive-blend` (Rust)
Working as the first combined motion-search + adaptive synthesis step.

Validated with Rust layer on Steam Deck:
- `vkcube --c 120`
- full Rust regression suite including `search-adaptive-blend`

Observed:
- first frame primes history
- subsequent generated frames perform a small neighborhood search in the previous frame
- adaptive current-frame weighting is then applied using the matched previous sample
- stable on the Deck 120-frame smoke path

#### 9. `multi-blend` (Rust)
Working as the first multi-FG stepping stone.

Validated with Rust layer on Steam Deck:
- `vkcube --c 120`
- `vkcube --c 600`
- `vkcube --c 120 --present_mode 0`
- full Rust regression suite including `multi-blend`

Observed:
- first frame primes history
- subsequent real frames emit **two generated frames** before the current real frame
- generated frames are rendered at multiple temporal blend positions between previous and current frames
- swapchain image count was increased from `3 -> 6` for the validated Deck path
- stable on Deck through 120-frame, 600-frame, and IMMEDIATE-mode runs

#### 10. `adaptive-multi-blend` (Rust)
Working as the current richest generated-frame backend in the Rust layer.

Validated with Rust layer on Steam Deck:
- `vkcube --c 120`
- `vkcube --c 600`
- `vkcube --c 120 --present_mode 0`
- full Rust regression suite including `adaptive-multi-blend`

Observed:
- adapts generated frame count based on runtime timing
- supports a new LSFG-style **target-FPS controller** via `PPFG_ADAPTIVE_MULTI_TARGET_FPS`
- fractional targets are accumulated over time via generated-frame credit, so effective multipliers can fluctuate between `0`, `1`, and `2` generated frames per real frame
- Deck validation now includes real target-FPS cases for `90`, `100`, `120`, and `150` FPS targets
- applies adaptive current-frame bias based on previous/current difference while doing multi-FG
- stable on the Deck 120-frame smoke path
- stable on an additional **600-frame** run
- demonstrates the first combined multi-FG + adaptive synthesis + target-FPS control mode in Rust

These modes are still stepping stones toward motion-aware interpolation, but they are now clearly beyond placeholder-only generation in the Rust implementation.

---

## Display target observations from current Steam Deck capture

Captured via:
- `scripts/collect-steamdeck-display-info.sh`
- artifact root: `artifacts/steamdeck/display-info/bfi-validation/`

Observed on the current Deck test target:
- internal connector: `eDP`
- active mode: `800x1280 @ 90.00 Hz`
- no external `DP-1` display was connected during the capture
- VRR is reported as unavailable on this active panel path (`vrr_capable = 0` in the captured outputs)
- Vulkan reports support for:
  - `VK_GOOGLE_display_timing`
  - `VK_KHR_present_id`
  - `VK_KHR_present_wait`

That means we can now confirm the **active panel mode** on the Linux target, but we still have **not** yet wired end-to-end past-presentation timing into the layer itself, so panel-level pacing/scanout confirmation is still weaker than the layer's own present logs.

---

## Benchmark / autoperf status

We now also have a first repo-specific **autoperf loop** for repeated Deck benchmarking.

Current pieces:
- `scripts/run-steamdeck-benchmark-suite.sh`
  - supports both `PPFG_BENCHMARK_PRESET=full` and `PPFG_BENCHMARK_PRESET=decision`
- `scripts/aggregate-benchmark-results.py`
  - aggregates repeated benchmark runs into mean / stdev summaries
- `scripts/compare-benchmark-results.py`
  - compares baseline vs candidate with weighted accept / reject logic
- `scripts/run-autoperf-loop.sh`
  - orchestrates repeated decision-subset runs, aggregation, comparison, and optional full-suite promotion
- `experiments/program.md`
  - records the current fast subset and acceptance rules

The current fast decision subset is:
- `blend`
- `reproject-blend-default`
- `multi-blend-count3`
- `adaptive-multi-target180`

First validated autoperf run:
- autoperf artifact root:
  - `artifacts/steamdeck/rust/autoperf/20260326-220336/`
- repeated decision runs:
  - `3`
- baseline:
  - `artifacts/steamdeck/rust/benchmark/extended-20260326-204745/`
- result:
  - `accepted=1`
  - weighted improvement `0.902%`
  - worst tracked regression `0.000%`
- promoted full-suite benchmark:
  - `artifacts/steamdeck/rust/benchmark/autoperf-20260326-220336-full/`
- promoted full-suite comparison:
  - `artifacts/steamdeck/rust/autoperf/20260326-220336/promoted-full-comparison.txt`
  - accepted with weighted improvement `1.163%`

This is intended to make future pacing / synchronization experiments much cheaper to validate before paying for the full Deck benchmark matrix.

---

## Future backend map

The repo now explicitly treats backend directions as separate tracks:

### Current mainline
- keep the default implementation **classical / analytical / post-process**
- continue Linux-first Vulkan-layer work
- borrow ideas from FSR3-style analytical FG where they fit post-process constraints

### Parallel research
- use `RIFE` / `rife-ncnn-vulkan` as a **quality oracle** on captured frame pairs
- evaluate when an experimental runtime ML backend would be worth the latency/runtime cost
- evaluate NVIDIA Optical Flow / FRUC as an optional vendor-specific acceleration path

### Not current mainline
- FSR4-style ML is currently a **later conceptual reference**, not a near-term Linux layer target
- the public FSR4 path is a poor direct fit for the current repo assumptions because it expects richer platform and integration constraints than the current Linux explicit-layer path provides

Detailed rationale now lives in:
- `docs/future-backends.md`

---

## Important technical insight from implementation

### The stable placeholder-frame paths were:
- increase swapchain image count
- acquire an extra image for the generated frame path
- either:
  - copy the current source frame into that acquired image (`copy`)
  - or maintain private history and present the previous real frame first (`history-copy`)
- drive both paths from explicit queue submission and explicit semaphore wiring
- use conservative synchronization and queue idle in test mode

That is not final-product pacing, but it is a real, working insertion path.

---

## Remaining gap to true frame generation

Right now the layer can do:
- **post-process frame insertion**
- **software black-frame insertion**
- **same-frame duplication**
- **previous-frame placeholder insertion with private history**
- **simple shader-based previous/current frame blending**
- **difference-aware adaptive blending**
- **small-neighborhood motion-search blending**
- **combined motion-search + adaptive blending**
- **initial multi-FG via two generated frames per real frame**
- **combined adaptive + multi-FG synthesis**

It still cannot do:
- **true motion-aware interpolated frame generation** comparable to mature optical-flow-backed systems
- **a pacing-decoupled target-FPS controller comparable to polished production FG stacks**
- **higher-quality motion/disocclusion-aware multi-FG**
- **runtime ML interpolation**
- **vendor optical-flow accelerated motion estimation**

So the next major milestone on the mainline is replacing duplicate copy / naive interpolation assumptions with:
- optical-flow / warp / blend / inpaint logic
- stronger confidence and disocclusion handling

Separately, the most important parallel research milestone is:
- using `RIFE`-style ML interpolation as a quality oracle so we can decide when ML is worth adding as an experimental runtime branch

---

## Artifacts

### vkcube
C++ MVP:
- `artifacts/steamdeck/vkcube/passthrough/ppfg-vkcube.log`
- `artifacts/steamdeck/vkcube/clear/ppfg-vkcube.log`
- `artifacts/steamdeck/vkcube/copy/ppfg-vkcube.log`
- `artifacts/steamdeck/vkcube/history-copy/ppfg-vkcube.log`
- `artifacts/steamdeck/vkcube/history-copy-long/ppfg-vkcube-long.log`
- `artifacts/steamdeck/vkcube/history-copy-immediate/ppfg-vkcube-immediate.log`
- `artifacts/steamdeck/vkcube/history-copy-mailbox/ppfg-vkcube-mailbox.log`

Rust parity port:
- `artifacts/steamdeck/rust/vkcube/passthrough/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/clear/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/bfi/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/bfi-smoke/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/bfi-long/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/bfi-immediate/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/bfi-period2-smoke/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/copy/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/history-copy/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/blend/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/blend-long/ppfg-vkcube-blend-long.log`
- `artifacts/steamdeck/rust/vkcube/blend-immediate/ppfg-vkcube-blend-immediate.log`
- `artifacts/steamdeck/rust/vkcube/adaptive-blend/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/adaptive-blend-long/ppfg-vkcube-adaptive-long.log`
- `artifacts/steamdeck/rust/vkcube/adaptive-blend-immediate/ppfg-vkcube-adaptive-immediate.log`
- `artifacts/steamdeck/rust/vkcube/search-blend/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/search-adaptive-blend/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/multi-blend/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/multi-blend-long/ppfg-vkcube-multi-long.log`
- `artifacts/steamdeck/rust/vkcube/multi-blend-immediate/ppfg-vkcube-multi-immediate.log`
- `artifacts/steamdeck/rust/vkcube/adaptive-multi-blend/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/adaptive-multi-blend-long/ppfg-vkcube-adaptive-multi-long.log`
- `artifacts/steamdeck/rust/vkcube/adaptive-multi-blend-immediate/ppfg-vkcube-adaptive-multi-immediate.log`
- `artifacts/steamdeck/rust/vkcube/adaptive-multi-blend-target100-long/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/adaptive-multi-blend-target120-smoke/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/adaptive-multi-blend-target150-smoke/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/adaptive-multi-blend-target90-immediate/ppfg-vkcube.log`

### vkgears
- `artifacts/steamdeck/vkgears/clear/ppfg-vkgears.log`
- `artifacts/steamdeck/rust/vkgears/history-copy/ppfg-vkgears.log`

### display info
- `artifacts/steamdeck/display-info/bfi-validation/summary.txt`
- `artifacts/steamdeck/display-info/bfi-validation/xrandr.txt`
- `artifacts/steamdeck/display-info/bfi-validation/modetest.txt`
- `artifacts/steamdeck/display-info/bfi-validation/drm_info.txt`
- `artifacts/steamdeck/display-info/bfi-validation/vulkan_display_timing.txt`

---

## Notable unresolved item

### `vkgears`
Under the current remote test setup, `vkgears` is not yet a useful validation target.

Observed behavior:
- layer negotiation occurs
- the process times out under the remote harness
- this remains true for both the C++ MVP and the Rust parity port
- we do not yet get the same clean create-device / create-swapchain / present trace as `vkcube`

That means `vkcube` is currently the reliable smoke-test target.

---

## Recommendation

The project has now crossed from:
- research only

to:
- working Linux runtime MVP with duplicate and history-based generated-frame insertion

The next best implementation step is:

## **keep improving the classical Linux mainline while treating ML and vendor optical flow as explicit side branches**

Recommended execution model now:
- keep the C++ layer as the already-proven oracle
- make the Rust port the primary place for new safety-minded implementation and test growth
- keep both on the same Deck smoke harness until interpolation parity is achieved
- continue iterating toward the roadmap recorded in `docs/rust-feature-roadmap.md`
- use `docs/future-backends.md` as the guide for when FSR3-style ideas, RIFE-style ML, and NVIDIA optical-flow paths should enter the roadmap

Meaning:
- keep the current queue/swapchain/present path
- treat `blend`, `adaptive-blend`, `search-blend`, `search-adaptive-blend`, `multi-blend`, and `adaptive-multi-blend` as shader stepping stones
- next target **stronger motion-aware synthesis, better reprojection, and better pacing in Rust**
- in parallel, use ML primarily as a research oracle first rather than immediately making it the default backend
