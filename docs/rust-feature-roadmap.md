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
- First shader-based generated backend:
  - `blend`
- Next synthesis step:
  - `adaptive-blend`
- First multi-FG stepping stone:
  - `multi-blend`

## Next implementation ladder

### 1. Multi-FG in Rust
Goal:
- generate more than one synthetic frame between real app frames

Current status:
- initial stepping stone achieved via `multi-blend`
- two generated frames are now emitted between real frames in Rust

Next likely path:
- generalize beyond fixed 2x generation
- make generated-frame count configurable/adaptive
- improve synchronization model beyond the current conservative approach
- validate with broader Deck finite-frame runs and additional modes

### 2. Adaptive FG controller
Goal:
- choose how aggressively to generate based on runtime conditions

Likely path:
- expose policy knobs via env/config
- start with simple heuristics:
  - present mode
  - generated-frame budget
  - frame time / queue pressure
  - scene-difference magnitude

### 3. Better motion-aware synthesis
Goal:
- move beyond blend-only interpolation

Likely path:
- frame-difference guided reprojection heuristics
- lightweight motion estimation experiments
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

Likely path:
- reduce `vkQueueWaitIdle` dependence
- improve semaphore/fence lifetime strategy
- explore pacing thread / scheduling logic

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
4. capture artifacts/logs
5. only then move to the next capability

## Current practical priority

With `multi-blend` now working, the next highest-value capability is:

## **adaptive FG frame-count control and richer motion-aware synthesis in Rust**
