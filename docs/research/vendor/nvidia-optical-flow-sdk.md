# NVIDIA Optical Flow SDK / NVOFA notes

## Why it matters to OMFG

NVIDIA's Optical Flow SDK is the clearest vendor-specific path for **hardware-accelerated post-process motion estimation on Linux**.

## Key sourced facts

- NVIDIA GPUs from the **Turing** generation onward include a hardware optical-flow accelerator, **NVOFA**, that operates independently of graphics / CUDA cores.
- NVOFA is exposed through multiple APIs:
  - `CUDA` — cross-platform, Linux and Windows
  - `Vulkan` — cross-platform, Linux and Windows
  - `DirectX 11/12` — Windows
- The SDK download page for Optical Flow SDK 5.0 calls out:
  - **native Vulkan optical flow support** as new in 5.0
  - support for **Windows and Linux**
  - Turing-and-newer GPU support
  - finer **1x1 / 2x2 / 4x4** grid support, with `1x1` and `2x2` available on **Ampere and newer**
- The application note adds an important caveat:
  - the **Vulkan interface does not support Optical Flow mode on Turing GPUs**
- The programming guide's Vulkan path requires:
  - `VK_KHR_timeline_semaphore`
  - `VK_NV_optical_flow`
  - a queue family supporting `VK_QUEUE_OPTICAL_FLOW_BIT_NV`
- The application note says the Vulkan interface is **not supported on WSL**, even though the SDK packaging more broadly mentions WSL support.
  - Practical reading: the SDK family may ship on WSL, but the native Vulkan optical-flow path should not be assumed there.

## OMFG takeaways

- This is a strong **NVIDIA-only acceleration branch** for motion estimation, not a cross-vendor mainline.
- The Vulkan interface is especially interesting because it aligns better with a Vulkan-layer architecture than a CUDA-only sidecar would.
- Turing vs Ampere behavior matters. If OMFG ever adds a vendor path here, capability probing must be explicit.
- Queue-family and extension requirements mean this belongs behind a clear capability gate and fallback path.

## Sources

- https://docs.nvidia.com/video-technologies/optical-flow-sdk/nvofa-programming-guide/index.html
- https://docs.nvidia.com/video-technologies/optical-flow-sdk/nvofa-application-note/index.html
- https://developer.nvidia.com/opticalflow/download
