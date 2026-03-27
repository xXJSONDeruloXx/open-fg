# Rust frame-generation roadmap

This document records the current implementation direction for the project.

## Primary direction

The Rust Vulkan layer is now the **primary implementation path**.

The C++ layer remains important as:
- the proven oracle/reference implementation
- a fallback when validating Rust behavior
- a comparison point during risky refactors

## Ongoing objective

Continue iterating on the Rust implementation until practical feature parity is achieved with the reference examples and architectures studied during research, including capabilities such as:
- single generated frame insertion
- multiple generated frames per real frame (**multi-FG**)
- adaptive generation strategies (**adaptive FG**)
- better synthesis quality than raw duplication or naive blending
- motion-aware interpolation
- confidence/disocclusion handling
- pacing and latency control
- future compositor-level integration where appropriate

## Current completed milestones

### Done
- Rust parity with the current C++ MVP for:
  - `passthrough`
  - `clear`
  - `copy`
  - `history-copy`
- Local Rust regression tests
- Linux/x86_64 Docker build/test path
- Steam Deck regression harness
- Simple software black-frame insertion mode:
  - `bfi`
- First shader-based generated backend:
  - `blend`
- Next synthesis step:
  - `adaptive-blend`
- First motion-search synthesis heuristic:
  - `search-blend`
- First combined motion-search + adaptive synthesis heuristic:
  - `search-adaptive-blend`
- First stronger symmetric patch-search reprojection heuristic:
  - `reproject-blend`
- First adaptive variant of the stronger reprojection heuristic:
  - `reproject-adaptive-blend`
- First multi-FG stepping stone:
  - `multi-blend`
- Adaptive multi-FG synthesis stepping stone:
  - `adaptive-multi-blend`
- First reprojection-backed higher-quality multi-FG mode:
  - `reproject-multi-blend`
- First adaptive reprojection-backed multi-FG mode:
  - `reproject-adaptive-multi-blend`
- First repo-specific autoperf harness:
  - fast decision benchmark subset
  - repeated-run aggregation
  - weighted baseline-vs-candidate comparison
  - optional full-suite promotion path
- Dynamic multi-FG swapchain headroom scaling:
  - new auto-expansion of swapchain image count for larger requested multi-FG counts
  - controlled by `OMFG_MULTI_SWAPCHAIN_MAX_GENERATED_FRAMES` (default `32`)
  - validated on the Steam Deck through a successful `multi-blend` count sweep from `1..20`
  - repeatable via `scripts/run-steamdeck-multi-count-sweep.sh`

## Mainline vs research branches

This repo now explicitly separates:

### Mainline
The mainline remains:
- Linux-first
- Vulkan-layer based
- post-process where possible
- cross-vendor by default
- validated on real Linux hardware

So the default backend path remains **classical / analytical** rather than ML-first.

### Parallel research branch
ML and vendor-specific paths are now tracked as explicit side branches rather than the default mainline:
- `RIFE` / `rife-ncnn-vulkan`
  - best immediate use: **quality oracle** on captured frame pairs
  - best later runtime use: **experimental single-FG backend**
- NVIDIA Optical Flow / FRUC
  - best use: **optional vendor-specific acceleration branch**
- FSR3-style analytical FG
  - best use: **algorithm and pacing inspiration** for the mainline
- FSR4 ML
  - best use: **later conceptual reference**, not near-term Linux mainline work

See `docs/future-backends.md` for the full rationale and branch placement.

## Next implementation ladder

### 1. Multi-FG in Rust
Goal:
- generate more than one synthetic frame between real app frames

Current status:
- initial stepping stone achieved via `multi-blend`
- adaptive synthesis variant achieved via `adaptive-multi-blend`
- higher-quality reprojection-backed variants now also exist:
  - `reproject-multi-blend`
  - `reproject-adaptive-multi-blend`
- the original validated mainline path emitted two generated frames between real frames in Rust
- swapchain headroom now scales automatically for larger requested multi-FG counts
- successful Deck sweep now validates `multi-blend` counts from `1..20`
- higher-quality reprojection-backed Deck validation now also covers:
  - `reproject-multi-blend` smoke / long / IMMEDIATE
  - `reproject-adaptive-multi-blend` smoke / long / IMMEDIATE
  - targeted higher-count smoke runs at `6` generated frames for both reprojection-backed multi-FG paths
- LSFG-style target-FPS control now exists in `adaptive-multi-blend` and `reproject-adaptive-multi-blend` via fractional generated-frame credit accumulation
- real Steam Deck target-FPS validation is now automated for:
  - `adaptive-multi-blend`
  - `reproject-adaptive-multi-blend` (`120` and `180` FPS smoke coverage)

Next likely path:
- add debug / observability views for reprojection-backed modes so motion/confidence failures are inspectable
- improve confidence/disocclusion handling inside the new reprojection-backed multi-FG modes
- add a hardware-agnostic post-process optical-flow path and benchmark it against the current reprojection baseline
- better separate controller policy from the intercepted present cadence model where that improves adaptive decisions
- validate with broader Deck finite-frame runs and additional quality settings

### 2. Adaptive FG controller
Goal:
- choose how aggressively to generate based on runtime conditions

Current status:
- first controller exists in `adaptive-multi-blend`
- it now supports both interval-based control and a target-FPS controller
- the target-FPS controller accumulates fractional generated-frame credit so effective multipliers can fluctuate over time

Next likely path:
- expand policy knobs via env/config
- better separate base-app cadence from current FG overhead
- combine multiple heuristics:
  - present mode
  - generated-frame budget
  - frame time / queue pressure
  - scene-difference magnitude
  - future GPU-side metrics if available

### 3. Better motion-aware synthesis
Goal:
- move beyond blend-only interpolation

Current status:
- `search-blend` and `search-adaptive-blend` established the first local motion-search heuristics
- `reproject-blend` now adds a stronger symmetric patch-search reprojection step
- `reproject-adaptive-blend` adds adaptive weighting on top of that reprojection path
- the current reprojection path now also has tunable quality controls:
  - `OMFG_REPROJECT_DISOCCLUSION_SCALE`
  - `OMFG_REPROJECT_HOLE_FILL_STRENGTH`
  - `OMFG_REPROJECT_HOLE_FILL_RADIUS`
  - `OMFG_REPROJECT_GRADIENT_CONFIDENCE_WEIGHT` (reduces confidence in flat regions; default `8.0`)
  - `OMFG_REPROJECT_CHROMA_WEIGHT` (blends between luma-only and RGB matching; default `0.3`)
  - `OMFG_REPROJECT_AMBIGUITY_SCALE` (suppresses confidence when multiple reprojection candidates are nearly tied; default `6.0`)
- that stronger reprojection path has now been propagated into multi-FG via:
  - `reproject-multi-blend`
  - `reproject-adaptive-multi-blend`
- all four reprojection-backed modes are now validated locally and in Linux Docker, and the base reprojection path is already validated on the Steam Deck through smoke, long, and IMMEDIATE runs
- focused Deck benchmarking now shows the reprojection-backed multi-FG path costs roughly `~3.76–3.79 ms/generated` with the current default reprojection settings

Next likely path:
- improve confidence and disocclusion handling around reprojected samples
- experiment with larger search windows / better patch metrics
- reduce visible failure cases in higher-count reprojection-backed multi-FG
- eventual optical-flow style pipeline

### 4. Debug views / observability
Goal:
- make motion, confidence, disocclusion, and hole-fill behavior visible enough to tune intentionally

Likely path:
- FSR-style debug views for selected motion / reprojection offsets
- confidence / ambiguity masks
- disocclusion / hole-fill masks
- source-selection / fallback visualization
- capture-friendly output modes that can be benchmarked and compared on Deck

### 5. Temporal quality / disocclusion handling
Goal:
- reduce ghosting, double edges, and smear

Likely path:
- confidence masks
- adaptive biasing toward current frame
- edge-aware / difference-aware compositing
- disocclusion fallback rules
- small neighborhood hole-fill / inpainting passes for higher-disocclusion regions
- tighter integration with the new debug / observability views

### 6. Hardware-agnostic optical flow
Goal:
- move from patch-search reprojection toward a more FSR3-like analytical motion-estimation core without requiring engine metadata

Current status:
- first `optflow-blend` single-FG v0 landed as a coarse-to-fine block-matching half-offset search
- validated on Deck; fast profile competitive with reprojection baseline
- `optflow-adaptive-blend` (shader mode 7) now adds adaptive current-frame bias on top of optflow
  - combines the coarse-to-fine pyramid search with the difference-weighted blend alpha from `adaptive-blend`
  - all optflow knobs apply; debug views work
  - validated locally: 71 tests pass, Docker build succeeds
- `optflow-multi-blend` now extends optical-flow generation to multi-FG (2+ frames per real frame)
  - uses shader mode 6 per generated frame position with per-frame temporal alpha
  - shares `OMFG_MULTI_BLEND_COUNT` with other multi-FG paths
  - dynamic swapchain headroom expansion applies
  - validated locally: 71 tests pass, Docker build succeeds
- `optflow-quality` benchmark preset now covers all three optflow variants vs reprojection-blend baseline
  - run with `OMFG_BENCHMARK_PRESET=optflow-quality ./scripts/run-steamdeck-benchmark-suite.sh`

Next likely path:
- Deck validation of `optflow-adaptive-blend` and `optflow-multi-blend` once credentials available
- improve confidence model: larger search window or better patch metrics for the coarse-to-fine path
- consider a separate multi-pass flow texture stage for cleaner motion field reuse in multi-FG
- keep vendor optical flow and ML as optional side branches rather than the default mainline

### 7. Pacing / latency improvements
Goal:
- keep validating actual present pacing against display-side timing where possible
- only pursue deeper pacing refactors when evidence shows behavior beyond expected display-paced waiting

Current status:
- first timing-aware present instrumentation now exists in the Rust layer
- the layer can now append and use `VK_KHR_present_id` / `VK_KHR_present_wait` when available
- a newer rerun of `scripts/run-present-timing-steamdeck-validation.sh` still confirms successful `present wait` results on injected presents after the recent analytical changes
- `VK_GOOGLE_display_timing` query hooks are now part of the layer, but the current `vkcube` Deck path still has not yielded useful past-presentation samples
- current acquire/present wall time under vsync-like Deck runs may simply reflect expected pacing against the active panel cadence, not necessarily wasted compute-side overhead

Likely path:
- use the new autoperf loop as the gate for pacing experiments before promoting to the full Deck suite
- strengthen the current timing instrumentation so it yields more useful panel-side evidence
- reduce `vkQueueWaitIdle` dependence where it proves to be unnecessary rather than assuming all wait time is a bug
- improve semaphore/fence lifetime strategy
- explore pacing thread / scheduling logic only when it helps visible pacing or controller quality
- keep using the now-confirmed `VK_GOOGLE_display_timing` / `VK_KHR_present_id` / `VK_KHR_present_wait` support on the Deck test target for stronger panel-side validation

### 8. Advanced parity targets
Longer-term targets include:
- multi-FG
- adaptive FG
- richer quality modes
- HUD-safe handling
- compositor/gamescope-style integration if the layer path tops out

## Validation rule

New capability work should continue following this loop:
1. add/extend Rust tests
2. build in Linux Docker
3. run Steam Deck `vkcube` smoke
4. for higher-risk motion-aware changes, run the advanced Deck validation set (`scripts/run-advanced-steamdeck-validation.sh`)
5. capture artifacts/logs
6. only then move to the next capability

## Current practical priority

With `optflow-adaptive-blend` and `optflow-multi-blend` now implemented locally (pending Deck validation),
the current mainline focus is:

## **validate the new optical-flow mode family on Deck, then improve quality across the full optflow stack**

More specifically, the current ordering is:
1. Steam Deck smoke validation for `optflow-adaptive-blend` and `optflow-multi-blend` once credentials available
2. `optflow-quality` benchmark run comparing new modes vs reprojection baseline on real hardware
3. improve optflow confidence model: stronger search, better patch metrics, or temporal flow stability
4. consider a separate multi-pass flow texture stage for cleaner motion field reuse across multi-FG frames
5. richer confidence / disocclusion / hole-filling improvements inside reprojection-backed multi-FG
6. cleaner controller-vs-backend separation for adaptive higher-quality multi-FG
7. pacing / present-timing follow-through as a validation/support track rather than assuming current vsync-paced waiting is the primary bottleneck

In parallel, but not as the default mainline:
- use `RIFE` / `rife-ncnn-vulkan` as a quality oracle on captured frame pairs
- evaluate NVIDIA Optical Flow as an optional vendor-specific motion-estimation backend
- keep FSR3 analytical concepts as algorithm references
- treat FSR4 ML as later, unless the architecture pivots toward richer engine metadata
