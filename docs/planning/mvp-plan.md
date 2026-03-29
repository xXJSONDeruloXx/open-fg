# MVP plan

## Recommendation

Build a **clean-room Vulkan layer MVP** first.

This MVP should answer one question:

> Can we transparently intercept Vulkan presentation on Linux, synthesize a generated frame from final images only, and present it with acceptable pacing and stability?

If the answer is yes, we will have the right foundation to later:
- improve quality
- add vendor acceleration paths
- and potentially move into a compositor / `gamescope` path

---

## MVP scope

## In scope
- Linux
- Vulkan applications
- Proton / DXVK / VKD3D titles that ultimately present through Vulkan
- one generated frame between two real frames
- post-process only: no app motion vectors, no depth, no explicit engine integration
- fixed multiplier: **2x presentation** only
- visible proof of frame insertion
- basic pacing and synchronization
- debug overlays / logging

## Out of scope for MVP
- native OpenGL path
- Wayland-native desktop-wide support
- full compositor integration
- ML interpolation
- HDR correctness
- perfect VRR behavior
- advanced HUD masking
- dual-GPU / heterogeneous devices
- Nvidia OFA / FRUC fast path
- AMD-specific hardware path

---

## Success criteria

The MVP is successful if it can do all of the following on a real Linux machine:

1. Load as a Vulkan layer and correctly intercept swapchain presentation.
2. Present an extra frame between real frames without crashing or deadlocking.
3. Demonstrate a visible difference between:
   - passthrough mode
   - generated-frame mode
4. Run in at least these smoke tests:
   - `vkcube`
   - `vkgears`
   - one simple native Vulkan title or sample
   - one Proton title
5. Maintain stable operation for at least 10+ minutes under repeated present loops.

### Stretch success for MVP
- generated frame looks directionally better than simple duplicate-frame insertion
- basic camera pans show increased apparent smoothness

---

## Proposed architecture

## Core modules

### 1. Vulkan layer hook
Responsibilities:
- export layer entrypoints
- intercept:
  - `vkCreateInstance`
  - `vkCreateDevice`
  - `vkCreateSwapchainKHR`
  - `vkDestroySwapchainKHR`
  - `vkQueuePresentKHR`
- augment swapchain image usage if needed
- manage per-device / per-swapchain state

### 2. Swapchain context
Responsibilities:
- track swapchain images and formats
- manage source frame history
- manage generated-frame target images
- manage fences / semaphores / command buffers
- handle resize / swapchain recreation

### 3. Presentation scheduler
Responsibilities:
- control when generated frames are submitted
- choose initial pacing model
- enforce safe ordering between:
  - game-rendered present
  - generated present
  - reacquire / reinsert steps

### 4. Interpolation backend interface
Responsibilities:
- abstract the interpolation algorithm from the presenter
- allow several backends later

Initial backend contract:
- input: previous real frame + current real frame
- output: one generated frame at `t=0.5`
- optional inputs later:
  - HUD mask
  - distortion field
  - vendor optical-flow vectors

### 5. Config + diagnostics
Responsibilities:
- env vars or config file for:
  - enable/disable
  - debug overlay
  - pacing mode
  - flow scale
  - algorithm mode
- logging and on-screen debug markers

---

## Backend progression

## Backend 0 — duplicate / blend placeholder
Purpose:
- prove frame insertion path
- validate pacing path before quality work

Possible modes:
- duplicate previous frame
- duplicate current frame
- 50/50 blend of previous/current

This is not a quality target. It is an infrastructure milestone.

## Backend 1 — simple post-process interpolation
Purpose:
- first real image-derived generated frame

Suggested approach:
- luminance pyramid
- block matching / simple dense optical flow
- confidence / forward-backward consistency if feasible
- warp previous and current toward midpoint
- blend by confidence
- hole fill / inpaint

## Backend 2 — improved optical-flow backend
Purpose:
- quality improvement after MVP

Possible additions:
- better flow refinement
- disocclusion heuristics
- scene-cut detection
- primitive HUD suppression heuristics

## Backend 3 — optional vendor / ML paths
Possible future paths:
- Nvidia OFA / FRUC backend
- ML backend like RIFE family

---

## Milestones

## Milestone 0 — repository scaffold
Deliverables:
- project structure
- build system
- docs
- Linux-targeted layer manifest layout
- logging utilities

## Milestone 1 — pass-through Vulkan layer
Deliverables:
- layer loads successfully
- intercepts swapchain creation and present
- forwards everything correctly
- no generated frames yet

Exit criteria:
- no-op layer works on Linux smoke tests

## Milestone 2 — frame insertion proof
Deliverables:
- capture real presented frame into owned images
- reacquire extra swapchain image(s)
- insert a placeholder generated frame
- present generated frame + real frame in stable order

Generated frame can be:
- duplicate previous
- duplicate current
- or static blend

Exit criteria:
- visible 2x present behavior
- stable synchronization

## Milestone 3 — pacing v0
Deliverables:
- fixed pacing model
- basic FIFO-safe behavior
- debug timing markers
- present counters / telemetry

Exit criteria:
- generated frames display predictably
- no obvious runaway present bursts

## Milestone 4 — interpolation backend v0
Deliverables:
- compute optical flow or block-matching path
- warp/blend/inpaint midpoint frame
- debug views for motion/confidence

Exit criteria:
- visible smoothness improvement beyond duplicate-frame mode in basic camera pans

## Milestone 5 — Linux validation pass
Deliverables:
- smoke test matrix run on actual Linux hardware
- notes on:
  - X11 vs Wayland/XWayland
  - nested gamescope behavior
  - Proton behavior
  - AMD vs NVIDIA differences if available

---

## Linux test matrix for MVP

## Minimum
- `vkcube`
- `vkgears`
- one native Vulkan sample/app
- one Proton game

## Nice-to-have
- one Unreal title under Proton
- one Unity title under Proton
- one title with obvious HUD/text overlays
- one title inside nested `gamescope`

---

## Immediate work we can do now on macOS

Even before moving to Linux, we can make progress on:
- repository structure
- architecture and module boundaries
- shader/backend API design
- layer manifest and hook skeletons
- telemetry/logging design
- CI / build layout
- documentation

## What must wait for Linux
- actual layer loading behavior
- swapchain interception correctness
- real pacing / presentation behavior
- compositor interaction
- Nvidia-specific OFA / FRUC work
- VRR / tearing validation

---

## Recommended MVP implementation strategy

### Strong recommendation
Do **not** begin with the “best” interpolation algorithm.

Begin with the **best infrastructure proof**:
1. pass-through layer
2. generated placeholder frame
3. stable present scheduling
4. only then real interpolation

That order minimizes the risk of confusing:
- algorithm bugs
- synchronization bugs
- present/pacing bugs

---

## What we should build first

If implementation starts now, the first concrete deliverable should be:

## **A Linux Vulkan layer skeleton that can hook `vkQueuePresentKHR` and create per-swapchain state.**

That is the smallest real step toward the MVP and unlocks all later work.
