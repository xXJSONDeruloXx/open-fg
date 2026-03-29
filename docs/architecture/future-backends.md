# Future backends and research fit

This document maps the major interpolation / frame-generation families we have researched onto the current repo roadmap.

It is intended to answer two questions for future sessions:

1. **What should stay on the current mainline?**
2. **When should ML, vendor optical flow, and FidelityFX-style approaches enter the roadmap?**

---

## Current mainline assumption

The current mainline remains:

- **Linux-first**
- **real-time**
- **fully post-process where possible**
- **Vulkan explicit-layer based**
- **cross-vendor by default**
- **validated on real Linux hardware**

That means the default path still assumes we only reliably have access to:

- previous and current presented color frames
- swapchain / present timing observations
- our own injected GPU workloads
- optional display timing extensions when available

It does **not** assume we have application-provided:

- motion vectors
- depth
- camera matrices / camera basis vectors
- HUD-less scene buffers
- engine-integrated UI composition hooks

This assumption is the main reason some otherwise-impressive FG systems fit poorly into the near-term roadmap.

---

## Backend families

## 1. Current repo mainline: classical post-process FG

### What it is
The current repo path builds frame generation only from final presented images and swapchain interception, using progressively stronger heuristics.

### Current implemented examples
- `blend`
- `adaptive-blend`
- `search-blend`
- `search-adaptive-blend`
- `reproject-blend`
- `reproject-adaptive-blend`
- `multi-blend`
- `adaptive-multi-blend`

### Why it stays mainline
This path:
- matches the repo's current Vulkan-layer architecture
- works on Linux now
- can be validated on the Steam Deck now
- avoids model-runtime complexity
- avoids engine integration requirements
- is the fastest way to improve quality while preserving portability

### Mainline next steps
- debug / observability views for motion, confidence, ambiguity, disocclusion, and hole-fill behavior
- stronger reprojection inside multi-FG
- confidence / disocclusion handling
- hardware-agnostic post-process optical-flow style estimation with explicit benchmark cost comparison
- better display-timing instrumentation
- better pacing and presentation control when measurements show something beyond expected display-paced waiting
- dynamic swapchain / scheduling improvements

---

## 2. FSR3-style analytical frame generation

### What it is
Public FidelityFX Frame Generation 3.x is a **hybrid analytical pipeline**, not a simple final-frame-only blend.

The public docs and older Vulkan-capable code describe a structure that combines:
- game motion vectors
- optical flow
- depth / reconstructed depth
- disocclusion masks
- interpolation
- inpainting
- swapchain pacing / present scheduling
- HUD-less / UI-aware handling

### Why it matters
This is the strongest public description of a modern non-ML game-oriented FG pipeline.

It is highly relevant as an **architecture reference** for:
- optical-flow-like motion estimation
- disocclusion handling
- inpainting strategy
- UI / HUD strategy
- frame pacing and swapchain logic

### Why it does *not* drop directly into our current path
The public FSR3 FG path assumes inputs our explicit Vulkan layer does not have:
- depth
- motion vectors
- camera data
- HUD-less buffers or explicit UI integration

So we should not think of current FidelityFX FG as something we can just wire in unchanged.

### Roadmap fit
**Near-term inspiration, not near-term literal integration.**

The right interpretation for this repo is:
- copy the **ideas**, not the interface assumptions
- build a **post-process analog** of the useful parts

### Specific ideas worth stealing
- block/luma-pyramid optical-flow style estimation
- confidence-guided blending between multiple motion estimates
- explicit disocclusion masks
- hole filling / inpainting
- HUD-safe handling concepts
- stricter pacing / present-thread thinking

---

## 3. FSR4 / ML FidelityFX frame generation

### What it is
The current public FSR4 FG docs describe a newer ML-based frame-generation path.

### Why it matters
It confirms AMD is now treating ML frame generation as a first-class future direction.

### Why it is a poor fit *right now*
The public doc currently requires:
- Windows 11
- DirectX 12 Agility SDK
- AMD 9000 series or later
- stricter engine integration and richer per-frame metadata

This is a poor match for:
- Linux-first work
- Steam Deck validation
- explicit Vulkan post-process-only interception

### Roadmap fit
**Later conceptual reference only.**

FSR4-like ML belongs in this repo only if we later pivot into one of these:
- engine/plugin integration
- Windows-specific backend work
- vendor-specific ML work
- compositor/system-level path with richer metadata

It is **not** the near-term default direction for the current Linux layer mainline.

---

## 4. RIFE / RIFE-ncnn-vulkan

### What it is
RIFE is a neural intermediate-frame interpolation family. `rife-ncnn-vulkan` is a Vulkan-based inference implementation that runs on Linux and only needs two input images to generate an interpolated one.

### Why it matters
Compared with FidelityFX FG, RIFE is much closer to the repo's post-process assumptions:
- Linux support exists
- Vulkan support exists
- no app motion vectors required
- no depth strictly required
- no engine integration required in principle

### Why it still is not a drop-in solution
RIFE-style tools solve **frame synthesis**, but not the full real-time game stack:
- no swapchain pacing strategy
- no low-latency presentation architecture
- no UI / HUD integration story
- no current layer integration path
- model packaging/runtime integration still needed
- inference latency still needs to be budgeted

### Best current fit
**Immediate research branch / quality oracle.**

That means:
- compare our classical outputs against RIFE outputs on captured frame pairs
- use RIFE to estimate the quality ceiling available from ML
- decide whether the quality jump justifies runtime complexity

### Best future runtime fit
**Experimental optional single-FG backend first.**

If ML becomes worth pursuing at runtime, the first landing should likely be:
- one generated frame (`t=0.5`)
- desktop Linux GPUs first
- explicit experimental mode
- comparison against our best classical backend

The concrete prep work for that landing is documented in:
- `docs/planning/ml-single-fg-prep.md`

### Why not multi-FG first
ML multi-FG gets expensive quickly:
- repeated inference or multiple timesteps
- much higher latency risk
- more memory pressure
- much harder pacing/scheduling integration

So ML should start with **single-generated-frame experimentation** rather than immediate multi-FG replacement.

---

## 5. NVIDIA Optical Flow / FRUC

### What it is
NVIDIA exposes:
- hardware optical flow (`NVOFA`)
- Linux-capable SDK support
- Vulkan optical flow support on supported GPUs
- FRUC libraries for frame-rate up-conversion

### Why it matters
This is likely the strongest vendor-specific path to better motion estimation quality without immediately requiring a full neural interpolation stack.

### Strengths
- dedicated hardware motion-estimation path
- Linux support exists
- good fit for vendor-specific acceleration branch
- potentially lower cost than full ML interpolation

### Limitations
- NVIDIA-only
- not a cross-vendor default path
- still requires integration work around our current layer architecture

### Roadmap fit
**Medium-term optional backend.**

This fits earlier than a full cross-vendor ML mainline because:
- it directly improves motion estimation
- it preserves a largely analytical pipeline
- it can be kept vendor-specific behind capability detection

---

## Suggested roadmap placement

## Tier 1: mainline now
These should remain the default path for the repo right now:
- pacing / display timing instrumentation
- dynamic swapchain headroom / present scheduling
- stronger classical reprojection in multi-FG
- confidence / disocclusion handling
- post-process optical-flow style motion estimation
- better HUD / UI-safe handling heuristics

## Tier 2: immediate parallel research
These should run in parallel with the mainline but not replace it yet:
- offline / captured-frame comparison against `rife-ncnn-vulkan`
- quality-gap analysis between classical modes and ML outputs
- capture-driven experiments for scene classes where current heuristics fail badly

## Tier 3: optional future runtime branches
These should only enter runtime implementation after the mainline learns enough:
- NVIDIA optical-flow backend
- experimental runtime ML single-FG backend
- hybrid classical + ML refinement backend

## Tier 4: later / architecture pivot only
These do not fit the current mainline assumptions yet:
- FSR4-like ML integration
- engine/plugin-first metadata-rich FG
- Windows/DX12-specific FG backend as the default path

---

## Decision gates for when ML should move up the roadmap

ML should move from research to runtime implementation only if at least one of these becomes true:

1. **Quality gap is clearly large**
   - RIFE-style outputs materially outperform our best classical outputs on representative scenes.

2. **Latency budget is acceptable**
   - runtime inference can stay within a practical per-generated-frame budget on target desktop Linux GPUs.

3. **Packaging/runtime story is manageable**
   - models, inference runtime, deployment, and fallback behavior are all tractable.

4. **Classical path visibly plateaus**
   - further classical improvements stop paying off enough relative to complexity.

Until then, ML should remain:
- a research oracle
- an optional experimental backend
- not the default mainline

---

## Current recommendation

If a future session asks:

> "What should we build next?"

The default answer should be:

1. keep the mainline **classical / post-process / Linux-first**
2. borrow **FSR3 analytical ideas** where they fit post-process constraints
3. use **RIFE** as a research oracle and later optional runtime branch
4. consider **NVIDIA Optical Flow** as a vendor fast path
5. treat **FSR4 ML** as a later reference unless the architecture pivots

That is the roadmap fit that best matches the repo's actual constraints today.
