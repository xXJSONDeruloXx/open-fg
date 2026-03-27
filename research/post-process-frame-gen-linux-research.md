# Linux real-time post-process frame interpolation / frame generation research

Date: 2026-03-26

## Goal

Understand what exists today, what is possible, and what is missing for a **fully post-process, real-time frame interpolation / frame generation system on Linux** that:

- does **not** require app-provided motion vectors, depth, or explicit engine integration
- can sit at a **driver / Vulkan layer / compositor** level
- inserts generated frames to improve perceived smoothness, similar in spirit to:
  - Lossless Scaling Frame Generation (LSFG)
  - AMD AFMF
  - NVIDIA Smooth Motion

## High-level conclusion

A Linux solution is **absolutely possible**, but the problem splits into **two mostly independent systems**:

1. **Frame synthesis**
   - Generate an intermediate frame from two presented frames using only post-process information.
   - This can be done with:
     - heuristic / compute optical flow
     - ML interpolation (RIFE-style)
     - vendor hardware optical flow / FRUC on NVIDIA

2. **Presentation / pacing / latency management**
   - Actually injecting and timing the generated frame is a major engineering challenge on its own.
   - Existing successful Linux implementations all solve this at the **swapchain / present / compositor** level.

The most important finding is:

- **The presentation layer side is already proven on Linux** by:
  - `lsfg-vk`
  - NVIDIA Smooth Motion (`VK_LAYER_NV_present`)
  - AMD's older Vulkan FSR3 frame-generation swapchain code (in SDK v1.1.4)

The part still missing in open source is:

- a **good, vendor-neutral, fully post-process interpolation algorithm** with acceptable quality/latency for games
- plus the glue to make it robust across Linux compositors, VRR, overlays, HDR, Proton/native Vulkan, etc.

---

## What exists today

### 1) NVIDIA Smooth Motion on Linux: real, shipping, driver-level-ish proof

**What it is**
- NVIDIA ships **Smooth Motion** on Linux.
- It is enabled via `NVPRESENT_ENABLE_SMOOTH_MOTION=1`.
- It activates the implicit Vulkan layer `VK_LAYER_NV_present`.
- NVIDIA says it **overrides the application's presentation to inject additional frames**.

**Why it matters**
- This is the strongest proof that **Linux can support transparent, present-layer frame generation in real time**.
- It confirms the architecture is viable:
  - hook presentation
  - synthesize extra frames asynchronously
  - inject them via a Vulkan layer

**Key sourced details**
- `research/fetch/nvidia-smooth-motion-linux.html`
- NVIDIA README says:
  - supported on **GeForce RTX 40+**
  - supports **Vulkan applications**
  - uses `VK_LAYER_NV_present`
  - presents from an **asynchronous compute queue** by default
- Official docs also say Smooth Motion can be used for titles without native DLSS FG.

**Implications**
- A Linux Vulkan-layer implementation is not hypothetical.
- NVIDIA’s closed implementation is effectively a reference for the architecture, even though not open source.

Sources:
- https://download.nvidia.com/XFree86/Linux-x86_64/590.48.01/README/nvpresent.html
- https://docs.nvidia.com/datacenter/tesla/driver-installation-guide/gaming.html

---

### 2) `lsfg-vk`: the strongest community proof of concept on Linux today

Repo cloned to:
- `research/repos/lsfg-vk`

**What it is**
- A Vulkan layer that brings **Lossless Scaling Frame Generation** to Linux.
- It depends on the proprietary Windows `Lossless.dll` from the Steam app.

**Why it matters**
- It proves that **post-process frame generation can be made to work on Linux via Vulkan-layer swapchain interception**.
- It does **not** require app motion vectors/depth.
- It is currently the most directly relevant open-source Linux implementation architecture-wise, even though the interpolation core is still proprietary.

**Important source observations**
- README: Vulkan layer that hooks Vulkan apps and generates extra frames using Lossless Scaling FG.
- `docs/Journey.md` explains the architecture:
  - hooks `vkCreateInstance`, `vkCreateDevice`, `vkCreateSwapchainKHR`, `vkQueuePresentKHR`
  - adds transfer usage to swapchain images
  - copies presented swapchain frames into private images
  - runs frame generation on a **separate Vulkan device**
  - uses shared memory / semaphores between hook side and frame-gen side
  - reinserts generated frames with extra `vkQueuePresentKHR` calls
  - forces **FIFO / V-Sync-like present mode** for pacing
- `lsfg-vk-layer/src/swapchain.cpp` shows the exact mechanics:
  - copy current swapchain image into source image
  - schedule generation
  - acquire extra swapchain images
  - blit generated frames back into swapchain images
  - present generated frames, then present original frame

**Why this is useful to build from**
- It already solves a lot of the hard Linux-specific problems:
  - Vulkan layer hooking
  - swapchain image access
  - synchronization
  - frame injection
  - pacing strategy
- If you swapped out the proprietary LSFG backend for an open interpolation backend, `lsfg-vk` is one of the clearest launch points.

**Main limitation**
- The actual interpolation algorithm is still from proprietary `Lossless.dll`.

Sources:
- https://github.com/PancakeTAS/lsfg-vk
- local files:
  - `research/repos/lsfg-vk/README.md`
  - `research/repos/lsfg-vk/docs/Journey.md`
  - `research/repos/lsfg-vk/lsfg-vk-layer/src/swapchain.cpp`
  - `research/repos/lsfg-vk/lsfg-vk-backend/include/lsfg-vk-backend/lsfgvk.hpp`

---

### 3) AMD AFMF: conceptually very relevant, but not available on Linux

**What it is**
- AMD AFMF is a driver-level frame generation feature.
- Official AMD docs say AFMF 2.1 supports DX11/DX12/Vulkan/OpenGL.

**Critical Linux finding**
- Official AMD documentation still lists **Windows 10/11 only**.
- Current AMD Linux software / release notes do **not** expose AFMF on Linux.
- AMD’s Linux strategy now centers on **Mesa/RADV**, and AMDVLK was discontinued.

**Why it matters**
- AFMF is the closest conceptual target to what you want.
- But there is **no official Linux AFMF runtime or public Linux AFMF API** to build on.

Sources:
- https://www.amd.com/en/products/software/adrenalin/afmf.html
- https://www.amd.com/en/resources/support-articles/release-notes/RN-AMDGPU-UNIFIED-LINUX-25-20-3.html
- https://github.com/GPUOpen-Drivers/AMDVLK/discussions/416

---

### 4) FidelityFX / FSR3 frame generation: useful pieces, but not turnkey for pure post-process

Repos cloned to:
- `research/repos/FidelityFX-SDK`
- `research/repos/FidelityFX-SDK-v1.1.4`

This needs to be split into two realities:

#### 4a) Current SDK (v2.2.0): Vulkan currently not supported

Current README says:
- `Vulkan is currently not supported in SDK 2.2`

So if you try to build a Linux Vulkan frame-generation path from the **latest** SDK, you immediately hit a wall.

Local source:
- `research/repos/FidelityFX-SDK/README.md`

#### 4b) Older SDK (v1.1.4): had real Vulkan frame-generation + swapchain code

In `v1.1.4`, AMD shipped:
- Vulkan backend code
- Vulkan frame interpolation swapchain
- GLSL shaders for optical flow and frame interpolation
- swapchain replacement / queuePresent replacement helpers

Key local files:
- `research/repos/FidelityFX-SDK-v1.1.4/sdk/include/FidelityFX/host/ffx_frameinterpolation.h`
- `research/repos/FidelityFX-SDK-v1.1.4/sdk/include/FidelityFX/host/ffx_opticalflow.h`
- `research/repos/FidelityFX-SDK-v1.1.4/sdk/include/FidelityFX/host/backends/vk/ffx_vk.h`
- `research/repos/FidelityFX-SDK-v1.1.4/sdk/src/backends/vk/FrameInterpolationSwapchain/FrameInterpolationSwapchainVK.cpp`
- `research/repos/FidelityFX-SDK-v1.1.4/docs/techniques/super-resolution-interpolation.md`
- `research/repos/FidelityFX-SDK-v1.1.4/docs/techniques/frame-interpolation.md`
- `research/repos/FidelityFX-SDK-v1.1.4/docs/techniques/optical-flow.md`

**Why it matters**
- AMD already had a **Vulkan frame-generation swapchain** and pacing implementation.
- This is a major open-source reference for Linux/Vulkan presentation architecture.

**But it is not pure post-process**
The older FFX frame interpolation API explicitly wants:
- `depth`
- `motionVectors`
- `dilatedDepth`
- `dilatedMotionVectors`
- `reconstructedPrevDepth`
- `cameraNear / cameraFar / FOV`
- camera basis vectors
- optional `HUDLess` and `distortionField`
- plus optical-flow outputs

The docs say frame interpolation is designed to work with:
- `FfxOpticalFlow`
- `FfxFsr3Upscaler`

And the algorithm itself explicitly contains both:
- **game motion vector field**
- **optical flow vector field**
- then blends those two results

So the open FFX path is **not** a drop-in LSFG/AFMF equivalent.
It is a **hybrid engine-integrated FG algorithm**, not a final-frame-only algorithm.

**Still very valuable**
- The **presentation / swapchain / pacing** pieces are extremely relevant.
- The **optical flow implementation** is useful as a building block.
- But to make it fully post-process, you would need to replace or redesign the parts that assume engine motion/depth/camera data.

---

### 5) NVIDIA Optical Flow SDK / FRUC: very relevant vendor-specific building blocks

Repo cloned to:
- `research/repos/NVIDIAOpticalFlowSDK`

Fetched docs:
- `research/fetch/nvidia-fruc-guide.html`
- `research/fetch/nvidia-nvofa-programming-guide.html`
- `research/fetch/nvidia-nvofa-application-note.html`
- `research/fetch/nvidia-opticalflow-download.html`

#### 5a) NVOFA / VK_NV_optical_flow

NVIDIA’s Optical Flow SDK 5.0 supports Linux and adds **native Vulkan optical flow**.

Important sourced points:
- Vulkan optical-flow interface works on Linux.
- Requires:
  - `VK_NV_optical_flow`
  - `VK_KHR_timeline_semaphore`
  - queue family supporting `VK_QUEUE_OPTICAL_FLOW_BIT_NV`
- Vulkan mode is **Ampere+**, not Turing.

This is important because it gives a possible high-performance Nvidia-only path for **post-process motion estimation** directly from images.

#### 5b) FRUC library

NVIDIA also ships **FRUC** (Frame Rate Up Conversion):
- takes two frames
- outputs interpolated frame
- Linux supported via `libNvFRUC.so`
- internally uses **NVOFA + CUDA**

This is arguably the closest vendor library to a post-process frame interpolation engine on Linux.

**Limitations**
- closed
- NVIDIA-only
- Linux path is CUDA-centric, not a simple vendor-neutral Vulkan drop-in

**What it proves**
- Hardware-assisted post-process interpolation from frames alone is practical on Linux.
- A compositor or layer could use this on Nvidia if CUDA/Vulkan interop is acceptable.

Sources:
- https://developer.nvidia.com/opticalflow/download
- https://docs.nvidia.com/video-technologies/optical-flow-sdk/nvofa-programming-guide/index.html
- https://docs.nvidia.com/video-technologies/optical-flow-sdk/nvfruc-programming-guide/index.html
- https://registry.khronos.org/vulkan/specs/latest/man/html/VK_NV_optical_flow.html

---

### 6) `gamescope`: best compositor-side launch point

Repo cloned to:
- `research/repos/gamescope`

**Why it matters**
`gamescope` already provides many of the runtime properties a Linux post-process FG system needs:
- owns presentation/composition
- already uses async Vulkan compute in composition path
- already handles scaling and latency-sensitive presentation
- already deals with VRR / tearing / frame pacing concerns
- can run nested around games, including Proton titles

README highlights:
- async Vulkan compute composite path
- low-latency design goals
- FSR/NIS upscaling already integrated
- Reshade-like post-processing support already exists

**Why it is promising**
If your goal is **API-agnostic post-process FG** instead of only Vulkan-hook FG, a compositor path may be cleaner than app hooking:
- works even when app is OpenGL or otherwise not easy to layer-hook
- naturally owns final presentation timing
- can integrate cursor/overlay composition later in the pipeline

**Main challenge**
- compositor only sees the final image, so quality depends entirely on post-process motion estimation
- direct scanout / bypass paths may need to be disabled or controlled
- integrating robust interpolation into a compositor is substantial work

Local sources:
- `research/repos/gamescope/README.md`
- `research/repos/gamescope/src/*`

---

### 7) `vkBasalt`: proof that Vulkan layer post-processing is practical, but not enough alone

Repo cloned to:
- `research/repos/vkBasalt`

Why it matters:
- `vkBasalt` is an open Vulkan post-processing layer.
- It hooks swapchain creation / present and rewrites image usage and presentation behavior.
- It proves the viability of **portable Vulkan-layer post-processing on Linux**.

Why it is not enough by itself:
- it does image effects, not frame insertion / pacing / extra presents at FG scale
- but its architecture reinforces that a Vulkan layer is a legitimate path for a Linux FG implementation

Local sources:
- `research/repos/vkBasalt/README.md`
- `research/repos/vkBasalt/src/basalt.cpp`

---

### 8) `rife-ncnn-vulkan`: strongest open color-only algorithm family, but not a finished gaming solution

Repo cloned to:
- `research/repos/rife-ncnn-vulkan`

**What it is**
- Vulkan-backed open-source inference runner for **RIFE**.
- Pure image-to-image interpolation from two frames.

**Why it matters**
- This is one of the most credible open-source **fully post-process interpolation** algorithm families.
- It is cross-vendor at runtime via Vulkan.

**Why it is not the answer by itself**
- existing implementations are aimed at image/video interpolation pipelines, not low-latency swapchain insertion
- no Linux shipping compositor/layer integration around it
- ML inference cost, memory, and frame pacing complexity are nontrivial for live gaming
- HUD/text/particle artifacts remain hard in final-frame-only interpolation

This is best seen as an **algorithm candidate**, not a full Linux FG stack.

Source:
- `research/repos/rife-ncnn-vulkan/README.md`

---

### 9) Community experiments already in this direction

#### `linux-fg`
Repo cloned to:
- `research/repos/linux-fg`

Status from source:
- very early/WIP
- X11 capture
- Vulkan shaders for block matching and interpolation
- native Wayland capture not implemented
- output path currently reads back to CPU/SDL surface
- `Scaler::ProcessFrame()` currently scales and displays but does **not** actually invoke the interpolation path end-to-end

This is useful as a sketch of the idea, but not yet a real foundation for low-latency FG.

Relevant files:
- `research/repos/linux-fg/README.md`
- `research/repos/linux-fg/shaders/motion.comp`
- `research/repos/linux-fg/shaders/interpolate.comp`
- `research/repos/linux-fg/src/scaler.cpp`
- `research/repos/linux-fg/src/window_capture.cpp`

#### `lsfg-vk-afmf`
Repo cloned to:
- `research/repos/lsfg-vk-afmf`

Status from source:
- currently **scaffolding only**
- build system works
- FidelityFX integration is still TODO
- source is stubbed out
- docs assume older FFX Vulkan-era layouts and do not reflect current SDK reality

This is conceptually aligned with your goal, but right now it is not yet a technical implementation.

Relevant files:
- `research/repos/lsfg-vk-afmf/src/afmf.cpp`
- `research/repos/lsfg-vk-afmf/CMakeLists.txt`
- docs under `research/repos/lsfg-vk-afmf/docs/`

---

## What the field looks like, distilled

### Proven on Linux today

- **Closed / shipping**
  - NVIDIA Smooth Motion: yes, Linux, Vulkan, present-layer injection
  - Lossless Scaling via `lsfg-vk`: yes, Linux, Vulkan layer, but proprietary interpolation core

- **Open / architectural building blocks**
  - old FFX Vulkan frame-generation swapchain code: yes
  - gamescope compositor path: yes
  - vkBasalt-style Vulkan layer path: yes

- **Open / algorithm candidates**
  - FFX Optical Flow: yes
  - RIFE / ncnn Vulkan: yes

### Missing

A mature, open-source, Linux-native solution that is all of the following at once:
- fully post-process
- real-time for games
- app-agnostic
- vendor-neutral
- low-latency
- robust UI/HUD handling
- robust VRR/pacing/present integration

That specific combination does **not** appear to exist yet.

---

## Core technical constraints for a real system

### A) Presentation/pacing is as important as interpolation

This is the single easiest thing to underestimate.

Evidence from:
- `lsfg-vk`
- FFX frame generation swapchains
- NVIDIA Smooth Motion docs

All successful designs have substantial machinery for:
- extra swapchain images
- acquiring/presenting additional images
- queue management
- async compute / separate present queues
- timing threads / pacing logic
- synchronization / semaphores / fences
- VRR / V-Sync / tearing behavior
- minimizing CPU/GPU pipeline bubbles and latency

Any serious Linux FG effort needs a first-class **present scheduler**, not just a shader.

### B) Pure color-only interpolation is possible, but quality is the hard part

Pure post-process means you lose:
- authoritative engine motion vectors
- depth / occlusion information
- knowledge of HUD vs world
- camera cuts / object classes / animation state

That causes artifacts in:
- thin geometry
- particles
- alpha effects
- HUD/text
- disocclusions
- very fast camera pans
- repeated patterns / noisy textures

This is why FFX blends optical flow **with** game motion vectors, instead of replacing them.

### C) HUD / UI handling is a major unsolved problem for post-process FG

Open FFX docs explicitly devote major API surface to UI composition.

For a pure post-process solution, you likely need one of:
- compositor-owned UI/cursor composition after FG
- HUDless extraction / diffing
- heuristic UI masks
- user-provided exclusion regions
- ML-based HUD detection

Without this, text and HUD artifacts will dominate perceived quality.

### D) Capture-based approaches are much weaker than layer/compositor approaches

`linux-fg` currently shows why:
- X11 capture
- CPU-side readback/display path
- no true low-latency presentation ownership
- Wayland capture is still difficult / restricted / latency-heavy

For real FG, the capture path should ideally be avoided.

The viable places are:
- **Vulkan layer / swapchain interception**
- **nested compositor (gamescope)**
- possibly desktop compositor integration

---

## Best candidate architectures

### Option 1: Build on `lsfg-vk` architecture, replace backend

**Best for:** Vulkan + Proton titles first

Use:
- `lsfg-vk` hook/swapchain/pacing structure

Replace backend with:
- vendor-neutral compute optical flow + interpolation
- or Nvidia-specific OFA/FRUC backend where available
- optional ML backend later

**Pros**
- shortest path to something game-usable
- proven Vulkan injection model
- already aligned with LSFG-like behavior

**Cons**
- Vulkan-centric, not universal desktop
- OpenGL/non-Vulkan apps still need another path
- UI handling remains hard

### Option 2: Implement in `gamescope`

**Best for:** system-level / compositor-level path, broader API coverage

Use:
- gamescope as frame owner / pacer / presenter
- final-image interpolation in compositor

**Pros**
- presentation ownership is natural
- not tied to only Vulkan-native apps
- better place to separate cursor/compositor-owned overlays
- possibly easiest route to broad Linux practicality

**Cons**
- algorithm only sees final frames
- direct scanout / bypass considerations
- deeper compositor engineering effort
- may add compositor-level latency if not done carefully

### Option 3: Hybrid path

- Vulkan layer for Vulkan/Proton apps
- gamescope path for broader compositor mode
- shared interpolation backend

This is probably the end-state if the goal is wide Linux usefulness.

---

## Best candidate algorithm stacks

### Stack A: Classic compute optical flow + warp + inpainting

**Most realistic first milestone**

Use pieces from:
- FFX optical flow ideas
- FFX inpainting / confidence ideas
- LSFG-style final-frame-only backend integration

Likely ingredients:
- luma pyramid
- block / patch matching or dense optical flow
- confidence / forward-backward consistency
- scene-cut detection
- warp previous/current toward t=0.5
- blend by confidence
- inpaint holes
- HUD suppression heuristics

**Why this is a good first target**
- more controllable latency than ML
- pure Vulkan compute possible
- vendor-neutral
- easier to integrate into layer/compositor than a neural net inference runtime

### Stack B: Nvidia-only accelerated backend

Two variants:
- NVOFA (`VK_NV_optical_flow`) + custom warp/inpaint
- FRUC (`libNvFRUC.so`) if acceptable

**Pros**
- likely best performance on Nvidia
- real Linux support exists

**Cons**
- vendor lock-in
- not a universal solution

### Stack C: ML interpolation backend (RIFE/IFRNet-style)

**Best for quality experiments, not necessarily first ship target**

**Pros**
- strongest open color-only interpolation family
- can outperform simple block/flow heuristics visually in some scenes

**Cons**
- latency / scheduling / VRAM cost
- harder real-time guarantees
- difficult cross-vendor low-latency deployment for live games
- still poor around HUD/text unless specially handled

---

## What specifically lacks today

1. **Open vendor-neutral Linux FG presenter stack**
   - Something like `lsfg-vk`, but with a fully open interpolation backend.

2. **Open pure-post-process algorithm tuned for games, not offline video**
   - Most open algorithms are either engine-assisted (FSR3 FG) or offline/video-centric (RIFE tools).

3. **Good HUD strategy**
   - This is the biggest perceptual blocker for final-frame-only FG.

4. **Cross-vendor acceleration story**
   - Nvidia has OFA/FRUC.
   - AMD has no public Linux AFMF or Linux hardware optical-flow equivalent.
   - Intel has no obvious plug-and-play equivalent here for Linux FG.

5. **Current AMD open Vulkan path continuity**
   - Old FFX Vulkan FG code exists, but latest SDK removed Vulkan support.
   - So anyone building on FFX today has to either:
     - pin old Vulkan-capable code,
     - vendor/fork it,
     - or port it forward themselves.

---

## Practical recommendation if building this for real

### Phase 1: solve presentation first

Start from either:
- `lsfg-vk` (Vulkan layer route), or
- `gamescope` (compositor route)

Do **not** start from screen capture.

### Phase 2: implement a simple open interpolation backend

Prefer:
- classic compute optical flow / block matching / confidence / warp / inpaint

Why:
- fastest route to measurable results
- lowest dependency risk
- easiest to profile and tune

### Phase 3: add vendor fast paths

- Nvidia OFA / FRUC path
- maybe optional ML path later

### Phase 4: attack HUD and latency

These are what separate a cool demo from a usable product.

---

## How this fits the current repo roadmap

For the current repo, the most practical branch placement is:

### Mainline now
- keep the default path **classical / analytical / post-process**
- use the current Vulkan layer + Rust implementation as the primary path
- borrow ideas from FSR3-style analytical FG where they fit post-process constraints

### Immediate parallel research
- use `rife-ncnn-vulkan` as a **quality oracle** on captured frame pairs
- compare its outputs against the repo's best classical modes
- use that to decide whether runtime ML is worth the complexity

### Medium-term optional runtime branches
- NVIDIA Optical Flow / FRUC backend on supported Linux/NVIDIA systems
- experimental runtime ML single-FG backend
- hybrid classical + ML refinement ideas

### Later / pivot-only branches
- FSR4-style ML integration
- engine/plugin-first integration requiring richer metadata
- Windows/DX12-specific ML-first FG path

The key reason for that ordering is simple:
- **FSR3-style analytical ideas fit the current repo's constraints conceptually**
- **RIFE fits as a synthesis oracle and later optional backend**
- **FSR4 ML does not fit the current Linux explicit-layer assumptions well enough to be near-term mainline work**

---

## My view on the strongest foundations from this research

### Best architectural foundation
- **`lsfg-vk`** for Vulkan-layer injection mechanics
- **old FFX Vulkan frame-generation swapchain code** for richer pacing/presenter design
- **`gamescope`** if you want broader compositor-level ownership

### Best algorithm foundation
- **FFX optical flow concepts** for a lightweight game-oriented non-ML motion estimator
- **RIFE-family research** for future quality experiments

### Best immediate reality check
- **NVIDIA Smooth Motion on Linux** proves the category is already real
- **`lsfg-vk`** proves the community can do it with Vulkan layers today

---

## Cloned / fetched material

### Cloned repos
- `research/repos/lsfg-vk`
- `research/repos/linux-fg`
- `research/repos/lsfg-vk-afmf`
- `research/repos/FidelityFX-SDK`
- `research/repos/FidelityFX-SDK-v1.1.4`
- `research/repos/NVIDIAOpticalFlowSDK`
- `research/repos/gamescope`
- `research/repos/vkBasalt`
- `research/repos/rife-ncnn-vulkan`

### Fetched docs/pages
- `research/fetch/nvidia-smooth-motion-linux.html`
- `research/fetch/nvidia-gaming-linux.html`
- `research/fetch/nvidia-fruc-guide.html`
- `research/fetch/nvidia-nvofa-programming-guide.html`
- `research/fetch/nvidia-nvofa-application-note.html`
- `research/fetch/nvidia-opticalflow-download.html`

---

## Bottom line

If the target is:

> “end-to-end real-time Linux frame interpolation that is fully post-process and does not depend on app motion vectors/depth”

then the answer is:

- **Yes, it is feasible.**
- **No, there is not yet a mature open Linux implementation that fully delivers it.**
- The closest practical foundations are:
  - **`lsfg-vk`** for layer/present architecture
  - **old FFX Vulkan swapchain code** for pacing/presenter design
  - **gamescope** for compositor ownership
  - **FFX optical flow / RIFE / NVIDIA OFA-FRUC** for the interpolation core depending on goals

The biggest gap is not “can Linux inject frames?”
The answer to that is already **yes**.

The biggest missing piece is:
- a **good open post-process interpolation core plus robust HUD/latency handling**.
