# GameScopeVK clean-room NN-lite shader notes

This branch adds an OMFG `nn-lite` shader family based on high-level architecture observations from `GameScopeVK-RE`, not on copied shader source or extracted weights.

## Source material reviewed

Local repos:

- `../GameScopeVK-RE`
- `../omfg`

GameScopeVK-RE files/tools reviewed:

- `README.md`
- `docs/shader_inventory.md`
- `docs/nn_architecture.md`
- `docs/pass_graph.md`
- `tools/extract_spirv.py`
- `tools/inventory_spirv.py`
- `tools/extract_nn_weights.py`
- `tools/behavioral_test_matrix.py`
- `tools/write_control.py`
- `tools/frida_runtime_trace.py`
- ELF/binary metadata from `libGameScopeVK.so`, `libxcb-dri3.so`, and `libxcb-present.so`

Important legal constraint from GameScopeVK-RE: the decompiled GLSL, SPIR-V blobs, and FP16 weight constants are proprietary analysis artifacts. OMFG must not reuse those shaders or weights.

## Reverse engineering/tooling summary

Commands run from `../GameScopeVK-RE`:

```bash
python3 tools/extract_spirv.py libGameScopeVK.so --outdir /tmp/gamescopevk_extract
python3 tools/inventory_spirv.py --shaderdir /tmp/gamescopevk_extract --outmd /tmp/gamescopevk_inventory.md
python3 tools/extract_nn_weights.py --summary
python3 tools/behavioral_test_matrix.py --list
python3 tools/write_control.py --fps 60 --enable 1 --flow-scale 0.5 --model 0 --multiplier 2
python3 tools/frida_runtime_trace.py --output /tmp/gamescopevk_trace.js
file libGameScopeVK.so libxcb-dri3.so libxcb-present.so
rabin2 -I libGameScopeVK.so
rabin2 -i libGameScopeVK.so
rabin2 -S libGameScopeVK.so
rabin2 -l libGameScopeVK.so
```

Findings used for OMFG design:

- GameScopeVK is an Android ARM64 Vulkan ICD-style shim with `vk_icd*` exports.
- The binary imports Vulkan, Android hardware buffer, socket, mmap, dlopen/dlsym, and XCB-related APIs.
- Embedded shaders use compute-style 16x16 workgroups in the documented inventory.
- The documented network is extremely small compared with public academic optical-flow models: about 10k FP16 constants for model 0 and about 21k for model 1.
- The architecture notes consistently describe 4-channel feature maps, 3x3 local context, bidirectional flow, confidence/occlusion handling, and softmax-weighted frame synthesis.
- The actual FP16 weights and decompiled code are not used here.

## OMFG implementation

New modes:

- `nn-lite-blend`
- `nn-lite-adaptive-blend`
- `nn-lite-multi-blend`
- `nn-lite-adaptive-multi-blend`

Aliases include shorter `nn-lite`, `nn-lite-adaptive`, `nn-lite-multi`, and `nn-lite-adaptive-multi` forms.

Shader modes inside `shaders/blend.frag`:

- `mode = 8`: NN-lite feature matching and softmax synthesis
- `mode = 9`: NN-lite plus the existing adaptive current-frame bias

The shader remains a single full-screen fragment pass so it fits OMFG's current post-process renderer. It adds:

- a clean-room fixed 4-channel feature encoder using luma, Sobel-like gradients, Laplacian-like local contrast, and chroma magnitude
- a tiny hand-authored MLP-like channel mixer with no extracted/trained weights
- feature-space coarse-to-fine half-offset search
- a four-candidate softmax-style bidirectional synthesis step
- reuse of existing confidence, ambiguity, disocclusion, hole-fill, and debug-view plumbing

## Runtime knobs

NN-lite has its own env knobs, with defaults chosen to stay light:

- `OMFG_NN_LITE_SEARCH_RADIUS` default: current optical-flow search radius, clamped `1..4`
- `OMFG_NN_LITE_PATCH_RADIUS` default: current optical-flow patch radius, clamped `0..2`
- `OMFG_NN_LITE_LEVELS` default: `2`, clamped `1..4`
- `OMFG_NN_LITE_CONFIDENCE_SCALE` default: `6.0`, clamped `0..32`
- `OMFG_NN_LITE_MOTION_PENALTY` default: `0.02`, clamped `0..1`

The existing debug views apply to NN-lite modes:

- `OMFG_DEBUG_VIEW=motion`
- `OMFG_DEBUG_VIEW=confidence`
- `OMFG_DEBUG_VIEW=ambiguity`
- `OMFG_DEBUG_VIEW=disocclusion`
- `OMFG_DEBUG_VIEW=hole-fill`
- `OMFG_DEBUG_VIEW=fallback`

## Verification

Run from `../omfg`:

```bash
./scripts/compile-rust-shaders.sh
cargo test --locked
cargo build --locked
```

All passed locally on this branch. Existing warnings are unchanged dead-code/style warnings from the Rust layer scaffolding.
