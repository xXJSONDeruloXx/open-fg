#version 450

layout(set = 0, binding = 0) uniform sampler2D u_prev_frame;
layout(set = 0, binding = 1) uniform sampler2D u_curr_frame;

layout(push_constant) uniform BlendParams {
    float alpha;
    float adaptive_strength;
    float adaptive_bias;
    float confidence_scale;
    uint search_radius;
    uint patch_radius;
    uint mode;
    uint reserved;
} u_params;

layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 out_color;

const int MAX_SEARCH_RADIUS = 4;
const int MAX_PATCH_RADIUS = 2;

float luma(vec3 color) {
    return dot(color, vec3(0.299, 0.587, 0.114));
}

float symmetric_patch_error(vec2 center_uv, ivec2 half_offset_px, int patch_radius, vec2 texel) {
    float error = 0.0;
    for (int patch_y = -MAX_PATCH_RADIUS; patch_y <= MAX_PATCH_RADIUS; ++patch_y) {
        if (abs(patch_y) > patch_radius) {
            continue;
        }
        for (int patch_x = -MAX_PATCH_RADIUS; patch_x <= MAX_PATCH_RADIUS; ++patch_x) {
            if (abs(patch_x) > patch_radius) {
                continue;
            }
            vec2 patch_offset = vec2(patch_x, patch_y) * texel;
            vec3 prev_sample = texture(u_prev_frame, center_uv + (vec2(half_offset_px) * texel) + patch_offset).rgb;
            vec3 curr_sample = texture(u_curr_frame, center_uv - (vec2(half_offset_px) * texel) + patch_offset).rgb;
            error += abs(luma(prev_sample) - luma(curr_sample));
        }
    }
    return error;
}

void main() {
    vec4 prev_color = texture(u_prev_frame, v_uv);
    vec4 curr_color = texture(u_curr_frame, v_uv);

    vec4 source_prev = prev_color;
    vec4 source_curr = curr_color;
    float blend_alpha = u_params.alpha;

    int search_radius = min(int(u_params.search_radius), MAX_SEARCH_RADIUS);
    int patch_radius = min(int(u_params.patch_radius), MAX_PATCH_RADIUS);

    if (u_params.mode == 2u || u_params.mode == 3u) {
        ivec2 size_px = textureSize(u_prev_frame, 0);
        vec2 texel = 1.0 / vec2(size_px);
        float best_error = 1e20;
        vec4 best_prev = prev_color;
        for (int oy = -MAX_SEARCH_RADIUS; oy <= MAX_SEARCH_RADIUS; ++oy) {
            if (abs(oy) > search_radius) {
                continue;
            }
            for (int ox = -MAX_SEARCH_RADIUS; ox <= MAX_SEARCH_RADIUS; ++ox) {
                if (abs(ox) > search_radius) {
                    continue;
                }
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
    } else if (u_params.mode == 4u || u_params.mode == 5u) {
        ivec2 size_px = textureSize(u_prev_frame, 0);
        vec2 texel = 1.0 / vec2(size_px);
        float zero_error = symmetric_patch_error(v_uv, ivec2(0), patch_radius, texel);
        float best_error = zero_error;
        ivec2 best_half_offset = ivec2(0);

        for (int oy = -MAX_SEARCH_RADIUS; oy <= MAX_SEARCH_RADIUS; ++oy) {
            if (abs(oy) > search_radius) {
                continue;
            }
            for (int ox = -MAX_SEARCH_RADIUS; ox <= MAX_SEARCH_RADIUS; ++ox) {
                if (abs(ox) > search_radius) {
                    continue;
                }
                float motion_penalty = 0.02 * float(ox * ox + oy * oy);
                float error = symmetric_patch_error(v_uv, ivec2(ox, oy), patch_radius, texel) + motion_penalty;
                if (error < best_error) {
                    best_error = error;
                    best_half_offset = ivec2(ox, oy);
                }
            }
        }

        vec2 half_offset_uv = vec2(best_half_offset) * texel;
        vec4 reproject_prev = texture(u_prev_frame, v_uv + half_offset_uv);
        vec4 reproject_curr = texture(u_curr_frame, v_uv - half_offset_uv);
        float confidence = clamp((zero_error - best_error) * u_params.confidence_scale, 0.0, 1.0);
        float residual = length(reproject_curr.rgb - reproject_prev.rgb);
        float disocclusion = clamp(residual * 1.5, 0.0, 1.0);
        confidence *= (1.0 - 0.5 * disocclusion);

        source_prev = mix(prev_color, reproject_prev, confidence);
        source_curr = mix(curr_color, reproject_curr, confidence);
    }

    if (u_params.mode == 1u || u_params.mode == 3u || u_params.mode == 5u) {
        float diff = length(source_curr.rgb - source_prev.rgb);
        float motion = clamp(diff * u_params.adaptive_strength, 0.0, 1.0);
        blend_alpha = clamp(mix(u_params.alpha, 1.0 - u_params.adaptive_bias, motion), 0.0, 1.0);
    }

    out_color = mix(source_prev, source_curr, blend_alpha);
}
