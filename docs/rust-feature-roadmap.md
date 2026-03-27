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
- First repo-specific autoperf harness:
  - fast decision benchmark subset
  - repeated-run aggregation
  - weighted baseline-vs-candidate comparison
  - optional full-suite promotion path

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
- two generated frames are now emitted between real frames in Rust
- LSFG-style target-FPS control now exists in `adaptive-multi-blend` via fractional generated-frame credit accumulation
- real Steam Deck target-FPS validation now covers `90`, `100`, `120`, and `150` FPS targets

Next likely path:
- generalize beyond the current `0..2` generated-frame range
- decouple controller quality from the current conservative synchronization overhead
- improve synchronization model beyond the current conservative approach
- validate with broader Deck finite-frame runs and additional modes

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
- both reprojection modes are now validated locally, in Linux Docker, and on the Steam Deck through smoke, long, and IMMEDIATE runs

Next likely path:
- propagate reprojection logic into multi-FG paths
- improve confidence and disocclusion handling around reprojected samples
- experiment with larger search windows / better patch metrics
- eventual optical-flow style pipeline

### 4. Temporal quality / disocclusion handling
Goal:
- reduce ghosting, double edges, and smear

Likely path:
- confidence masks
- adaptive biasing toward current frame
- edge-aware / difference-aware compositing
- disocclusion fallback rules

### 5. Pacing / latency improvements
Goal:
- reduce the current conservative synchronization model
- validate actual present pacing against display-side timing where possible

Likely path:
- use the new autoperf loop as the gate for pacing experiments before promoting to the full Deck suite
- reduce `vkQueueWaitIdle` dependence
- improve semaphore/fence lifetime strategy
- explore pacing thread / scheduling logic
- use the now-confirmed `VK_GOOGLE_display_timing` / `VK_KHR_present_id` / `VK_KHR_present_wait` support on the Deck test target for stronger panel-side validation

### 6. Advanced parity targets
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

With `reproject-blend`, `reproject-adaptive-blend`, and `adaptive-multi-blend` now working, the current mainline priority is:

## **bringing stronger reprojection into higher-quality multi-FG and richer confidence/disocclusion handling**

More specifically, the current ordering is:
1. pacing / present-timing instrumentation and scheduling improvements
2. dynamic multi-FG scaling and swapchain headroom improvements
3. stronger reprojection inside multi-FG
4. confidence / disocclusion / hole-filling improvements
5. post-process optical-flow style estimation

In parallel, but not as the default mainline:
- use `RIFE` / `rife-ncnn-vulkan` as a quality oracle on captured frame pairs
- evaluate NVIDIA Optical Flow as an optional vendor-specific motion-estimation backend
- keep FSR3 analytical concepts as algorithm references
- treat FSR4 ML as later, unless the architecture pivots toward richer engine metadata
