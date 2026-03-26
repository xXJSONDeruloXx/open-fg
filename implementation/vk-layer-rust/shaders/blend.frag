#version 450

layout(set = 0, binding = 0) uniform sampler2D u_prev_frame;
layout(set = 0, binding = 1) uniform sampler2D u_curr_frame;

layout(push_constant) uniform BlendParams {
    float alpha;
    float adaptive_strength;
    float adaptive_bias;
    uint mode;
} u_params;

layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 out_color;

void main() {
    vec4 prev_color = texture(u_prev_frame, v_uv);
    vec4 curr_color = texture(u_curr_frame, v_uv);

    vec4 source_prev = prev_color;
    float blend_alpha = u_params.alpha;

    if (u_params.mode == 1u) {
        float diff = length(curr_color.rgb - prev_color.rgb);
        float motion = clamp(diff * u_params.adaptive_strength, 0.0, 1.0);
        blend_alpha = clamp(mix(u_params.alpha, 1.0 - u_params.adaptive_bias, motion), 0.0, 1.0);
    } else if (u_params.mode == 2u || u_params.mode == 3u) {
        ivec2 size_px = textureSize(u_prev_frame, 0);
        vec2 texel = 1.0 / vec2(size_px);
        float best_error = 1e20;
        vec4 best_prev = prev_color;
        for (int oy = -1; oy <= 1; ++oy) {
            for (int ox = -1; ox <= 1; ++ox) {
                vec2 offset_uv = v_uv + vec2(ox, oy) * texel;
                vec4 candidate = texture(u_prev_frame, offset_uv);
                float error = length(candidate.rgb - curr_color.rgb);
                if (error < best_error) {
                    best_error = error;
                    best_prev = candidate;
                }
            }
        }
        source_prev = best_prev;

        if (u_params.mode == 3u) {
            float diff = length(curr_color.rgb - best_prev.rgb);
            float motion = clamp(diff * u_params.adaptive_strength, 0.0, 1.0);
            blend_alpha = clamp(mix(u_params.alpha, 1.0 - u_params.adaptive_bias, motion), 0.0, 1.0);
        }
    }

    out_color = mix(source_prev, curr_color, blend_alpha);
}
