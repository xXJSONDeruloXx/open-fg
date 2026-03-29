# NVIDIA Smooth Motion on Linux

## Why it matters to OMFG

This is the strongest closed-source proof that **transparent frame generation through a Linux Vulkan present layer is real and shipping**.

## Key sourced facts

- NVIDIA Smooth Motion is described as a **driver-based AI model** that infers an additional frame between two rendered frames.
- It is positioned for games that **do not natively support DLSS Frame Generation**.
- On Linux it supports **Vulkan applications**.
- It is enabled by setting:
  - `NVPRESENT_ENABLE_SMOOTH_MOTION=1`
- Enabling it activates the implicit Vulkan layer:
  - `VK_LAYER_NV_present`
- NVIDIA says that layer **overrides the application's presentation to inject additional frames**.
- The layer presents from an **asynchronous compute queue** by default.
- For troubleshooting / compatibility:
  - `NVPRESENT_LOG_LEVEL=4` enables debug logging
  - `NVPRESENT_LOG_FILE` redirects logs from stderr to a file
  - `VK_LOADER_DEBUG=layer` helps confirm the layer is loading
  - `NVPRESENT_QUEUE_FAMILY=1` forces presentation from a graphics queue instead of the async compute queue, with some performance cost
- The gaming guide notes that **native DLSS Frame Generation and Smooth Motion should not be enabled together**.
- The Linux README positions Smooth Motion for **GeForce RTX 40-series and newer GPUs**.

## OMFG takeaways

- A Linux Vulkan-layer architecture for frame injection is not hypothetical.
- Present interception plus extra-frame insertion is proven in production on Linux.
- Queue-family choice matters enough that NVIDIA exposes a compatibility/performance knob for it.
- Logging and loader-debug ergonomics are worth treating as first-class features in OMFG too.

## Sources

- https://download.nvidia.com/XFree86/Linux-x86_64/590.48.01/README/nvpresent.html
- https://docs.nvidia.com/datacenter/tesla/driver-installation-guide/gaming.html
