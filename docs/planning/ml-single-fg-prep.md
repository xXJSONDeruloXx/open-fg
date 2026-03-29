# ML single-FG prep plan

This document describes the **prep work** for an eventual experimental ML-backed single generated frame mode, likely using a RIFE-family backend such as `rife-ncnn-vulkan`.

It is intentionally a **pre-implementation** plan.

## Goal

Make the current Rust Vulkan layer ready for a future experimental mode that:
- generates **one** synthetic frame between two real presented frames
- remains optional and explicitly experimental
- plugs into the existing swapchain / present interception architecture
- can fail safely and fall back cleanly when the ML backend is unavailable or too slow

## Non-goals

This prep plan does **not** try to:
- ship a production-ready ML backend immediately
- make ML the default mainline path
- solve multi-FG ML first
- solve full HUD/UI-aware ML compositing up front
- commit yet to one specific runtime integration strategy
  - embedded NCNN/Vulkan runtime
  - external `rife-ncnn-vulkan` sidecar
  - some later alternative

## Why prep first

The current analytical pipeline already provides:
- swapchain interception
- generated-frame scheduling
- extra present insertion
- smoke/regression/benchmark harnesses

What is missing for ML is mostly the **synthesis backend contract**, not the outer runtime shell.

So the right first step is to create clean seams for:
- input frame handoff
- output frame return
- timeout / failure behavior
- benchmarking and observability

## Proposed future mode

Planned experimental mode name:
- `rife-single-fg`

Behavioral intent:
- prime on first real frame like the current history-based modes
- on subsequent real frames, synthesize exactly one midpoint frame (`t=0.5`)
- present the generated frame using the current generated-frame insertion machinery
- if generation is unavailable, timed out, or invalid, skip the generated frame or fall back according to policy

## Prep work package

## 1. Create an explicit synthesis backend boundary

The current layer logic should be refactored so analytical shader synthesis and future ML synthesis both fit behind a clearer contract.

### Target shape
A future-friendly split should make it obvious which layer parts are:
- frame capture / history maintenance
- generation backend selection
- backend execution
- present scheduling / injection
- fallback accounting and logs

### Minimal contract to prepare
At minimum, the code should grow a conceptual interface equivalent to:
- input:
  - previous frame image / view
  - current frame image / view
  - target generated image / view
  - interpolation time (`0.5` for first ML landing)
  - current mode/config snapshot
- output:
  - success / failure / timeout / unavailable
  - optional backend timing stats
  - optional backend-specific diagnostics

This does not require introducing traits everywhere immediately, but it should make the future ML path a first-class backend rather than a one-off special case.

## 2. Add experimental mode plumbing without real inference yet

Prepare the runtime/config/logging surface for an ML single-FG mode.

### Planned mode string
- `rife-single-fg`

### Initial semantics
- experimental only
- single generated frame only
- no multi-FG support
- no silent promotion to default path

### What should be wired now
- mode parser acceptance
- mode labels / logging strings
- benchmark label support
- a clean disabled/unavailable fallback path

### What should not be faked
Do not pretend the ML backend exists if it does not. The mode should clearly log one of:
- backend unavailable
- backend disabled
- backend timed out
- backend failed

## 3. Define image handoff / staging expectations

Before a real RIFE backend is attempted, the layer needs an explicit answer for how frames move between the Vulkan layer runtime and the ML backend.

### Questions the prep work should answer
- what image format is the backend expected to consume?
- do we hand it Vulkan images directly, CPU buffers, or staging copies?
- where does color conversion happen if needed?
- who owns temporary buffers?
- what synchronization boundary marks "previous/current are ready"?
- what synchronization boundary marks "generated output is ready for present injection"?

### Recommendation
Document and prepare for a backend boundary that can support **both**:
- a future embedded Vulkan-capable ML runtime
- a crude temporary sidecar prototype if needed

That means avoiding a design that hardcodes only one path too early.

## 4. Define fallback policy up front

An ML backend is much more failure-prone than the current analytical shader modes.

The runtime should have explicit policy for:
- backend missing
- model missing
- backend initialization failure
- inference timeout
- invalid output
- per-frame transient failure

### Recommended first policy
For the initial experimental mode:
- first frame primes history
- on ML failure, **skip generated injection** rather than fabricating low-confidence garbage
- keep the app's original present path correct
- log a once-per-class warning plus counters

That preserves correctness while still making failures observable.

## 5. Add configuration surface for future ML backend selection

Even before implementation, the env/config names should be documented and reserved.

Recommended initial knobs:
- `OMFG_RIFE_ENABLED`
- `OMFG_RIFE_BACKEND`
  - e.g. `external`, `embedded`, `auto`
- `OMFG_RIFE_MODEL_DIR`
- `OMFG_RIFE_TIMEOUT_MS`
- `OMFG_RIFE_TTA`
- `OMFG_RIFE_GPU_ID`
- `OMFG_RIFE_SCALE`
- `OMFG_RIFE_LOG_LEVEL`

Not all of these need to do anything yet, but the naming should be settled early so docs/scripts/examples stop drifting.

## 6. Decide model/runtime packaging story

A real ML backend will need a packaging answer before code lands.

### Questions to settle
- are model files stored in-repo, fetched separately, or user-supplied?
- how are they deployed to remote Linux targets?
- what path convention should the wrapper and docs assume?
- how large is the deployment footprint?
- what are the licensing constraints for models and runtime?

### Recommendation
For the first experimental path, assume:
- model/runtime assets are **not** committed to the main repo by default
- the layer reads a user-supplied `OMFG_RIFE_MODEL_DIR`
- missing assets cause a clean unavailable fallback, not a crash

## 7. Add benchmark and observability hooks for ML now

If ML eventually lands, it needs to be measurable from day one.

Prep work should reserve places for:
- backend init success/failure logs
- per-frame backend time
- timeout counters
- generated/skipped counters
- benchmark labels separating analytical vs ML runs

Recommended future labels:
- `rife-single-fg`
- `rife-single-fg-timeout`
- `rife-single-fg-fallback`

## 8. Validation sequence for the eventual implementation

When real ML work begins later, the sequence should be:

1. offline captured-frame comparison against RIFE
2. backend abstraction + mode plumbing complete
3. local proof of image handoff and output ingestion
4. single-FG experimental runtime mode on desktop Linux GPU
5. benchmark cost vs `reproject-blend` / `optflow-blend`
6. failure/timeout behavior validation
7. only then broader runtime experimentation

## 9. Suggested implementation order for the prep branch

The prep work itself should likely land in this order:

1. document the future mode and config surface
2. factor backend selection / execution seam out of the current blend-path code
3. add `rife-single-fg` parser/logging/fallback plumbing
4. add backend-unavailable accounting and benchmark counters
5. keep the mode experimental and non-default

## Definition of done for prep work

This prep plan is complete when:
- the docs clearly describe the future ML single-FG shape
- a backend boundary exists in code or is at least made explicit enough to implement cleanly next
- `rife-single-fg` is no longer a naming/design question
- failure/fallback behavior is decided
- model/runtime/config/deployment questions are documented
- the next engineer can start real RIFE integration without first redesigning the runtime surface

## Current recommendation

Do **not** treat this as permission to make ML the mainline.

The intended next real ML step after prep remains:
- offline / captured-frame RIFE comparison first, or
- an explicitly experimental single-FG runtime backend on a separate follow-up branch
