# Open questions and next Linux experiments

## Open questions that still matter

### 1. What is the minimum viable interpolation backend quality bar?
Questions:
- Is simple block-matching / optical-flow + warp enough to feel useful?
- Or will the first acceptable version require significantly better flow/confidence/inpainting?

Linux experiments needed:
- compare duplicate-frame insertion vs simple blend vs simple flow/warp backend
- record side-by-side captures on pans and HUD-heavy scenes

---

### 2. How bad is HUD/text distortion in practice for pure post-process mode?
Questions:
- Are static HUDs acceptable with no masking?
- Do we need masking immediately for MVP, or can it wait?
- What heuristic is cheapest and least wrong?

Linux experiments needed:
- run HUD-heavy titles
- compare:
  - no masking
  - naive high-contrast mask
  - temporal-diff mask
  - optional user-defined exclusion rectangle

---

### 3. What pacing model is good enough for MVP?
Questions:
- Is FIFO-only pacing acceptable for MVP?
- Do we need a presenter thread immediately?
- How much latency does the naive model add?

Linux experiments needed:
- placeholder frame insertion under FIFO
- measure visible pacing stability
- check nested `gamescope` vs direct compositor behavior

---

### 4. What is the correct initial target environment?
Questions:
- direct Vulkan apps only?
- Proton only?
- native + Proton from day one?
- X11 only, or XWayland nested under `gamescope`?

Linux experiments needed:
- smoke matrix:
  - `vkcube`
  - `vkgears`
  - one native Vulkan title
  - one Proton title
  - one nested `gamescope` session

---

### 5. Do we want permissive licensing from day one?
Questions:
- Is this intended to stay permissive / MIT / BSD-like?
- Are we okay with GPL obligations if we fork `lsfg-vk`?

Recommendation:
- default to **clean-room + permissive** unless there is a strong reason not to

Implication:
- `lsfg-vk` should remain a reference, not a code donor

---

### 6. How important is NVIDIA-specific acceleration in the early roadmap?
Questions:
- Should OFA / FRUC support be planned immediately?
- Or should it wait until vendor-neutral backend v0 exists?

Recommendation:
- defer until after vendor-neutral MVP proves the runtime architecture

---

### 7. Do we ultimately want a `gamescope` backend?
Questions:
- Is the long-term vision Vulkan-layer only?
- Or system-level compositor integration?

Recommendation:
- keep `gamescope` as a strategic phase 2 / 3 path
- do not block MVP on compositor integration

---

## Best next experiments on a Linux machine

## Experiment A — pass-through layer proof
Goal:
- prove we can load a custom Vulkan layer and safely intercept present

Success:
- pass-through behavior on `vkcube` / `vkgears`
- no crashes / no deadlocks

---

## Experiment B — frame insertion placeholder
Goal:
- prove generated-frame insertion path independent of interpolation quality

Method:
- insert duplicate or blended frame between real frames

Success:
- visible 2x presentation pattern
- stable synchronization

---

## Experiment C — simple optical-flow backend
Goal:
- evaluate whether a color-only v0 backend is directionally promising

Method:
- basic flow estimation
- warp previous/current toward midpoint
- blend and inpaint

Success:
- visible improvement over duplicate frame mode in simple camera motion

---

## Experiment D — HUD artifact survey
Goal:
- determine whether HUD masking is a phase-1 blocker

Method:
- test HUD-heavy titles
- record common failure modes

Success:
- clear answer on whether HUD mitigation must be part of MVP or can wait for v1

---

## Experiment E — nested `gamescope` compatibility
Goal:
- understand whether the layer MVP coexists acceptably with `gamescope`

Method:
- run sample apps inside nested `gamescope`
- compare pacing / presentation behavior

Success:
- identify whether `gamescope` is helpful, neutral, or problematic for the layer MVP

---

## Recommended immediate next action

When implementation begins, the first Linux runtime experiment should be:

## **Experiment A + B**

Meaning:
1. pass-through Vulkan layer
2. placeholder generated-frame insertion

That gives the fastest answer to the highest-risk architecture question.
