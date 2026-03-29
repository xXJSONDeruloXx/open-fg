# Testing strategy

## Current host reality

Current machine:
- macOS
- Apple Silicon (`arm64`, Apple M4 Pro)

That matters a lot.

This machine is good for:
- research
- design
- documentation
- source scaffolding
- build-system setup
- some static validation

This machine is **not** a trustworthy place to validate the actual Linux FG runtime.

---

## What we can do right now on macOS

## Safe / useful work
- write the codebase
- organize modules
- author shaders
- set up Linux build scripts and CI
- write unit tests for non-runtime logic
- inspect / diff reference projects
- do build-only checks in Linux containers or VMs

## Maybe-useful build environments
- Docker Desktop / containerized Linux for compile-only tasks
- a Linux VM for compile-only tasks
- remote Linux CI runners for build/test orchestration

These are useful for:
- catching missing includes
- catching CMake issues
- validating directory structure
- validating code formatting / linting

They are **not** enough for real frame-generation runtime validation.

---

## What we cannot meaningfully validate from this Mac

We should not treat the following as real test coverage from macOS-hosted containers/VMs:

- Linux Vulkan layer loading behavior
- swapchain replacement semantics on actual Linux GPU drivers
- queue-family behavior for generated-frame presentation
- present timing / pacing / latency
- VRR / tearing / V-Sync behavior
- `gamescope` compositor interaction in realistic gaming conditions
- NVIDIA Optical Flow / FRUC behavior
- any driver-level parity with Smooth Motion / AFMF-like systems

---

## Why containers are insufficient on macOS

### Docker Desktop on macOS
Docker Desktop runs Linux containers inside a **Linux VM**, not on the macOS kernel directly.

That means the container does **not** have the host’s native Linux Vulkan driver stack, because the host is not Linux.

Additional issue:
- official Docker GPU support for Docker Desktop is effectively aimed at **Windows + WSL2**, not macOS
- on macOS, Vulkan usually means **MoltenVK over Metal**, which is not the same thing as a Linux Vulkan ICD/driver environment

## Practical conclusion
Use containers on macOS for:
- build-only validation

Do not use them for:
- Linux Vulkan-layer runtime claims

Sources:
- Docker Desktop on Mac uses a Linux VM
- Docker Desktop GPU support is Windows/WSL2-focused
- macOS Vulkan is typically via MoltenVK

---

## Why VMs are insufficient on Apple Silicon macOS

There are two broad VM routes on Apple Silicon macOS:

### 1. Apple Virtualization.framework path
This gives Linux VMs a Virtio GPU 2D-style graphics model.

That is not a real target environment for our work.

It does **not** give us:
- native Linux AMD/NVIDIA Vulkan driver behavior
- true GPU passthrough for target desktop GPUs
- real queue/present behavior comparable to Linux gaming hardware

### 2. QEMU / UTM VirGL / Venus-style paths
These can expose some accelerated guest graphics paths, but they are:
- paravirtualized
- not equivalent to testing on real Linux gaming GPUs
- not equivalent to validating driver/layer/compositor timing behavior

## Practical conclusion
On Apple Silicon macOS, a Linux VM can be useful for:
- compile-only checks
- maybe some lightweight Vulkan API smoke experiments

But it is **not** a serious validation environment for this project.

If we want Apple GPU Vulkan specifically, the relevant route is **bare-metal Asahi Linux**, not a macOS-hosted VM. Even that would still not replace testing on target AMD/NVIDIA Linux gaming hardware.

Sources:
- Apple Virtualization.framework Linux graphics are 2D-oriented
- UTM docs and maintainer comments note lack of 3D support in the Apple virtualization path
- VirGL / Venus in QEMU are paravirtualized, not full passthrough
- Asahi Linux provides bare-metal Apple GPU Vulkan, which shows the difference between VM vs native host testing

---

## Recommended real test environments

## Best option: real Linux hardware
For this project, the real test environment should be a native Linux box.

### Minimum recommended Linux machine
- x86_64 Linux
- recent Mesa / RADV or recent NVIDIA proprietary driver
- Vulkan working natively
- ability to run:
  - native Vulkan samples
  - Proton
  - optionally `gamescope`

### Currently available real target
We now have a confirmed native Linux test target available remotely:
- Steam Deck / SteamOS
- see `docs/targets/steamdeck.md`

#### Running the full hardware regression suite

The regression suite is fully automated. To run it:

1. **Copy and fill in credentials** (one-time):
   ```bash
   cp .env.steamdeck.local.example .env.steamdeck.local
   # edit .env.steamdeck.local — set STEAMDECK_PASS
   ```
   The file is gitignored. If it exists and `STEAMDECK_PASS` is non-empty,
   the suite runs all hardware stages automatically.

2. **Run the suite:**
   ```bash
   bash scripts/run-layer-regression-suite.sh
   ```

What it does end-to-end:
- Runs `cargo test --locked` (125 unit tests, runs locally)
- Builds `libVkLayer_OMFG_rust.so` via Docker (`linux/amd64`) — no native Linux host needed
- Deploys the `.so` + manifest to the Deck over SSH/SCP
- Runs `vkcube --c 120` for **all 19 modes** on the Deck's AMD RADV GPU
- Pulls the layer log back and asserts expected markers via `scripts/assert-vkcube-log.py`
- Prints `Regression suite passed for rust` on success

If `STEAMDECK_PASS` is not set, the script exits after unit tests with a clear message — no partial or silent failure.

### Best matrix
#### Linux box A — AMD / RADV
Best for:
- open-source stack behavior
- `gamescope`
- Mesa / compositor friendliness

#### Linux box B — NVIDIA
Best for:
- comparison against Smooth Motion concepts
- OFA / FRUC experiments later
- vendor-specific present behavior

#### Optional handheld / Steam Deck class device
Best for:
- `gamescope`-centric experiments
- AMD APU path
- real-world “Lossless Scaling-like” usage scenarios

---

## Recommended phased test strategy

## Phase 0 — macOS authoring only
Do now:
- documentation
- code scaffolding
- build scripts
- CI

## Phase 1 — Linux build sanity
On Linux VM or container if needed:
- compile the code
- verify layer manifest packaging
- run static tests only

## Phase 2 — Linux smoke runtime on native hardware
On real Linux machine:
- load pass-through layer
- verify intercepts
- verify no crashes on `vkcube` / `vkgears`

## Phase 3 — frame insertion runtime
On real Linux machine:
- validate generated placeholder frame insertion
- log swapchain/present timing
- test resize / alt-tab / swapchain recreation

## Phase 4 — interpolation quality runtime
On real Linux machine:
- compare placeholder vs interpolation backend
- capture artifacts
- evaluate pacing and latency

---

## If we absolutely want a VM on this Mac

Use it only for:
- Linux build/test automation
- packaging checks
- smoke validation of ordinary Linux userspace assumptions

Do **not** treat it as:
- a performance environment
- a Vulkan driver environment representative of the target product
- a compositor/presentation timing environment

## Best possible VM expectation on this host
- “Does the project compile and run a trivial process?”

## Not a valid VM expectation on this host
- “Does the Linux post-process FG runtime behave correctly in a real gaming stack?”

---

## Bottom line

### Short answer
- **Container on macOS:** useful for build-only checks
- **Linux VM on Apple Silicon macOS:** useful for build-only / light smoke checks
- **Real native Linux hardware:** required for meaningful runtime testing

### Recommendation
Do not spend much time trying to validate the actual FG stack in a macOS-hosted Linux VM.

Use this Mac to:
- design
- write code
- structure the repo
- prepare CI

Then switch to a real Linux machine for runtime work as soon as the first hook/present prototype exists.
