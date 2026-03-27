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
  - `scripts/run-present-timing-steamdeck-validation.sh`
  - `scripts/run-bfi-steamdeck-validation.sh`
  - `scripts/collect-steamdeck-display-info.sh`
  - `scripts/run-steamdeck-benchmark-suite.sh`
  - `scripts/run-autoperf-loop.sh`
  - `scripts/run-steamdeck-multi-count-sweep.sh`
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
- `reproject-blend`
- `reproject-adaptive-blend`
- `multi-blend`
- `adaptive-multi-blend`
- `reproject-multi-blend`
- `reproject-adaptive-multi-blend`

Validated via:
- local `cargo test --locked`
- Linux/x86_64 Docker build + test via `OMFG_LAYER_IMPL=rust ./scripts/build-linux-amd64.sh`
- full Deck smoke suite via `OMFG_LAYER_IMPL=rust ./scripts/run-layer-regression-suite.sh`

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
- `OMFG_BFI_PERIOD=2 vkcube --c 120`
- full Rust regression suite including `bfi`
- dedicated BFI validation via `scripts/run-bfi-steamdeck-validation.sh`

Observed:
- every inserted generated image is cleared to solid black
- default `OMFG_BFI_PERIOD=1` inserts one black frame after every intercepted real present
- `OMFG_BFI_PERIOD=2` reduces insertion cadence and was validated on Deck by ending at `black frame present=60` for a `120`-frame app run
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

#### 8a. `reproject-blend` (Rust)
Working as the first stronger reprojection backend.

Validated with Rust layer on Steam Deck:
- `vkcube --c 120`
- `vkcube --c 600`
- `vkcube --c 120 --present_mode 0`
- full Rust regression suite including `reproject-blend`
- advanced Deck validation via `scripts/run-advanced-steamdeck-validation.sh`

Observed:
- first frame primes history
- subsequent generated frames use a stronger **symmetric patch-search reprojection** step
- reprojected samples are blended with confidence weighting and a disocclusion-aware fallback toward the original frames
- the reprojection path now also exposes tunable quality controls:
  - `OMFG_REPROJECT_DISOCCLUSION_SCALE`
  - `OMFG_REPROJECT_HOLE_FILL_STRENGTH`
  - `OMFG_REPROJECT_HOLE_FILL_RADIUS`
  - `OMFG_REPROJECT_GRADIENT_CONFIDENCE_WEIGHT` (reduces confidence in flat regions where motion estimation is unreliable; default `8.0`)
  - `OMFG_REPROJECT_CHROMA_WEIGHT` (blends between luma-only and full RGB patch matching; default `0.3`)
- local validation for those new quality controls is green (`cargo test`, `./scripts/test-rust-layer.sh`, `OMFG_LAYER_IMPL=rust ./scripts/build-linux-amd64.sh`)
- stable on Deck through smoke, long, and IMMEDIATE-mode runs

#### 8b. `reproject-adaptive-blend` (Rust)
Working as the adaptive variant of the stronger reprojection backend.

Validated with Rust layer on Steam Deck:
- `vkcube --c 120`
- `vkcube --c 600`
- `vkcube --c 120 --present_mode 0`
- full Rust regression suite including `reproject-adaptive-blend`
- advanced Deck validation via `scripts/run-advanced-steamdeck-validation.sh`

Observed:
- combines the stronger symmetric reprojection step with adaptive current-frame biasing
- keeps the same confidence/disocclusion-aware reprojection fallback path
- stable on Deck through smoke, long, and IMMEDIATE-mode runs

#### 9. `multi-blend` (Rust)
Working as the first multi-FG stepping stone.

Validated with Rust layer on Steam Deck:
- `vkcube --c 120`
- `vkcube --c 600`
- `vkcube --c 120 --present_mode 0`
- full Rust regression suite including `multi-blend`

Observed:
- first frame primes history
- subsequent real frames emit generated frames before the current real frame
- generated frames are rendered at multiple temporal blend positions between previous and current frames
- the layer now **auto-expands swapchain headroom** for larger `OMFG_MULTI_BLEND_COUNT` requests
- new env knob:
  - `OMFG_MULTI_SWAPCHAIN_MAX_GENERATED_FRAMES` (default `32`)
- Steam Deck multiplier sweep now validates successful `multi-blend` counts from `1..20`
- artifact root for the successful post-change sweep:
  - `artifacts/steamdeck/rust/benchmark/multi-count-sweep3-20260326-231948/`
- counts above the current GPU-acquire-chain limit (`4` generated acquires in the current fast path) still succeed by falling back to the CPU acquire path once enough swapchain images are provisioned
- swapchain image count now scales with requested multiplier, e.g.:
  - `count=6` -> `minImages=3->7`
  - `count=10` -> `minImages=3->11`
  - `count=20` -> `minImages=3->21`
- stable on Deck through 120-frame, 600-frame, and IMMEDIATE-mode runs

#### 10. `adaptive-multi-blend` (Rust)
Working as the current controller-oriented adaptive multi-FG backend in the Rust layer.

Validated with Rust layer on Steam Deck:
- `vkcube --c 120`
- `vkcube --c 600`
- `vkcube --c 120 --present_mode 0`
- full Rust regression suite including `adaptive-multi-blend`

Observed:
- adapts generated frame count based on runtime timing
- supports a new LSFG-style **target-FPS controller** via `OMFG_ADAPTIVE_MULTI_TARGET_FPS`
- fractional targets are accumulated over time via generated-frame credit, so effective multipliers can fluctuate between `0`, `1`, and `2` generated frames per real frame by default
- the same dynamic swapchain headroom expansion used by `multi-blend` now also applies to larger `OMFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES` experiments
- Deck validation now includes real target-FPS cases for `90`, `100`, `120`, and `150` FPS targets
- applies adaptive current-frame bias based on previous/current difference while doing multi-FG
- stable on the Deck 120-frame smoke path
- stable on an additional **600-frame** run
- demonstrates the first combined multi-FG + adaptive synthesis + target-FPS control mode in Rust

#### 10a. `reproject-multi-blend` (Rust)
Working as the first higher-quality reprojection-backed multi-FG path.

Validated with Rust layer on Steam Deck:
- `vkcube --c 120`
- `vkcube --c 600`
- `vkcube --c 120 --present_mode 0`
- `OMFG_MULTI_BLEND_COUNT=6 vkcube --c 120`
- full Rust regression suite including `reproject-multi-blend`
- advanced Deck validation via `scripts/run-advanced-steamdeck-validation.sh`

Observed:
- propagates the stronger symmetric reprojection + confidence/disocclusion path into multi-FG generation
- the current reprojection path now includes a small neighborhood hole-fill fallback for higher-disocclusion regions, driven by the same `OMFG_REPROJECT_DISOCCLUSION_SCALE`, `OMFG_REPROJECT_HOLE_FILL_STRENGTH`, and `OMFG_REPROJECT_HOLE_FILL_RADIUS` knobs
- local validation for that richer reprojection path is green (`cargo test`, `./scripts/test-rust-layer.sh`, `OMFG_LAYER_IMPL=rust ./scripts/build-linux-amd64.sh`)
- stable on Deck through smoke, long, and IMMEDIATE-mode runs
- larger-count validation now proves the higher-quality reprojection path also benefits from dynamic swapchain headroom expansion
- a targeted `count=6` Deck smoke run succeeded with:
  - `requestedGeneratedFrames=6`
  - `minImages=3->7`
  - generated output continuing through `reproject multi blended frame present=660`
- higher counts above the current GPU-acquire fast path fall back to the CPU acquire path but still complete successfully once enough images are provisioned
- targeted benchmark artifact:
  - `artifacts/steamdeck/rust/benchmark/reproject-multi-20260327-002943/`
  - `reproject-multi-count2` ~ `7.563 ms` GPU total, ~ `3.781 ms/generated`
  - `reproject-multi-count3` ~ `11.274 ms` GPU total, ~ `3.758 ms/generated`

#### 10b. `reproject-adaptive-multi-blend` (Rust)
Working as the current richest generated-frame backend in the Rust layer.

Validated with Rust layer on Steam Deck:
- `vkcube --c 120`
- `vkcube --c 600`
- `vkcube --c 120 --present_mode 0`
- `OMFG_ADAPTIVE_MULTI_TARGET_FPS=120 vkcube --c 120`
- `OMFG_ADAPTIVE_MULTI_TARGET_FPS=180 vkcube --c 120`
- `OMFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=1 OMFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=6 OMFG_ADAPTIVE_MULTI_INTERVAL_THRESHOLD_MS=1.0 vkcube --c 120`
- full Rust regression suite including `reproject-adaptive-multi-blend`
- advanced Deck validation via `scripts/run-advanced-steamdeck-validation.sh`
- target-FPS validation via `scripts/run-target-fps-steamdeck-validation.sh`

Observed:
- combines stronger reprojection, confidence/disocclusion-aware fallback, adaptive current-frame weighting, and adaptive multi-FG control in one backend
- inherits the richer reprojection quality controls and neighborhood hole-fill fallback used by `reproject-multi-blend`
- the same target/controller plumbing used by `adaptive-multi-blend` now also drives the reprojection-backed multi-FG path
- higher-count adaptive validation now also succeeds on Deck:
  - `requestedGeneratedFrames=6`
  - `emittedGeneratedFrames=6`
  - `minImages=3->7`
- targeted benchmark artifact:
  - `artifacts/steamdeck/rust/benchmark/reproject-multi-20260327-002943/`
  - `reproject-adaptive-multi-default` ~ `7.469 ms` GPU total, ~ `3.782 ms/generated`
  - `reproject-adaptive-multi-target180` ~ `7.466 ms` GPU total, ~ `3.785 ms/generated`

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

## Present timing instrumentation status

The Rust layer now has a first pass of present-timing instrumentation built around:
- `VK_KHR_present_id`
- `VK_KHR_present_wait`
- `VK_GOOGLE_display_timing`

Current behavior:
- the layer now appends `VK_KHR_present_id` and `VK_KHR_present_wait` to device creation when available so it can use them internally
- generated/original injected presents now route through timing-aware present helpers
- optional env controls exist:
  - `OMFG_PRESENT_TIMING=1`
  - `OMFG_PRESENT_WAIT=1`
  - `OMFG_PRESENT_WAIT_TIMEOUT_NS=<ns>`
- a Deck smoke run with timing enabled validated successful `present wait` results on injected presents

Current known limitation:
- on the current Deck `vkcube` path, this instrumentation successfully proved the `present_id` / `present_wait` plumbing, but the `VK_GOOGLE_display_timing` query path still did not report active samples through this app path, so stronger panel-side timing proof remains an open follow-up item

Current timing-validation artifact:
- `artifacts/steamdeck/rust/vkcube/multi-blend-present-timing/omfg-vkcube.log`

---

## Benchmark / autoperf status

We now also have a first repo-specific **autoperf loop** for repeated Deck benchmarking.

Current pieces:
- `scripts/run-steamdeck-benchmark-suite.sh`
  - supports both `OMFG_BENCHMARK_PRESET=full` and `OMFG_BENCHMARK_PRESET=decision`
- `scripts/run-steamdeck-multi-count-sweep.sh`
  - probes how far `multi-blend` scaling can be pushed on the Deck
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

Post-dynamic-headroom validation rerun status:
- `cargo test --locked`
- `./scripts/test-rust-layer.sh`
- `OMFG_LAYER_IMPL=rust ./scripts/build-linux-amd64.sh`
- `OMFG_LAYER_IMPL=rust ./scripts/run-layer-regression-suite.sh`
- `OMFG_LAYER_IMPL=rust ./scripts/run-advanced-steamdeck-validation.sh`
- `OMFG_LAYER_IMPL=rust ./scripts/run-target-fps-steamdeck-validation.sh`
- `OMFG_LAYER_IMPL=rust ./scripts/run-bfi-steamdeck-validation.sh`

All of the above completed successfully after the dynamic multi-FG headroom work.

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

A later dynamic-multiplier sweep on the Deck (`multi-count-sweep3-20260326-231948`) also produced an important pacing clue:
- `multi-blend` counts `1..20` all completed successfully once swapchain headroom scaled with requested multiplier
- average **CPU wall time per generated frame** stayed near the display refresh interval (`~11.0-11.3 ms/generated`)
- average **GPU time per generated frame** remained tiny relative to that wall

This strongly suggests the current architecture is still primarily constrained by **present/acquire pacing against the 90 Hz panel**, not by shader cost for the classical multi-blend backend.

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
Artifact roots are under:
- `artifacts/steamdeck/vkcube/`
- `artifacts/steamdeck/rust/vkcube/`

New OMFG runs write:
- `omfg-vkcube.log`
- `omfg-vkcube-*.log`

### vkgears
Artifact roots are under:
- `artifacts/steamdeck/vkgears/`
- `artifacts/steamdeck/rust/vkgears/`

OMFG runs write `omfg-vkgears.log`.

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
