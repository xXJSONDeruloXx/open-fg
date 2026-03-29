# OMFG wrapper and environment variables

This document covers two related things:

1. the canonical Steam Deck wrapper script now tracked in-repo at `scripts/steamdeck-omfg-wrapper.sh`
2. the OMFG environment variables currently used by the Vulkan layer and its Deck test harness

## What the wrapper does

`/home/deck/omfg.sh` is a **launch wrapper**. It does not replace the game executable. It:

1. prepares the Steam/Proton container so it can see OMFG files
2. exports Vulkan-layer environment variables
3. optionally exports mode/tuning/debug knobs
4. `exec`s the original Steam/Proton launch command unchanged

In plain English: **the game still launches normally, but OMFG inserts itself into the Vulkan pipeline before the game starts presenting frames**.

## Canonical wrapper in this repo

Tracked file:

- `scripts/steamdeck-omfg-wrapper.sh`

Typical Deck install:

```bash
scp scripts/steamdeck-omfg-wrapper.sh deck@steamdeck:/home/deck/omfg.sh
ssh deck@steamdeck 'chmod +x /home/deck/omfg.sh'
```

Optional pattern for persistent config:

```bash
cat >/home/deck/omfg.env <<'EOF'
export OMFG_LAYER_IMPL=rust
export OMFG_LAYER_MODE=reproject-blend
export OMFG_WRAPPER_LOG_FILE=/home/deck/post-proc-fg-research/logs/omfg-wrapper.log
EOF

export OMFG_WRAPPER_ENV_FILE=/home/deck/omfg.env
/home/deck/omfg.sh <original command here>
```

## Accepted boolean values

Every OMFG boolean env parsed by the Rust layer accepts:

- true: `1`, `true`, `yes`, `on`
- false: `0`, `false`, `no`, `off`

Anything else falls back to that variable's default.

## Hot-reload config file

The Rust layer now supports an **optional hot-reload TOML file** selected by:

- `OMFG_HOT_CONFIG_PATH=/path/to/hot.conf.toml`

Canonical checked-in file:
- `config/omfg-live.toml`

When this env var is set, OMFG checks that file roughly every `250ms` and overlays any matching values on top of the process environment.

### Supported TOML shapes

Either top-level keys:

```toml
OMFG_LAYER_MODE = "reproject-blend"
OMFG_DEBUG_VIEW = "confidence"
OMFG_REPROJECT_CONFIDENCE_SCALE = 6.0
```

Or an `[env]` / `[omfg]` table:

```toml
[env]
OMFG_LAYER_MODE = "search-adaptive-blend"
OMFG_BLEND_ADAPTIVE_STRENGTH = 3.0
OMFG_SEARCH_BLEND_RADIUS = 2
```

Only OMFG-related keys are consumed:

- keys starting with `OMFG_`
- `ENABLE_OMFG_RUST`
- `DISABLE_OMFG_RUST`

If the file becomes invalid while the game is running, OMFG keeps the **last successfully parsed config** and logs a warning instead of crashing.

### Which vars are realistically hot-adjustable?

These are the knobs that are meaningfully re-read during frame generation / present handling and are therefore the best candidates for live tuning:

- `OMFG_LAYER_MODE`
- `OMFG_DEBUG_VIEW`
- blend/search vars
- reprojection vars
- optical-flow vars
- adaptive multi-FG vars
- `OMFG_BFI_PERIOD`
- `OMFG_BFI_HOLD_MS`
- `OMFG_VISUAL_HOLD_MS`
- `OMFG_PRESENT_TIMING`
- `OMFG_PRESENT_WAIT`
- `OMFG_PRESENT_WAIT_TIMEOUT_NS`
- `OMFG_BENCHMARK`
- `OMFG_BENCHMARK_LABEL`
- `OMFG_COPY_ORIGINAL_PRESENT_FIRST`
- `OMFG_BLEND_ORIGINAL_PRESENT_FIRST`
- `OMFG_HISTORY_COPY_FREEZE_HISTORY`
- generated-acquire timeout vars

### Which vars are **not** truly live?

Some vars are only consulted at startup or swapchain/device creation, so changing them in the hot file usually affects the **next launch**, **next device**, or **next swapchain recreation**, not the current live frame stream:

- `OMFG_LAYER_LOG_FILE`
- `OMFG_SWAPCHAIN_IMAGE_BUMP_OVERRIDE`
- `OMFG_CREATE_DEVICE_DEBUG`
- `OMFG_CREATE_DEVICE_APPEND_TIMING_EXTENSIONS`
- `OMFG_CREATE_DEVICE_APPEND_TIMING_FEATURES`
- `ENABLE_OMFG_RUST`
- `DISABLE_OMFG_RUST`
- wrapper-only vars like `OMFG_BASE_DIR`, `OMFG_LAYER_DIR`, `VK_LAYER_PATH`, etc.

---

# 1) Core wrapper / injection vars

These control whether the OMFG Vulkan layer gets injected at all.

| Variable | Accepted values | Default | Effect |
|---|---|---:|---|
| `OMFG_BASE_DIR` | path | `/home/deck/post-proc-fg-research` | Base directory used by the canonical wrapper. |
| `OMFG_WRAPPER_ENV_FILE` | path | unset | Optional file sourced by `scripts/steamdeck-omfg-wrapper.sh` before exporting vars. |
| `OMFG_HOT_CONFIG_PATH` | path to TOML file | unset | Optional live-reload config overlay. The Rust layer polls it and overlays matching OMFG values on top of the process env. |
| `OMFG_LAYER_IMPL` | `rust` | `rust` | Selects the implementation family targeted by helper scripts. This repository is now Rust-only. |
| `OMFG_LAYER_DIR` | path | `${OMFG_BASE_DIR}/deploy/vk-layer-${OMFG_LAYER_IMPL}` | Directory prepended to `VK_LAYER_PATH` by the canonical wrapper. |
| `OMFG_LAYER_NAME` | Vulkan layer name | `VK_LAYER_OMFG_${OMFG_LAYER_IMPL}` | Layer name exported into `VK_INSTANCE_LAYERS`. |
| `OMFG_DISABLE_LAYER` | boolean | `0` | Wrapper-level hard off switch. If true, the game runs without OMFG injection. |
| `ENABLE_OMFG_RUST` | boolean-ish env gate | `1` when wrapper enables Rust layer | Loader gate expected by `VkLayer_OMFG_rust.json`. |
| `DISABLE_OMFG_RUST` | boolean-ish env gate | unset | Rust-layer disable gate. If true, loader should ignore the Rust layer. |
| `VK_LAYER_PATH` | path list | wrapper prepends OMFG layer dir | Makes Vulkan able to find the OMFG manifest + shared object. |
| `VK_INSTANCE_LAYERS` | layer name(s) | `VK_LAYER_OMFG_rust` in Rust wrapper use | Requests that Vulkan load the OMFG instance layer. |
| `PRESSURE_VESSEL_FILESYSTEMS_RW` | path list | wrapper prepends OMFG base dir | Lets Steam Runtime / Proton see OMFG files from inside the pressure-vessel container. |
| `OMFG_LAYER_LOG_FILE` | path | unset in raw layer, `${LOG_DIR}/omfg.log` in canonical wrapper | File append target for OMFG runtime logs. If unset, logs only go to stderr/nowhere depending on launch path. |
| `OMFG_WRAPPER_LOG_FILE` | path | unset | Optional log file for wrapper-level diagnostics (argv, cwd, injected mode, etc.). |
| `OMFG_TRUNCATE_LAYER_LOG` | boolean | `1` in canonical wrapper | Whether the canonical wrapper removes the old `OMFG_LAYER_LOG_FILE` before launch. |
| `OMFG_LOG_DIR` | path | `${OMFG_BASE_DIR}/logs` | Canonical wrapper log directory. |

---

# 2) Runtime mode selection

## `OMFG_LAYER_MODE`

Canonical accepted values:

- `passthrough`
- `clear`
- `bfi`
- `copy`
- `history-copy`
- `blend`
- `adaptive-blend`
- `search-blend`
- `search-adaptive-blend`
- `reproject-blend`
- `reproject-adaptive-blend`
- `optflow-blend`
- `optflow-adaptive-blend`
- `multi-blend`
- `adaptive-multi-blend`
- `reproject-multi-blend`
- `reproject-adaptive-multi-blend`
- `optflow-multi-blend`
- `optflow-adaptive-multi-blend`

Default:

- layer parser default: `passthrough` if unset / unknown
- canonical wrapper default: `reproject-blend`
- current test scripts often override explicitly

Accepted aliases in `src/config.rs`:

| Canonical mode | Also accepted |
|---|---|
| `passthrough` | unset / unknown values fall back here |
| `clear` | `clear-test` |
| `bfi` | `black-frame`, `black-frame-insertion`, `bfi-test` |
| `copy` | `copy-test`, `duplicate` |
| `history-copy` | `history`, `copy-prev`, `history-copy-test` |
| `blend` | `blend-test`, `history-blend`, `blend-prev-current` |
| `adaptive-blend` | `adaptive`, `adaptive-blend-test`, `blend-adaptive` |
| `search-blend` | `motion-search`, `motion-search-blend`, `search-blend-test` |
| `search-adaptive-blend` | `adaptive-search-blend`, `motion-search-adaptive`, `search-adaptive-blend-test` |
| `reproject-blend` | `vector-reproject-blend`, `motion-reproject`, `reproject-blend-test` |
| `reproject-adaptive-blend` | `adaptive-reproject-blend`, `vector-reproject-adaptive`, `reproject-adaptive-blend-test` |
| `optflow-blend` | `optical-flow`, `optical-flow-blend`, `optflow-blend-test` |
| `optflow-adaptive-blend` | `optflow-adaptive`, `optical-flow-adaptive`, `optflow-adaptive-blend-test` |
| `optflow-multi-blend` | `optflow-multi-fg`, `optflow-multi`, `optical-flow-multi`, `optflow-multi-blend-test` |
| `optflow-adaptive-multi-blend` | `optflow-adaptive-multi-fg`, `optflow-adaptive-multi`, `optical-flow-adaptive-multi`, `optflow-adaptive-multi-blend-test` |
| `reproject-multi-blend` | `reproject-multi-fg`, `reproject-multi-blend-test`, `multi-reproject-blend` |
| `reproject-adaptive-multi-blend` | `adaptive-reproject-multi-blend`, `reproject-adaptive-multi-fg`, `reproject-adaptive-multi-blend-test` |
| `multi-blend` | `multi-fg`, `multi-fg-test`, `multi-blend-test` |
| `adaptive-multi-blend` | `adaptive-multi-fg`, `adaptive-multi-blend-test`, `multi-blend-adaptive` |

### Layman descriptions for the main modes

| Mode | What it does in plain English |
|---|---|
| `passthrough` | OMFG intercepts but does not synthesize extra frames. |
| `clear` | Fills generated output with a flat clear color for testing plumbing. |
| `bfi` | Inserts black frames between real frames. |
| `copy` | Repeats the current/last real frame as the generated frame. |
| `history-copy` | Reuses a previous frame from history. |
| `blend` | Blends neighboring frames together. |
| `adaptive-blend` | Blend mode with strength adjusted by motion/interval heuristics. |
| `search-blend` | Blend mode with a local matching/search heuristic. |
| `search-adaptive-blend` | Search-based blend plus adaptive behavior. |
| `reproject-blend` | Estimates motion and warps pixels to build a synthetic frame. |
| `reproject-adaptive-blend` | Reprojection with adaptive generated-frame behavior. |
| `optflow-blend` | Uses optical-flow style motion estimation to synthesize a frame. |
| `optflow-adaptive-blend` | Optical flow with adaptive behavior. |
| `multi-blend` | Emits more than one generated frame between real frames. |
| `adaptive-multi-blend` | Multi-FG count changes dynamically. |
| `reproject-multi-blend` | Multi-FG using reprojection as the synthesis backend. |
| `reproject-adaptive-multi-blend` | Adaptive multi-FG using reprojection. |
| `optflow-multi-blend` | Multi-FG using optical flow. |
| `optflow-adaptive-multi-blend` | Adaptive multi-FG using optical flow. |

---

# 3) Layer tuning vars

These are read directly by the Rust Vulkan layer.

## Shared / scheduling / logging

| Variable | Accepted values | Default | Effect |
|---|---|---:|---|
| `OMFG_SWAPCHAIN_IMAGE_BUMP_OVERRIDE` | integer `u32` | unset | Forces an additional swapchain image-count bump. Useful for experiments when generated frames need more headroom. |
| `OMFG_BENCHMARK` | boolean | `false` | Enables benchmark-oriented logging / labeling paths. |
| `OMFG_BENCHMARK_LABEL` | non-empty string | `default` | Label written into benchmark logs. |
| `OMFG_HISTORY_COPY_FREEZE_HISTORY` | boolean | `false` | Freezes history updates for `history-copy` style experiments. |
| `OMFG_COPY_ORIGINAL_PRESENT_FIRST` | boolean | `false` | In `copy` paths, present the original before the generated frame. |
| `OMFG_BLEND_ORIGINAL_PRESENT_FIRST` | boolean | `false` | In blend paths, present the original before the generated frame. For the multi-blend family on `MAILBOX`, OMFG now auto-enables original-first unless you explicitly set this variable, because leaving the generated frame last produced better RE8 pacing in Deck measurements. |
| `OMFG_GENERATED_ACQUIRE_TIMEOUT_NS` | integer `u64` nanoseconds | unset | Hard override for generated-image acquire timeout. |
| `OMFG_GENERATED_ACQUIRE_TIMEOUT_INTERVAL_MULTIPLIER` | float `>= 1.0` | `4.0` | If no hard timeout is supplied, timeout is derived from recent present interval × this multiplier. |
| `OMFG_GENERATED_ACQUIRE_TIMEOUT_MIN_NS` | integer `u64` | `50000000` | Lower clamp for adaptive acquire timeout. |
| `OMFG_GENERATED_ACQUIRE_TIMEOUT_MAX_NS` | integer `u64` | `500000000` | Upper clamp for adaptive acquire timeout. |
| `OMFG_PRESENT_TIMING` | boolean | `true` | Enables present-id / present-timing instrumentation where OMFG can safely inject its own present IDs. If the game already supplies `VkPresentIdKHR`, OMFG now preserves the app's IDs and skips its own injected present-id tagging on generated/original injected presents to avoid compatibility hangs. |
| `OMFG_PRESENT_WAIT` | boolean | `false` | Enables present wait behavior where available. |
| `OMFG_PRESENT_WAIT_TIMEOUT_NS` | integer `u64` nanoseconds | `5000000000` | Present-wait timeout. |
| `OMFG_CREATE_DEVICE_DEBUG` | boolean | `false` | Extra device-creation debug logging/behavior. |
| `OMFG_CREATE_DEVICE_APPEND_TIMING_EXTENSIONS` | boolean | `false` | Forces timing-related device extension append during device creation. Usually left off for real-game compatibility. |
| `OMFG_CREATE_DEVICE_APPEND_TIMING_FEATURES` | boolean | `false` | Forces timing-related device feature append during device creation. Usually left off for real-game compatibility. |
| `OMFG_VISUAL_HOLD_MS` | integer `u32` milliseconds | `0` | Sleeps after certain presents to make visual inspection easier. |

## Blend / search-blend tuning

| Variable | Accepted values | Default | Effect |
|---|---|---:|---|
| `OMFG_BLEND_ADAPTIVE_STRENGTH` | float | `2.0` | Controls how strongly adaptive blend responds to motion/interval heuristics. |
| `OMFG_BLEND_ADAPTIVE_BIAS` | float | `0.25` | Bias term for adaptive blend weighting. |
| `OMFG_SEARCH_BLEND_RADIUS` | integer `1..4` | `1` | Search radius for search-based blend matching. Higher values search farther but cost more. |

## Reprojection tuning

| Variable | Accepted values | Default | Effect |
|---|---|---:|---|
| `OMFG_REPROJECT_SEARCH_RADIUS` | integer `1..4` | `2` | How far reprojection searches for matching pixels/patches. |
| `OMFG_REPROJECT_PATCH_RADIUS` | integer `0..2` effectively clamped with max `2` | `1` | Patch size used during reprojection matching. |
| `OMFG_REPROJECT_CONFIDENCE_SCALE` | float | `4.0` | Confidence scaling for reprojection candidate selection. |
| `OMFG_REPROJECT_DISOCCLUSION_SCALE` | float `0.0..8.0` | `1.5` | Strength of disocclusion detection. |
| `OMFG_REPROJECT_HOLE_FILL_STRENGTH` | float `0.0..1.0` | `0.75` | Strength of neighborhood hole filling in disoccluded areas. |
| `OMFG_REPROJECT_HOLE_FILL_RADIUS` | integer `0..2` effectively clamped with max `2` | `1` | Neighborhood radius used for hole fill. |
| `OMFG_REPROJECT_DISOCCLUSION_CURRENT_BIAS` | float `0.0..1.0` | `0.75` | In uncertain disoccluded regions, bias fallback toward the current frame. |
| `OMFG_REPROJECT_GRADIENT_CONFIDENCE_WEIGHT` | float `0.0..32.0` | `8.0` | Reduces confidence in flat/low-detail regions where motion is ambiguous. |
| `OMFG_REPROJECT_CHROMA_WEIGHT` | float `0.0..1.0` | `0.3` | Mix between luma-only and RGB matching. |
| `OMFG_REPROJECT_AMBIGUITY_SCALE` | float `0.0..32.0` | `6.0` | Suppresses confidence when multiple candidates are similarly plausible. |

## Optical flow tuning

| Variable | Accepted values | Default | Effect |
|---|---|---:|---|
| `OMFG_OPTICAL_FLOW_SEARCH_RADIUS` | integer `1..4` | `2` | Search radius for optical-flow matching. |
| `OMFG_OPTICAL_FLOW_PATCH_RADIUS` | integer `0..2` effectively clamped with max `2` | `1` | Patch size used for optical-flow matching. |
| `OMFG_OPTICAL_FLOW_LEVELS` | integer `1..4` | `3` | Number of pyramid levels / hierarchical passes. |
| `OMFG_OPTICAL_FLOW_CONFIDENCE_SCALE` | float `0.0..32.0` | `4.0` | Confidence scaling for optical-flow candidate selection. |
| `OMFG_OPTICAL_FLOW_MOTION_PENALTY` | float `0.0..1.0` | `0.01` | Penalizes implausibly large/unstable motion. |
| `OMFG_OPTICAL_FLOW_RADIUS` | numeric | currently docs/plans only | Mentioned in planning docs; not currently consumed in the Rust runtime. |
| `OMFG_OPTICAL_FLOW_SMOOTHNESS` | numeric | currently docs/plans only | Mentioned in planning docs; not currently consumed in the Rust runtime. |
| `OMFG_OPTICAL_FLOW_LOWRES_FACTOR` | numeric | currently docs/plans only | Mentioned in planning docs; not currently consumed in the Rust runtime. |

## Multi-FG / adaptive multi-FG tuning

| Variable | Accepted values | Default | Effect |
|---|---|---:|---|
| `OMFG_MULTI_BLEND_COUNT` | integer `>= 0` | `2` in multi modes | Number of generated frames requested between real frames for fixed non-adaptive multi modes. `0` means "armed but currently off" if swapchain headroom was reserved. |
| `OMFG_MULTI_BLEND_RESERVED_COUNT` | integer `>= 0` | falls back to `OMFG_MULTI_BLEND_COUNT` | Extra fixed multi-FG headroom to reserve at swapchain creation even if the current live count is lower. Useful for launching with count `0` and hot-switching to `1/2/3` later. |
| `OMFG_MULTI_SWAPCHAIN_MAX_GENERATED_FRAMES` | integer `>= 1` | `32` | Hard cap used when auto-expanding swapchain image count for multi-FG modes. |
| `OMFG_ADAPTIVE_MULTI_TARGET_FPS` | float `>= 0.0` | `0.0` | If `> 0`, adaptive multi modes target an output FPS instead of just using threshold rules. |
| `OMFG_ADAPTIVE_MULTI_INTERVAL_SMOOTHING_ALPHA` | float `0.0..1.0` | `0.25` | Smoothing factor for measured present interval. |
| `OMFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES` | integer `>= 0` | `1`, or `0` when target-FPS mode is enabled | Lower bound for adaptive generated-frame count. |
| `OMFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES` | integer `>= min` | `2` | Upper bound for adaptive generated-frame count. |
| `OMFG_ADAPTIVE_MULTI_INTERVAL_THRESHOLD_MS` | float | `5.0` | Threshold used by non-target-FPS adaptive multi logic. |

## Black-frame insertion

| Variable | Accepted values | Default | Effect |
|---|---|---:|---|
| `OMFG_BFI_PERIOD` | integer `>= 1` | `1` | Insert one black frame every N real-frame opportunities. |
| `OMFG_BFI_HOLD_MS` | integer `u32` milliseconds | `0` | Extra sleep/hold around BFI-generated frames for observation/testing. |

## Debug views

`OMFG_DEBUG_VIEW` canonical values:

- `off`
- `motion`
- `confidence`
- `ambiguity`
- `disocclusion`
- `hole-fill`
- `fallback`

Accepted aliases:

| Canonical value | Also accepted |
|---|---|
| `motion` | `vector`, `offset`, `reprojection-offset` |
| `confidence` | `reproject-confidence` |
| `ambiguity` | `reproject-ambiguity` |
| `disocclusion` | `reproject-disocclusion`, `occlusion` |
| `hole-fill` | `holefill`, `reproject-hole-fill` |
| `fallback` | `source`, `fallback-source` |
| `off` | unset / unknown |

Additional debug vars:

| Variable | Accepted values | Default | Effect |
|---|---|---:|---|
| `OMFG_DEBUG_VIEW` | see above | `off` | Selects debug visualization mode. Only effective on reprojection-backed and current optical-flow-backed modes. |
| `OMFG_DEBUG_VIEW_OPACITY` | numeric | docs only / future-facing | Reserved/documented in plans; not currently consumed in the Rust runtime. |
| `OMFG_DEBUG_VIEW_SCALE` | numeric | docs only / future-facing | Reserved/documented in plans; not currently consumed in the Rust runtime. |

---

# 4) Steam Deck real-game harness vars

These are used by the scripts under `scripts/`, not by the layer itself.

## Real-game launcher wrappers

| Variable | Accepted values | Default | Effect |
|---|---|---:|---|
| `OMFG_GAME_WAIT_SEC` | integer seconds | `45` in generic real-game script unless preset overrides | Total time to watch a launched game before collecting artifacts. |
| `OMFG_BEYOND_WAIT_SEC` | integer seconds | `45` in Beyond script; sweep presets may override | Same as above, but Beyond-specific. |
| `OMFG_PROCESS_POLL_INTERVAL_SEC` | integer seconds | `15` | How often the harness prints process snapshots while waiting. |
| `OMFG_WINDOW_NAME_PATTERN` | window title substring/regex for `xdotool search --name` | game title | Which window to target for optional auto-input. |
| `OMFG_AUTO_KEYS` | `xdotool key` sequence | unset | Keys to send periodically to the game window. |
| `OMFG_AUTO_KEYS_EVERY_SEC` | integer seconds | `0` | Period for auto-input. `0` disables it. |
| `OMFG_CAPTURE_DISPLAY` | non-empty string / boolean-ish convention | unset | If set, capture `xwininfo` tree and a screenshot at the end of the run. |
| `OMFG_DISABLE_LAYER` | boolean | `0` | Launch the game without OMFG injection. Useful for control runs. |
| `OMFG_CLEANUP_WAIT_SEC` | integer seconds | `20` | Time the generic real-game script waits for old matching processes to disappear before relaunching. |
| `OMFG_RESTART_STEAM_CLIENT` | boolean | `0` | Restart Steam client before launch and collect client logs. |
| `OMFG_PROTON_LOG` | non-empty string / usually `1` | unset | If set, Proton logging is enabled and copied back as an artifact. |
| `OMFG_REAL_GAME_MODES` | space-separated mode list | built-in sweep mode list | Overrides the set of modes used by `run-steamdeck-real-game-mode-sweep.sh`. |

## Deck regression / vkcube harness

| Variable | Accepted values | Default | Effect |
|---|---|---:|---|
| `OMFG_VKCUBE_COUNT` | integer | script-specific | Number of vkcube presents/frames to request. |
| `OMFG_VKCUBE_TIMEOUT_SEC` | integer seconds | script-specific | Timeout for the vkcube smoke test. |
| `OMFG_VKCUBE_ARTIFACT_SUFFIX` | string | unset | Suffix appended to artifact directory names. |
| `OMFG_VKCUBE_PRESENT_MODE` | Vulkan present-mode string understood by test harness | unset | Requested present mode for vkcube tests. |
| `OMFG_VKCUBE_EXTRA_ARGS` | string | unset | Extra CLI args forwarded into vkcube. |

## Build/deploy toggles

| Variable | Accepted values | Default | Effect |
|---|---|---:|---|
| `OMFG_BUILD_FIRST` | boolean | `1` | Real-game sweep should build before running. |
| `OMFG_DEPLOY_FIRST` | boolean | `1` | Real-game sweep should deploy to Deck before running. |

---

# 5) Benchmark / sweep / autoperf vars

These belong to the helper scripts, not the layer runtime.

## Benchmark suite

| Variable | Accepted values | Default | Effect |
|---|---|---:|---|
| `OMFG_BENCHMARK_PRESET` | `full`, `decision`, `reproject-quality`, `reproject-disocclusion`, `optflow-compare`, `optflow-quality` | `full` | Selects which benchmark case matrix to run in `scripts/run-steamdeck-benchmark-suite.sh`. |
| `OMFG_BENCHMARK_CASES` | comma-separated case labels | unset | Restricts the benchmark suite to named cases only. |
| `OMFG_BENCHMARK_RUN_ID` | string | timestamp | Run identifier used for artifact directories. |
| `OMFG_BENCHMARK_ARTIFACT_PREFIX` | string | unset | Prefix added to per-case artifact suffixes. |
| `OMFG_BENCHMARK_VKCUBE_COUNT` | integer | `120` | vkcube frame count for benchmark cases. |
| `OMFG_BENCHMARK_TIMEOUT_SEC` | integer seconds | `30` | Timeout per benchmark case. |

## Multi-count sweep

| Variable | Accepted values | Default | Effect |
|---|---|---:|---|
| `OMFG_MULTI_SWEEP_MIN_COUNT` | integer | `1` | Lower bound for `run-steamdeck-multi-count-sweep.sh`. |
| `OMFG_MULTI_SWEEP_MAX_COUNT` | integer | `20` | Upper bound for `run-steamdeck-multi-count-sweep.sh`. |
| `OMFG_MULTI_SWEEP_VKCUBE_COUNT` | integer | `30` | vkcube frame count for the multi-count sweep. |
| `OMFG_MULTI_SWEEP_TIMEOUT_SEC` | integer seconds | `25` | Timeout per count in the multi-count sweep. |
| `OMFG_MULTI_SWEEP_RUN_ID` | string | timestamp-based | Artifact run id for the multi-count sweep. |

## Autoperf

| Variable | Accepted values | Default | Effect |
|---|---|---:|---|
| `OMFG_AUTOPERF_RUN_ID` | string | timestamp | Artifact run id for `scripts/run-autoperf-loop.sh`. |
| `OMFG_AUTOPERF_REPETITIONS` | integer | `3` | How many benchmark repetitions to run before aggregating. |
| `OMFG_AUTOPERF_BENCHMARK_PRESET` | any benchmark preset | `decision` | Preset used for the repeated benchmark runs. |
| `OMFG_AUTOPERF_COMPARE_PRESET` | any compare preset accepted by `compare-benchmark-results.py` | `decision` | Comparison rubric used against the baseline. |
| `OMFG_AUTOPERF_RUN_FULL_ON_ACCEPT` | boolean | `0` | If the short comparison passes, automatically run the full benchmark suite. |
| `OMFG_AUTOPERF_BASELINE` | path | current default baseline artifact dir in script | Baseline results path used for comparison. |

---

# 6) Script-selected layer metadata vars

These are exported by `scripts/_omfg_layer_impl.sh`.

| Variable | Accepted values | Default | Effect |
|---|---|---:|---|
| `OMFG_LAYER_BUILD_SUBDIR` | internal string | based on impl | Build subdir for the chosen layer implementation. |
| `OMFG_LAYER_ENABLE_ENV` | internal env name | based on impl | Name of the enable env (`ENABLE_OMFG_RUST`, etc.). |
| `OMFG_LAYER_DISABLE_ENV` | internal env name | based on impl | Name of the disable env. |
| `OMFG_LAYER_LIB_BASENAME` | internal filename | based on impl | Shared-library basename for deployment/build scripts. |
| `OMFG_LAYER_MANIFEST_BASENAME` | internal filename | based on impl | Vulkan manifest filename for deployment/build scripts. |
| `OMFG_LAYER_REMOTE_BASE_DEFAULT` | path | based on impl | Default remote deployment directory on Deck. |
| `OMFG_LAYER_ARTIFACT_ROOT_REL` | path | based on impl | Artifact root used by helper scripts. |
| `OMFG_LAYER_SOURCE_DIR` | path | based on impl | Source directory for the chosen impl. |
| `OMFG_LAYER_BUILD_SYSTEM` | `cmake`, `cargo` | based on impl | Build system for helper scripts. |

---

# 7) Variables mentioned in docs/plans but not currently consumed by the Rust runtime

These exist in planning docs or older notes, but are not currently read by `src/lib.rs`:

- `OMFG_OPTICAL_FLOW_RADIUS`
- `OMFG_OPTICAL_FLOW_SMOOTHNESS`
- `OMFG_OPTICAL_FLOW_LOWRES_FACTOR`
- `OMFG_DEBUG_VIEW_OPACITY`
- `OMFG_DEBUG_VIEW_SCALE`
- `OMFG_HOOK`
- `OMFG_EXPORT`
- `OMFG_MVP`
- `OMFG_RUST`

If these become real runtime knobs later, this doc should be updated.

---

# 8) Quick recipes

## Safe control run: launch with wrapper but no OMFG

```bash
export OMFG_DISABLE_LAYER=1
/home/deck/omfg.sh <steam/proton command>
```

## Basic reprojection run

```bash
export OMFG_LAYER_IMPL=rust
export OMFG_LAYER_MODE=reproject-blend
export OMFG_LAYER_LOG_FILE=/home/deck/post-proc-fg-research/logs/reproject.log
/home/deck/omfg.sh <steam/proton command>
```

## Search-adaptive run with wrapper logging

```bash
export OMFG_LAYER_MODE=search-adaptive-blend
export OMFG_WRAPPER_LOG_FILE=/home/deck/post-proc-fg-research/logs/omfg-wrapper.log
/home/deck/omfg.sh <steam/proton command>
```

## Adaptive multi-FG target 120 fps

```bash
export OMFG_LAYER_MODE=adaptive-multi-blend
export OMFG_ADAPTIVE_MULTI_TARGET_FPS=120
export OMFG_ADAPTIVE_MULTI_MIN_GENERATED_FRAMES=0
export OMFG_ADAPTIVE_MULTI_MAX_GENERATED_FRAMES=2
/home/deck/omfg.sh <steam/proton command>
```

## Start multi-FG armed but visually off, then hot-switch live

```bash
export OMFG_LAYER_MODE=reproject-multi-blend
export OMFG_MULTI_BLEND_COUNT=0
export OMFG_MULTI_BLEND_RESERVED_COUNT=3
/home/deck/omfg.sh <steam/proton command>
```

With that launch shape:

- `OMFG_MULTI_BLEND_COUNT=0` → effectively native/off
- `OMFG_MULTI_BLEND_COUNT=1` → 2x FG
- `OMFG_MULTI_BLEND_COUNT=2` → 3x FG
- `OMFG_MULTI_BLEND_COUNT=3` → 4x FG
