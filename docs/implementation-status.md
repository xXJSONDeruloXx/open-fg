# Implementation status

## Summary

We now have:
- a **working Linux Vulkan layer MVP** in C++
- a **working Rust parity port** for the current MVP scope
- a first **Rust shader-based generated-frame backend** (`blend`)
- a repeatable local + Linux + Steam Deck regression harness

This is beyond paper architecture at this point.

## Current status

### Working
- explicit Vulkan layer negotiation / loading
- instance/device/swapchain/present interception
- queue tracking
- swapchain mutation for extra image capacity
- remote build + deploy loop
- remote smoke test loop on Steam Deck
- log capture back into local artifacts
- Rust unit tests for mode parsing, swapchain mutation, sequencing, exports, and loader negotiation
- generic regression harness scripts:
  - `scripts/run-layer-regression-suite.sh`
  - `scripts/assert-vkcube-log.py`

### Rust parity status

Rust implementation location:
- `implementation/vk-layer-rust/`

Rust implementation currently has verified parity for the existing MVP feature set:
- `passthrough`
- `clear`
- `copy`
- `history-copy`

Rust also now has an additional next-step backend mode:
- `blend`

Validated via:
- local `cargo test --locked`
- Linux/x86_64 Docker build + test via `PPFG_LAYER_IMPL=rust ./scripts/build-linux-amd64.sh`
- full Deck smoke suite via `PPFG_LAYER_IMPL=rust ./scripts/run-layer-regression-suite.sh`

### Verified runtime modes on Steam Deck

The modes below are proven on the Steam Deck in the original C++ implementation, and now also in the Rust parity port for the main `vkcube` smoke path.

#### 1. `passthrough`
Working.

Validated with:
- `vkcube --c 120`

Observed:
- 120 real presents completed cleanly
- swapchain creation and present logging correct
- no crashes / no hangs

#### 2. `clear`
Working.

Validated with:
- `vkcube --c 120`

Observed:
- 120 real presents completed cleanly
- 120 generated placeholder presents completed cleanly
- extra frame insertion proven on real Linux hardware

#### 3. `copy`
Working.

Validated with:
- `vkcube --c 120`

Observed:
- 120 real presents completed cleanly
- 120 duplicated generated-frame presents completed cleanly
- swapchain image count bumped from 3 -> 5
- usage flags bumped to include `TRANSFER_SRC` + `TRANSFER_DST`
- per-frame copy from source app image into generated swapchain image succeeded across the full run

#### 4. `history-copy`
Working and now the best placeholder mode.

Validated with:
- `vkcube --c 120`

Observed:
- first frame primes private history
- subsequent frames present a generated placeholder frame derived from the **previous** real frame
- generated frame is presented **before** the current real frame
- 120 real presents completed cleanly
- 119+ generated placeholder presents completed cleanly after priming, with stable operation through the run
- additional stress run completed cleanly through **600 real frames** on `vkcube`
- `vkcube` also completed successfully in **IMMEDIATE** present mode with `history-copy`
- `vkcube` also completed successfully in **MAILBOX** present mode with `history-copy`
- private history image allocation and reuse works on the Steam Deck

This is the current strongest proof that the project direction is viable, because it demonstrates:
- persistent frame history
- timing-aligned placeholder insertion
- present ordering closer to actual frame generation

#### 5. `blend` (Rust)
Working as the first shader-based generated backend.

Validated with Rust layer on Steam Deck:
- `vkcube --c 120`
- `vkcube --c 600`
- `vkcube --c 120 --present_mode 0`

Observed:
- first frame primes history
- subsequent frames render a generated frame from a **50/50 blend** of previous and current frames
- generated frame is presented before the current real frame
- stable through 120-frame and 600-frame runs on Deck
- stable in **IMMEDIATE** present mode on Deck
- uses a real graphics pipeline + shader pass, not just transfer-copy placeholders

This is the first step beyond placeholder-only generation in the Rust implementation.

---

## Important technical insight from implementation

### The stable placeholder-frame paths were:
- increase swapchain image count
- acquire an extra image for the generated frame path
- either:
  - copy the current source frame into that acquired image (`copy`)
  - or maintain private history and present the previous real frame first (`history-copy`)
- drive both paths from explicit queue submission and explicit semaphore wiring
- use conservative synchronization and queue idle in test mode

That is not final-product pacing, but it is a real, working insertion path.

---

## Remaining gap to true frame generation

Right now the layer can do:
- **post-process frame insertion**
- **same-frame duplication**
- **previous-frame placeholder insertion with private history**
- **simple shader-based previous/current frame blending**

It still cannot do:
- **motion-aware interpolated frame generation**

So the next major milestone is replacing duplicate copy with:
- optical-flow / warp / blend / inpaint logic

---

## Artifacts

### vkcube
C++ MVP:
- `artifacts/steamdeck/vkcube/passthrough/ppfg-vkcube.log`
- `artifacts/steamdeck/vkcube/clear/ppfg-vkcube.log`
- `artifacts/steamdeck/vkcube/copy/ppfg-vkcube.log`
- `artifacts/steamdeck/vkcube/history-copy/ppfg-vkcube.log`
- `artifacts/steamdeck/vkcube/history-copy-long/ppfg-vkcube-long.log`
- `artifacts/steamdeck/vkcube/history-copy-immediate/ppfg-vkcube-immediate.log`
- `artifacts/steamdeck/vkcube/history-copy-mailbox/ppfg-vkcube-mailbox.log`

Rust parity port:
- `artifacts/steamdeck/rust/vkcube/passthrough/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/clear/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/copy/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/history-copy/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/blend/ppfg-vkcube.log`
- `artifacts/steamdeck/rust/vkcube/blend-long/ppfg-vkcube-blend-long.log`
- `artifacts/steamdeck/rust/vkcube/blend-immediate/ppfg-vkcube-blend-immediate.log`

### vkgears
- `artifacts/steamdeck/vkgears/clear/ppfg-vkgears.log`
- `artifacts/steamdeck/rust/vkgears/history-copy/ppfg-vkgears.log`

---

## Notable unresolved item

### `vkgears`
Under the current remote test setup, `vkgears` is not yet a useful validation target.

Observed behavior:
- layer negotiation occurs
- the process times out under the remote harness
- this remains true for both the C++ MVP and the Rust parity port
- we do not yet get the same clean create-device / create-swapchain / present trace as `vkcube`

That means `vkcube` is currently the reliable smoke-test target.

---

## Recommendation

The project has now crossed from:
- research only

to:
- working Linux runtime MVP with duplicate and history-based generated-frame insertion

The next best implementation step is:

## **add a real generated-frame backend behind the existing `history-copy` / `copy` infrastructure**

Recommended execution model now:
- keep the C++ layer as the already-proven oracle
- make the Rust port the primary place for new safety-minded implementation and test growth
- keep both on the same Deck smoke harness until interpolation parity is achieved

Meaning:
- keep the current queue/swapchain/present path
- treat `blend` as the first shader backend stepping stone
- replace raw copy / previous-frame placeholders / simple blending with motion-aware interpolation logic
