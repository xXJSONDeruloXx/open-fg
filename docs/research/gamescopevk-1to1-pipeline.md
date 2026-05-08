# GameScopeVK 1:1 Pipeline Port — Research Notes

This branch contains a 1:1 port of the GameScopeVK (GameHub) frame interpolation
neural network pipeline into OMFG for research purposes.

## Source

The decompiled GLSL compute shaders were extracted from `libGameScopeVK.so` (ARM64, 2.1 MB)
by the [GameScopeVK-RE](https://github.com/xXJSONDeruloXx/GameScopeVK-RE) project using
spirv-cross. The original shaders are proprietary to bigeyes.com / youwo.com / GameHub.

## What was ported

Model-0 pipeline (19 compute dispatches):

| Pass | Shader | Role | Input Bindings | Output Bindings |
|------|--------|------|---------------|-----------------|
| 0 | shader_03 | Image pyramid builder | tex@32 (source frame) + UBO b0 | img@48..54 (7 r8 pyramid levels) |
| 1 | shader_05 | NN feature extract L1 | tex@32 (2× downsampled) | img@48 (rgba8 feature map) |
| 2 | shader_06 | NN feature extract L2 | tex@32 | img@48 (rgba8 feature map) |
| 3 | shader_26 | Feature channel A | tex@32 (feature input) | img@48 (rgba8) |
| 4 | shader_27 | Feature channel B | tex@32 | img@48 (rgba8) |
| 5 | shader_28 | Feature channel C | tex@32 | img@48 (rgba8) |
| 6 | shader_07 | Flow init | tex@32 | img@48,49 (rgba8 fwd+bwd init) |
| 7 | shader_09 | Coarse multi-scale OF | tex@32..37 (6 pyramid levels) | img@48,49 (coarse flow) |
| 8 | shader_08 | OF refinement first | tex@32,33 (prev+curr features) | img@48,49 (refined flow) |
| 9 | shader_10 | OF iterative (1/4 res) | tex@32,33 | img@48,49 |
| 10 | shader_11 | OF iterative (1/2 res) | tex@32,33 | img@48,49 |
| 11 | shader_12 | OF iterative (full res) | tex@32,33 | img@48,49 |
| 12 | shader_17 | Final full-res OF | tex@32,33 (high-res features) | img@48,49 |
| 13 | shader_13 | Flow pyramid expand | tex@32,33 + UBO | img@48..53 (6 pyramid levels) |
| 14 | shader_25 | Multi-scale flow aggregation | tex@32..37 + UBO | img@48 (aggregated flow) |
| 15 | shader_29 | Flow merge 2→1 | tex@32,33 | img@48 (merged flow) |
| 16 | shader_30 | Flow expand 1→2 | tex@32 | img@48,49 (expanded flow) |
| 17 | shader_14 | Flow warp + blend | tex@32..36 + UBO b0 | img@48,49,50 (warped+blend) |
| 18 | shader_04 | Frame synthesis (softmax) | tex@32..36 + UBO b0 | img@48 (interpolated frame) |

## FP16 → float32 conversion

The original shaders use `float16_t`/`f16vec4`/`f16mat4` via AMD/NV extensions
not available in standard desktop Vulkan. For this port, all FP16 types were converted
to standard `float`/`vec4`/`mat4`. The exact weight constant values are preserved — only
the precision container changed.

## Weight provenance

All weight constants in the shaders are exactly as decompiled from the GameScopeVK binary.
No weights were modified, retrained, or replaced. The weight matrices appear as:
- **shader_05**: 9-element channel mixer (single-channel 3×3 conv)
- **shader_06**: 9 × `mat4` transforms (4-channel 3×3 conv with FP16 weight matrices)
- **shader_07**: 9 × 2 output `mat4` blocks (flow init layer)
- **shader_08**: 9 × 2 `mat4` blocks per output (OF refinement)
- **shader_09**: 6-input multi-scale conv
- **shader_10-12**: 2-input × 2-output × 9 `mat4` blocks (standard OF layers)
- **shader_17**: 2-input × 2-output × 72 `mat4` blocks (largest shader, full-res OF)
- **shader_25**: 6-input aggregation weights + UBO blend weights
- **shader_26-28**: single-input 3×3 conv layers (feature channel processing)

## Architecture

### Descriptor set layout (shared by all 19 passes)

| Binding | Type | Description |
|---------|------|-------------|
| 0 | UNIFORM_BUFFER | UBO: {scale, alpha, epsilon} |
| 31 | SAMPLER | Bilinear clamp-to-edge sampler |
| 32-47 | COMBINED_IMAGE_SAMPLER | Input textures (frame, features, flow) |
| 48-54 | STORAGE_IMAGE | Output images (pyramid, features, flow, output) |

### Per-pass descriptor sets

Each of the 19 passes has its own descriptor set with different image views
bound at the input (b32..) and output (b48..) slots. This avoids the need
for update-after-bind extensions.

### Dispatch dimensions

All shaders use `local_size_x=16, local_size_y=16`.
Passes that operate at reduced resolution dispatch fewer workgroups:

| Resolution | Passes | Example dispatch (1920×1080) |
|------------|--------|------------------------------|
| Full | 0, 11-18 | 120 × 68 |
| Half | 1-6, 8, 10 | 60 × 34 |
| Quarter | 9 | 30 × 17 |
| Eighth | 7 | 15 × 9 |

### Memory barriers

A full compute→compute memory barrier (`SHADER_WRITE → SHADER_READ`) is inserted
between every pass to ensure output images from one pass are visible as input
textures to the next pass.

## OMFG mode

New mode: `gs-vk-blend` (aliases: `gs-vk`, `gsvk`)

This mode adds Vulkan compute pipeline dispatch support to OMFG alongside the existing
fragment shader pipeline. It requires:
- Compute queue support
- Storage image support
- Sufficient descriptor set slots for the pipeline's binding layouts
- `vkCreateComputePipelines`, `vkCmdDispatch`, buffer creation support

## How to compile

```bash
./scripts/compile-gsvk-shaders.sh
cargo build --locked
cargo test --locked
```

## Runtime

Set `OMFG_MODE=gs-vk-blend` to activate the GameScopeVK compute pipeline for
frame interpolation.
