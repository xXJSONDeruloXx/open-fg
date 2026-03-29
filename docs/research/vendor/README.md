# Vendor research notes

This directory replaces the old checked-in `research/fetch/*.html` snapshots with curated Markdown notes.

## Summaries

- [NVIDIA Smooth Motion on Linux](./nvidia-smooth-motion.md)
  - Linux Smooth Motion / `VK_LAYER_NV_present` notes
- [NVIDIA Optical Flow SDK / NVOFA notes](./nvidia-optical-flow-sdk.md)
  - `VK_NV_optical_flow`, Vulkan requirements, and SDK packaging notes
- [NVIDIA FRUC notes](./nvidia-fruc.md)
  - FRUC library behavior, Linux packaging, and OMFG relevance

## Source ledger

### Smooth Motion
- https://download.nvidia.com/XFree86/Linux-x86_64/590.48.01/README/nvpresent.html
- https://docs.nvidia.com/datacenter/tesla/driver-installation-guide/gaming.html

### Optical Flow SDK / NVOFA
- https://docs.nvidia.com/video-technologies/optical-flow-sdk/nvofa-programming-guide/index.html
- https://docs.nvidia.com/video-technologies/optical-flow-sdk/nvofa-application-note/index.html
- https://developer.nvidia.com/opticalflow/download

### FRUC
- https://docs.nvidia.com/video-technologies/optical-flow-sdk/nvfruc-programming-guide/index.html

## Documentation policy

Prefer small, source-linked Markdown summaries over raw vendor HTML dumps in git.
