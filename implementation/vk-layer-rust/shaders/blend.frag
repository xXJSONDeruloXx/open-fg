#version 450

layout(set = 0, binding = 0) uniform sampler2D u_prev_frame;
layout(set = 0, binding = 1) uniform sampler2D u_curr_frame;

layout(push_constant) uniform BlendParams {
    float alpha;
    float adaptive_strength;
    float adaptive_bias;
    float confidence_scale;
    float disocclusion_scale;
    float hole_fill_strength;
    float gradient_confidence_weight;
    float chroma_weight;
    float ambiguity_scale;
    float optflow_motion_penalty;
    float disocclusion_current_bias;
    uint search_radius;
    uint patch_radius;
    uint hole_fill_radius;
    uint optflow_levels;
    uint mode;
    uint debug_view;
} u_params;

layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 out_color;

const int MAX_SEARCH_RADIUS = 4;
const int MAX_PATCH_RADIUS = 2;
const int MAX_HOLE_FILL_RADIUS = 2;
const int MAX_OPTFLOW_LEVELS = 4;

float luma(vec3 color) {
    return dot(color, vec3(0.299, 0.587, 0.114));
}

float symmetric_patch_error(vec2 center_uv, ivec2 half_offset_px, int patch_radius, vec2 texel, float chroma_weight) {
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
            // Blend between luma-only (0.0) and full RGB matching (1.0)
            float luma_diff = abs(luma(prev_sample) - luma(curr_sample));
            float rgb_diff = length(prev_sample - curr_sample);
            error += mix(luma_diff, rgb_diff, chroma_weight);
        }
    }
    return error;
}

vec4 neighborhood_temporal_fill(vec2 center_uv, vec2 half_offset_uv, float alpha, int radius, vec2 texel) {
    vec4 accum = vec4(0.0);
    float total_weight = 0.0;

    for (int oy = -MAX_HOLE_FILL_RADIUS; oy <= MAX_HOLE_FILL_RADIUS; ++oy) {
        if (abs(oy) > radius) {
            continue;
        }
        for (int ox = -MAX_HOLE_FILL_RADIUS; ox <= MAX_HOLE_FILL_RADIUS; ++ox) {
            if (abs(ox) > radius) {
                continue;
            }
            vec2 offset_uv = vec2(ox, oy) * texel;
            vec4 prev_sample = texture(u_prev_frame, center_uv + half_offset_uv + offset_uv);
            vec4 curr_sample = texture(u_curr_frame, center_uv - half_offset_uv + offset_uv);
            float pair_residual = length(curr_sample.rgb - prev_sample.rgb);
            float spatial_penalty = float(abs(ox) + abs(oy));
            float weight = 1.0 / (1.0 + pair_residual * 8.0 + spatial_penalty * 0.75);
            accum += mix(prev_sample, curr_sample, alpha) * weight;
            total_weight += weight;
        }
    }

    return accum / max(total_weight, 1e-4);
}

void coarse_to_fine_half_offset(
    vec2 center_uv,
    int search_radius,
    int patch_radius,
    int levels,
    vec2 texel,
    float chroma_weight,
    float motion_penalty_scale,
    out ivec2 best_half_offset,
    out float zero_error,
    out float best_error,
    out float second_best_error
) {
    zero_error = symmetric_patch_error(center_uv, ivec2(0), patch_radius, texel, chroma_weight);
    best_half_offset = ivec2(0);
    best_error = zero_error;
    second_best_error = 1e20;

    int clamped_levels = clamp(levels, 1, MAX_OPTFLOW_LEVELS);
    for (int level = MAX_OPTFLOW_LEVELS - 1; level >= 0; --level) {
        if (level >= clamped_levels) {
            continue;
        }

        int step_px = 1 << level;
        float level_best_error = 1e20;
        float level_second_best_error = 1e20;
        ivec2 level_best_half_offset = best_half_offset;

        for (int oy = -MAX_SEARCH_RADIUS; oy <= MAX_SEARCH_RADIUS; ++oy) {
            if (abs(oy) > search_radius) {
                continue;
            }
            for (int ox = -MAX_SEARCH_RADIUS; ox <= MAX_SEARCH_RADIUS; ++ox) {
                if (abs(ox) > search_radius) {
                    continue;
                }
                ivec2 candidate_half_offset = best_half_offset + ivec2(ox * step_px, oy * step_px);
                float motion_penalty = motion_penalty_scale * float(
                    candidate_half_offset.x * candidate_half_offset.x + candidate_half_offset.y * candidate_half_offset.y
                );
                float error = symmetric_patch_error(center_uv, candidate_half_offset, patch_radius, texel, chroma_weight) + motion_penalty;
                if (error < level_best_error) {
                    level_second_best_error = level_best_error;
                    level_best_error = error;
                    level_best_half_offset = candidate_half_offset;
                } else if (error < level_second_best_error) {
                    level_second_best_error = error;
                }
            }
        }

        best_half_offset = level_best_half_offset;
        best_error = level_best_error;
        second_best_error = level_second_best_error;
    }
}

void main() {
    vec4 prev_color = texture(u_prev_frame, v_uv);
    vec4 curr_color = texture(u_curr_frame, v_uv);

    vec4 source_prev = prev_color;
    vec4 source_curr = curr_color;
    float blend_alpha = u_params.alpha;

    int search_radius = min(int(u_params.search_radius), MAX_SEARCH_RADIUS);
    int patch_radius = min(int(u_params.patch_radius), MAX_PATCH_RADIUS);
    vec2 reproject_half_offset_uv = vec2(0.0);
    vec2 reproject_texel = vec2(0.0);
    float reproject_hole_fill_weight = 0.0;
    bool reproject_mode = false;

    vec2 debug_half_offset_px = vec2(0.0);
    float debug_confidence = 0.0;
    float debug_ambiguity = 0.0;
    float debug_disocclusion = 0.0;
    float debug_fallback_weight = 1.0;
    float debug_fallback_alpha = u_params.alpha;
    vec4 debug_reproject_prev = prev_color;
    vec4 debug_reproject_curr = curr_color;

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
    } else if (u_params.mode == 4u || u_params.mode == 5u || u_params.mode == 6u) {
        reproject_mode = true;
        ivec2 size_px = textureSize(u_prev_frame, 0);
        vec2 texel = 1.0 / vec2(size_px);
        float chroma_weight = u_params.chroma_weight;
        float zero_error = 0.0;
        float best_error = 1e20;
        float second_best_error = 1e20;
        ivec2 best_half_offset = ivec2(0);

        if (u_params.mode == 6u) {
            coarse_to_fine_half_offset(
                v_uv,
                search_radius,
                patch_radius,
                int(u_params.optflow_levels),
                texel,
                chroma_weight,
                u_params.optflow_motion_penalty,
                best_half_offset,
                zero_error,
                best_error,
                second_best_error
            );
        } else {
            zero_error = symmetric_patch_error(v_uv, ivec2(0), patch_radius, texel, chroma_weight);
            best_error = zero_error;
            for (int oy = -MAX_SEARCH_RADIUS; oy <= MAX_SEARCH_RADIUS; ++oy) {
                if (abs(oy) > search_radius) {
                    continue;
                }
                for (int ox = -MAX_SEARCH_RADIUS; ox <= MAX_SEARCH_RADIUS; ++ox) {
                    if (abs(ox) > search_radius) {
                        continue;
                    }
                    float motion_penalty = 0.02 * float(ox * ox + oy * oy);
                    float error = symmetric_patch_error(v_uv, ivec2(ox, oy), patch_radius, texel, chroma_weight) + motion_penalty;
                    if (error < best_error) {
                        second_best_error = best_error;
                        best_error = error;
                        best_half_offset = ivec2(ox, oy);
                    } else if (error < second_best_error) {
                        second_best_error = error;
                    }
                }
            }
        }

        vec2 half_offset_uv = vec2(best_half_offset) * texel;
        vec4 reproject_prev = texture(u_prev_frame, v_uv + half_offset_uv);
        vec4 reproject_curr = texture(u_curr_frame, v_uv - half_offset_uv);
        float confidence = clamp((zero_error - best_error) * u_params.confidence_scale, 0.0, 1.0);
        debug_half_offset_px = vec2(best_half_offset);

        // Gradient-weighted confidence: reduce trust in flat regions where motion
        // estimation is unreliable. Textured regions preserve full confidence.
        if (u_params.gradient_confidence_weight > 0.0) {
            float grad_x = abs(luma(texture(u_curr_frame, v_uv + vec2(texel.x, 0.0)).rgb)
                             - luma(texture(u_curr_frame, v_uv - vec2(texel.x, 0.0)).rgb));
            float grad_y = abs(luma(texture(u_curr_frame, v_uv + vec2(0.0, texel.y)).rgb)
                             - luma(texture(u_curr_frame, v_uv - vec2(0.0, texel.y)).rgb));
            float gradient_mag = sqrt(grad_x * grad_x + grad_y * grad_y);
            float gradient_factor = clamp(gradient_mag * u_params.gradient_confidence_weight, 0.0, 1.0);
            // In flat regions (low gradient), scale confidence down to 25% max
            confidence *= mix(0.25, 1.0, gradient_factor);
        }

        if (u_params.ambiguity_scale > 0.0 && second_best_error < 1e19) {
            float ambiguity_margin = max(second_best_error - best_error, 0.0);
            float ambiguity_factor = clamp(ambiguity_margin * u_params.ambiguity_scale, 0.0, 1.0);
            debug_ambiguity = 1.0 - ambiguity_factor;
            // When multiple candidates are nearly tied, suppress confidence.
            confidence *= mix(0.2, 1.0, ambiguity_factor);
        }

        float residual = length(reproject_curr.rgb - reproject_prev.rgb);
        float source_residual = length(curr_color.rgb - prev_color.rgb);
        float disocclusion = clamp(max(residual - 0.5 * source_residual, 0.0) * u_params.disocclusion_scale, 0.0, 1.0);
        confidence *= (1.0 - 0.6 * disocclusion);

        reproject_half_offset_uv = half_offset_uv;
        reproject_texel = texel;
        debug_confidence = confidence;
        debug_disocclusion = disocclusion;
        debug_fallback_weight = 1.0 - confidence;
        debug_reproject_prev = reproject_prev;
        debug_reproject_curr = reproject_curr;
        reproject_hole_fill_weight = clamp((1.0 - confidence) * disocclusion * u_params.hole_fill_strength, 0.0, 1.0);

        source_prev = mix(prev_color, reproject_prev, confidence);
        source_curr = mix(curr_color, reproject_curr, confidence);
    }

    if (u_params.mode == 1u || u_params.mode == 3u || u_params.mode == 5u) {
        float diff = length(source_curr.rgb - source_prev.rgb);
        float motion = clamp(diff * u_params.adaptive_strength, 0.0, 1.0);
        blend_alpha = clamp(mix(u_params.alpha, 1.0 - u_params.adaptive_bias, motion), 0.0, 1.0);
    }

    vec4 blended_color = mix(source_prev, source_curr, blend_alpha);
    if (reproject_mode) {
        float fallback_alpha = clamp(mix(blend_alpha, 1.0, debug_disocclusion * u_params.disocclusion_current_bias), 0.0, 1.0);
        debug_fallback_alpha = fallback_alpha;
        vec4 fallback_color = mix(prev_color, curr_color, fallback_alpha);
        vec4 reprojection_color = mix(debug_reproject_prev, debug_reproject_curr, blend_alpha);
        blended_color = mix(fallback_color, reprojection_color, debug_confidence);
    }
    if (reproject_mode && reproject_hole_fill_weight > 0.0 && u_params.hole_fill_radius > 0u) {
        int hole_fill_radius = min(int(u_params.hole_fill_radius), MAX_HOLE_FILL_RADIUS);
        vec4 hole_fill_color = neighborhood_temporal_fill(v_uv, reproject_half_offset_uv, blend_alpha, hole_fill_radius, reproject_texel);
        blended_color = mix(blended_color, hole_fill_color, reproject_hole_fill_weight);
    }

    if (u_params.debug_view == 1u) {
        float search_norm = max(float(search_radius), 1.0);
        if (u_params.mode == 6u) {
            search_norm *= float(1 << max(int(u_params.optflow_levels) - 1, 0));
        }
        vec2 motion_norm = clamp(debug_half_offset_px / search_norm, vec2(-1.0), vec2(1.0));
        float motion_mag = clamp(length(debug_half_offset_px) / (search_norm * 1.41421356), 0.0, 1.0);
        out_color = vec4(0.5 + 0.5 * motion_norm.x, 0.5 + 0.5 * motion_norm.y, motion_mag, 1.0);
        return;
    }
    if (u_params.debug_view == 2u) {
        out_color = vec4(vec3(debug_confidence), 1.0);
        return;
    }
    if (u_params.debug_view == 3u) {
        out_color = vec4(vec3(debug_ambiguity), 1.0);
        return;
    }
    if (u_params.debug_view == 4u) {
        out_color = vec4(vec3(debug_disocclusion), 1.0);
        return;
    }
    if (u_params.debug_view == 5u) {
        out_color = vec4(vec3(reproject_hole_fill_weight), 1.0);
        return;
    }
    if (u_params.debug_view == 6u) {
        out_color = vec4(debug_fallback_weight, debug_fallback_alpha, reproject_hole_fill_weight, 1.0);
        return;
    }

    out_color = blended_color;
}
