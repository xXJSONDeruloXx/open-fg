# Open Multi Frame Generation (OMFG)

OMFG is a **Rust Vulkan layer** for Linux that intercepts swapchain and present flow to insert generated frames between real application presents.

## Current technical surface

- explicit Vulkan layer ABI exports
- instance / device / swapchain / present interception
- post-process-only frame generation
- no dependency on engine motion vectors, depth, or game integration
- generated-frame scheduling and extra present insertion
- shader-backed synthesis paths ranging from simple blend modes to motion-aware reprojection and optical-flow-style heuristics
- multi-frame-generation modes with adaptive frame-count control

## Current mode families

- utility / validation: `passthrough`, `clear`, `bfi`, `copy`, `history-copy`
- single generated frame: `blend`, `adaptive-blend`, `search-blend`, `search-adaptive-blend`, `reproject-blend`, `reproject-adaptive-blend`, `optflow-blend`
- multi generated frame: `multi-blend`, `adaptive-multi-blend`, `reproject-multi-blend`, `reproject-adaptive-multi-blend`, `optflow-multi-blend`, `optflow-adaptive-multi-blend`

