# NVIDIA FRUC notes

## Why it matters to OMFG

FRUC is the closest NVIDIA-provided library to a ready-made **post-process frame interpolation engine on Linux**.

## Key sourced facts

- NVIDIA's **FRUC** (Frame Rate Up Conversion) library ships as part of newer Optical Flow SDK releases.
- It exposes APIs that take **two consecutive frames** and return an **interpolated frame** between them.
- Internally it uses:
  - **NVOFA** for optical flow
  - **CUDA** for the interpolation pipeline
- The FRUC docs state Linux support, including:
  - Ubuntu 18 and newer
  - Linux display driver `510.47.03` or newer
- The Linux shared library is:
  - `libNvFRUC.so`
- Documented surface formats include:
  - `ARGB`
  - `NV12`
- The API model is previous-frame / next-frame based:
  - the current call provides the next frame
  - the library caches the previous frame internally
  - the first call cannot interpolate and returns the original frame
- The docs explicitly describe requesting intermediate timestamps such as `1.25`, `1.5`, and `1.75` between two integer-timestamp frames.
- The API also exposes a quality/fallback signal:
  - `bHasFrameRepetitionOccurred`
  - when set, FRUC has returned the previous frame instead of a synthesized one because quality was not good enough

## OMFG takeaways

- FRUC is a useful conceptual reference for what a vendor-quality Linux interpolation API looks like.
- It reinforces that a practical production implementation often needs **fallback-to-repeat behavior** when confidence is low.
- It is still a vendor-specific path and not suitable as the default OMFG architecture.
- It is more relevant as:
  - a capability benchmark
  - a design reference
  - or a future optional NVIDIA backend branch

## Sources

- https://docs.nvidia.com/video-technologies/optical-flow-sdk/nvfruc-programming-guide/index.html
- https://developer.nvidia.com/opticalflow/download
