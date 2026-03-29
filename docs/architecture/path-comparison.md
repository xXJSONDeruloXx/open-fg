# Path comparison: Vulkan layer vs compositor (gamescope)

## The two serious implementation paths

### Path A — Vulkan layer / swapchain interception
Reference inspirations:
- `lsfg-vk`
- NVIDIA Smooth Motion (`VK_LAYER_NV_present`)
- old FFX Vulkan frame-generation swapchain code

### Path B — compositor-level implementation
Reference inspiration:
- `gamescope`

---

## Executive summary

If the immediate goal is:

> prove a Linux-native, post-process frame-generation MVP that feels LSFG/AFMF-like

then the best first move is:

## Recommendation

### **Choose Path A for MVP**
But do it as a:
- **clean-room reimplementation**, not a straight `lsfg-vk` fork
- with a future plan to add a **gamescope path** later

Reason:
- it is the shortest route to a working proof
- it is the closest to the target UX
- it keeps scope contained to Vulkan/Proton titles first
- it lets us solve the hardest core problems before broadening scope

---

## Decision matrix

Scoring:
- 5 = strongest
- 1 = weakest

| Criterion | Path A: Vulkan layer | Path B: gamescope / compositor | Notes |
|---|---:|---:|---|
| Closeness to LSFG / AFMF style UX | 5 | 3 | Layer path is the direct analog to LSFG / Smooth Motion |
| MVP scope / time-to-first-proof | 5 | 2 | Layer path is narrower and more controllable |
| API coverage long term | 2 | 5 | Compositor can cover more than Vulkan-native apps |
| Presentation/pacing control | 4 | 5 | Compositor naturally owns presentation |
| HUD / cursor / overlay control | 2 | 4 | Compositor has better options for separating late-composed UI |
| Linux desktop integration | 2 | 5 | Compositor path is more system-level |
| Algorithm experimentation speed | 4 | 3 | Layer path is simpler for early shader/backend iteration |
| Licensing flexibility | 3 | 4 | Direct `lsfg-vk` fork implies GPL; gamescope is BSD-2; clean-room layer can stay permissive |
| Portability of code architecture | 4 | 3 | Layer path can later be reused in other presenters; compositor code is more platform-specific |
| Test complexity | 3 | 2 | Both are hard; gamescope path is broader and operationally heavier |

### Net read
- **Path A wins for MVP**
- **Path B wins for broader productization**

---

## Path A — Vulkan layer MVP

## What it means

Interpose on Vulkan application presentation, roughly like:
- intercept `vkCreateInstance`
- intercept `vkCreateDevice`
- intercept `vkCreateSwapchainKHR`
- intercept `vkQueuePresentKHR`
- capture real presented frames
- run post-process interpolation
- acquire additional swapchain images
- present generated frames in between real frames

## Why it is compelling

### 1. It matches the desired product behavior
This is the closest thing to:
- Lossless Scaling FG
- NVIDIA Smooth Motion
- a hypothetical Linux AFMF analog

### 2. It is already proven on Linux
By:
- `lsfg-vk`
- NVIDIA Smooth Motion

### 3. It keeps the problem focused
For MVP we can target:
- Vulkan-native games
- Proton Vulkan/DXVK/VKD3D workloads that resolve to Vulkan presentation
- nested gamescope scenarios later if useful

### 4. It lets us iterate the interpolation backend quickly
We can plug in:
- placeholder generated frames first
- then simple warp/blend
- then optical-flow v0
- then confidence / inpainting

## Biggest drawbacks

### 1. Vulkan-first, not universal
It does not automatically solve:
- native OpenGL apps
- arbitrary desktop windows
- global desktop compositor behavior

### 2. HUD handling is weaker
At layer level, we only see the final composed frame.
We do not naturally own late-stage cursor/HUD composition unless the upstream app or compositor helps us.

### 3. Licensing trap if we fork `lsfg-vk`
`lsfg-vk` is GPL-3.0-or-later.

That means:
- directly forking / copying substantial code from it likely pulls us into GPL obligations
- if we want permissive licensing, we should use it as an **architectural reference**, not as a code donor

## Best way to use this path

### Recommended stance
- **Do not base the MVP on copied `lsfg-vk` source**
- **Do** use `lsfg-vk` to study:
  - hook points
  - swapchain image strategy
  - synchronization model
  - pacing assumptions
- implement a fresh architecture with our own code

---

## Path B — gamescope / compositor path

## What it means

Implement frame generation inside a compositor-owned presentation pipeline, most likely with `gamescope`.

Potentially:
- game renders into surface
- compositor obtains real frames
- compositor generates intermediate frames
- compositor presents paced sequence to display

## Why it is compelling

### 1. Better long-term ownership of presentation
The compositor naturally owns:
- present timing
- queueing
- VRR behavior
- final display cadence

### 2. Better path for UI/cursor handling
Because the compositor may own or at least see more of the final composition process, it has better options for:
- cursor composition after FG
- overlay exclusion
- HUD-aware handling in some scenarios

### 3. Broader possible app coverage
Long term this is the best route if the goal becomes:
- desktop-wide FG
- non-Vulkan app coverage
- system-level feature rather than app-layer feature

## Biggest drawbacks

### 1. Much bigger MVP scope
A compositor path means touching:
- compositor render flow
- direct scanout / bypass rules
- nested vs embedded modes
- present timing subtleties
- potentially more surface classes and edge cases

### 2. Harder debugging surface
You are changing a larger and already complex runtime.
Failures are less isolated than in a layer path.

### 3. Still no magic source of motion vectors
Even at compositor level, if we remain fully post-process, we still only have final images.
So the core interpolation-quality problem remains.

## Best use of this path

Treat it as the likely **Phase 2 / Phase 3** target once:
- interpolation backend is credible
- pacing logic is better understood
- we know what HUD/cursor strategy actually works

---

## Licensing and reuse implications

| Source | License | Implication |
|---|---|---|
| `lsfg-vk` | GPL-3.0-or-later | Fine as a study reference; direct code reuse constrains downstream licensing |
| `gamescope` | BSD-2-Clause | More permissive for direct integration/reuse |
| old/new FidelityFX SDK | MIT | Very permissive for reuse of relevant code and concepts |
| `vkBasalt` | permissive / zlib-like | Good study/reference for Vulkan layer techniques |

## Practical recommendation on licensing

For maximum freedom:
- keep the new MVP codebase **clean-room and permissively licensed**
- use:
  - `lsfg-vk` as architecture reference only
  - FFX code as reusable permissive reference where appropriate
  - `gamescope` integration later if needed

---

## Final recommendation

## MVP path
### **Path A: clean-room Vulkan layer**

### Suggested positioning
- target: Vulkan / Proton games first
- algorithm: post-process optical-flow + warp/inpaint v0
- presenter: custom swapchain/pacing logic inspired by `lsfg-vk` + FFX v1.1.4
- non-goal for MVP: full desktop/system-wide FG

## Follow-on path
### **Path B: gamescope backend after MVP**

Use when we have:
- evidence the interpolation core is worth productizing
- a better understanding of latency/HUD tradeoffs
- a real need for broader app coverage beyond Vulkan-layer mode

---

## Bottom line

If we try to start in `gamescope`, we risk taking on too much surface area too early.

If we start with a clean-room Vulkan layer, we can prove the essential claim faster:

> “Can we build an open Linux post-process FG stack that actually inserts useful generated frames in real time?”

That is the right MVP question.
