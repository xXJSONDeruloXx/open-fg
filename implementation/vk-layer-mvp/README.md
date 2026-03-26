# Vulkan layer MVP

Current MVP implementation for a Linux post-process frame-generation layer.

## What exists now

A working explicit Vulkan layer that can:

- intercept instance/device/swapchain/present calls
- track queue + swapchain state
- modify swapchain creation for extra images / transfer usage
- run on a real Steam Deck target
- smoke test successfully with `vkcube`

## Current modes

### `passthrough`
No extra frame insertion.
Useful for validating that the layer loads cleanly and does not break presentation.

### `clear`
Proof-of-insertion mode.
After the real app frame is presented, the layer acquires another swapchain image, clears it to a visible green frame, and presents it.

Purpose:
- prove extra frame acquisition
- prove extra present scheduling
- prove swapchain image recycling and synchronization

### `copy`
Stable duplicate-frame insertion mode.
The layer:
1. acquires an additional swapchain image
2. copies the app’s current present image into that extra image
3. presents the original frame
4. presents the copied frame as a generated placeholder

This is **not interpolation yet**.
It is a stable **duplicate-frame insertion** path that proves:
- present interception
- extra-image acquisition
- command submission
- copy into generated frame
- two presents per app frame

### `history-copy`
Current best timing-aligned placeholder mode.
The layer:
1. stores the current real frame into a private history image
2. on the next frame, acquires an extra swapchain image
3. copies the **previous real frame** into that extra image
4. presents that previous-frame placeholder first
5. then presents the current real frame

This is still **not real interpolation**, but it is a better approximation of the intended frame-generation ordering than `copy` mode.

It proves:
- persistent frame history
- generated-frame-first ordering
- separate original/generated present semaphores
- post-process insertion using prior-frame state

## What it is not yet

Not yet implemented:
- real interpolation / optical flow
- pacing thread
- latency optimization
- HUD masking
- compositor integration

## Real target tested

Primary validated runtime target:
- Steam Deck / SteamOS
- RADV / VANGOGH
- Vulkan sample: `vkcube`

## Key implementation insight

The current stable placeholder paths work by:
- increasing swapchain image count
- acquiring one extra swapchain image for the generated frame path
- either:
  - copying the current frame into that extra image (`copy`)
  - or storing current into history and presenting the previous frame first (`history-copy`)
- submitting explicit transfer work that waits on app render-complete semaphores
- signaling separate semaphores for original vs generated presentation
- using `vkQueueWaitIdle` in test-oriented modes so semaphore reuse is safe and predictable

That last step is not production-grade, but it makes the MVP robust.

## Build

From project root:

```bash
./scripts/build-linux-amd64.sh
```

## Deploy to Steam Deck

```bash
export STEAMDECK_PASS='...'
./scripts/deploy-steamdeck-layer.sh
```

## Smoke tests

### vkcube
```bash
export STEAMDECK_PASS='...'
export PPFG_LAYER_MODE=passthrough
./scripts/test-steamdeck-vkcube.sh

export PPFG_LAYER_MODE=clear
./scripts/test-steamdeck-vkcube.sh

export PPFG_LAYER_MODE=copy
./scripts/test-steamdeck-vkcube.sh

export PPFG_LAYER_MODE=history-copy
./scripts/test-steamdeck-vkcube.sh
```

### vkgears
```bash
export STEAMDECK_PASS='...'
export PPFG_LAYER_MODE=copy
./scripts/test-steamdeck-vkgears.sh
```

## Artifact locations

Local logs and copied test output end up under:
- `artifacts/steamdeck/vkcube/`
- `artifacts/steamdeck/vkgears/`

## Recommended next step

Replace `copy` mode’s raw duplicate-frame copy with a real generated-frame backend:
- first simple warp / blend / optical-flow v0
- then proper confidence / disocclusion handling
