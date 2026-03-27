# OMFG hardware-agnostic optical-flow v0 plan

This document defines the first hardware-agnostic optical-flow direction for the Rust OMFG layer.

## Goal

Move OMFG from patch-search reprojection toward a more FSR3-like analytical motion-estimation stack while staying:
- post-process only
- Linux-first
- Vulkan-based
- cross-vendor by default
- benchmarkable on real hardware

## Why this is next

The main analytical gap versus FSR3 FG is not just interpolation math; it is motion-estimation quality.

Current OMFG reprojection uses:
- local symmetric patch search
- confidence weighting
- ambiguity suppression
- disocclusion-aware fallback
- simple neighborhood hole fill

That is a solid post-process baseline, but it still struggles in the classic color-only failure cases:
- flat regions
- repeated textures
- thin detail
- large motion
- complex disocclusion
- lighting/transparency changes

A hardware-agnostic optical-flow path is the most meaningful next analytical step we can make without requiring engine motion vectors or depth.

## Current landing status

A first `optflow-blend` single-FG v0 is now implemented as a coarse-to-fine block-matching half-offset search inside the existing generated-frame shader path.

Current measured Deck result from `artifacts/steamdeck/rust/benchmark/optflow-compare-20260327-102630/`:
- `reproject-blend-default`: `avgCpuTotalMs=15.98`, `avgGpuCmdMs=3.898`
- `optflow-blend-default`: `avgCpuTotalMs=21.255`, `avgGpuCmdMs=11.101`
- `optflow-blend-fast`: `avgCpuTotalMs=15.1`, `avgGpuCmdMs=2.998`

Interpretation:
- the wider default optical-flow v0 profile is too expensive on the Deck today
- the tighter fast profile is already competitive with the current reprojection baseline
- the next optical-flow step should improve quality and confidence handling without regressing toward the expensive default profile

## Recommended v0 scope

Start with a **single-FG path first**.

Recommended new modes:
- `optflow-blend` ← **DONE** (v0)
- ~~later: `optflow-adaptive-blend`~~ → **DONE**: landed as shader mode 7
- ~~much later: `optflow-multi-blend`~~ → **DONE**: landed, uses shader mode 6 per frame

Why single-FG first:
- smaller implementation surface
- simpler validation
- clearer perf comparison
- easier to isolate motion-estimation quality changes from sequencing complexity

## Recommended architecture

## 1. Separate motion-estimation stage from final blending stage

Do not try to bury optical flow inside the existing final fragment pass only.

Recommended structure:
1. build a low-resolution or pyramid representation of the previous/current frames
2. estimate a flow field from previous -> current
3. optionally estimate a reverse flow or confidence proxy
4. generate the interpolated frame by warping around the flow field
5. apply confidence/disocclusion fallback and hole fill

That gives OMFG the right long-term shape:
- motion estimation becomes a reusable subsystem
- final synthesis becomes easier to tune independently
- future multi-FG and debug views can use the same motion field

## 2. v0 motion-estimation algorithm

Recommended first implementation:
- cross-vendor coarse-to-fine block-matching optical flow
- luma-first matching, with optional chroma contribution later
- 2-3 pyramid levels max for v0
- small search radius per level

Reasoning:
- fits current post-process assumptions
- easier than full dense variational flow
- much closer to FSR-style analytical thinking than current single-scale patch search
- can be implemented with ordinary Vulkan shader stages on any vendor

A reasonable v0 representation is:
- flow texture: `RG16_SFLOAT` or similar storing pixel or UV motion
- optional confidence texture: `R16_SFLOAT`

## 3. Confidence model for v0

Do not treat optical flow as automatically trustworthy.

Recommended v0 confidence sources:
- forward residual after warp
- local texture/gradient strength
- forward/backward consistency if reverse flow is added
- motion magnitude sanity clamp
- neighborhood agreement / smoothness heuristic

If reverse flow is too expensive for v0, start with:
- residual
- gradient weighting
- neighborhood consistency

## 4. Synthesis path

Use the motion field to build a symmetric interpolation, conceptually similar to what we already do with the best half-offset.

For a midpoint frame:
- sample previous frame at `uv + 0.5 * flow`
- sample current frame at `uv - 0.5 * flow`
- blend using adaptive alpha if enabled
- reduce trust in low-confidence or high-disocclusion regions
- fall back toward original frames and hole fill as needed

This keeps the existing OMFG confidence/disocclusion ideas relevant instead of replacing them.

## Comparison strategy

The v0 optical-flow path must be justified against the current reprojection baseline.

## Baseline cases

Primary baseline:
- `reproject-blend-default`

Secondary baseline later:
- `reproject-adaptive-blend-default`

## Minimum benchmark questions

For Deck and local benchmarking, measure:
- average CPU total time
- average GPU command time
- average GPU time per generated frame
- success / stability on smoke runs
- visual behavior under debug views

## Initial benchmark cases

Recommended first comparison set:
- `reproject-blend-default`
- `optflow-blend-v0`
- optional lighter variant: `optflow-blend-v0-lowres`
- optional tighter-search variant: `optflow-blend-v0-fast`

Only after that is green should we compare adaptive or multi-FG variants.

## Integration plan

### Phase 1: motion-field resources and plumbing
- add new mode parsing for `optflow-blend`
- allocate reusable flow/confidence resources in the swapchain-owned injected resources
- add shader modules and pipeline plumbing for the motion-estimation stage

### Phase 2: optical-flow v0 estimation
- create the first coarse-to-fine block-matching field
- expose basic debug views for motion magnitude / direction / confidence
- validate that the field is coherent on simple scenes

### Phase 3: synthesis using the flow field
- warp previous/current frames from the estimated field
- blend into a midpoint generated frame
- keep existing fallback / hole-fill concepts in place

### Phase 4: benchmark and Deck validation
- add benchmark preset cases
- compare cost versus `reproject-blend-default`
- validate smoke/long/immediate behavior on Deck if stable

## Debug-view dependency

This plan depends on debug views landing first.

At minimum, optical-flow v0 should be inspected with:
- motion vector / offset view
- confidence view
- disocclusion/fallback view

That is the main reason debug / observability should land before optical flow.

## Recommended initial knobs

Recommended envs for v0:
- `OMFG_OPTICAL_FLOW_LEVELS`
- `OMFG_OPTICAL_FLOW_RADIUS`
- `OMFG_OPTICAL_FLOW_SMOOTHNESS`
- `OMFG_OPTICAL_FLOW_CONFIDENCE_SCALE`
- `OMFG_OPTICAL_FLOW_LOWRES_FACTOR`

Do not overexpose too many knobs in the first landing. Start with a small set and expand only after measurements justify it.

## Non-goals for v0

Do not require these in the first optical-flow landing:
- hardware/vendor optical flow APIs
- ML interpolation
- multi-FG support
- HUD-safe handling
- explicit camera/depth reconstruction
- production-quality inpainting

Those are later steps.

## Success criteria

Optical-flow v0 is worth keeping if it meets all of these:
- passes local Rust tests and Linux Docker build
- runs on Steam Deck through the standard smoke path
- produces coherent debug views
- shows quality advantages on at least some motion cases versus patch-search reprojection
- has a measured, documented GPU cost versus the reprojection baseline

If it is strictly more expensive with no visible gain, it should remain experimental or be revised before promotion.

## Longer-term path after v0

If v0 is promising, the follow-up order should be:
1. improve confidence with stronger consistency checks ← **in progress**
2. improve disocclusion masking and hole fill ← **in progress** (shared with reprojection path)
3. ~~add adaptive variant~~ → **DONE**: `optflow-adaptive-blend` (shader mode 7) landed
   - combines coarse-to-fine block-matching with difference-weighted blend alpha
   - all optflow knobs apply; debug views work
   - locally validated: 71 tests pass, Docker build succeeds
   - Deck validation pending (no credentials in current host)
4. ~~evaluate propagation into multi-FG~~ → **DONE**: `optflow-multi-blend` landed
   - extends optical-flow generation to 2+ generated frames per real frame
   - uses shader mode 6 per frame with per-frame temporal alpha
   - shares `OMFG_MULTI_BLEND_COUNT` and headroom expansion with other multi-FG paths
   - locally validated: 71 tests pass, Docker build succeeds
   - Deck validation pending (no credentials in current host)
5. compare against vendor optical flow or ML oracle outputs where helpful

## Current next steps for the optflow stack

After Deck validation of the new modes:
1. run `OMFG_BENCHMARK_PRESET=optflow-quality` on Deck to measure `optflow-adaptive-blend` and `optflow-multi-blend` cost vs reprojection baseline
2. improve the coarse-to-fine search: larger per-level search radius, sub-pixel refinement, or temporal seed from previous frame
3. consider a separated multi-pass flow texture stage to reuse the motion field cleanly across multi-FG frames
4. compare `optflow-adaptive-blend` quality against `reproject-adaptive-blend` using real game captures or RIFE as oracle
