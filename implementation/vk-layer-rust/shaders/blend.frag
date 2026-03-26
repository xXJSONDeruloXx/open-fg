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

    float blend_alpha = u_params.alpha;
    if (u_params.mode != 0u) {
        float diff = length(curr_color.rgb - prev_color.rgb);
        float motion = clamp(diff * u_params.adaptive_strength, 0.0, 1.0);
        blend_alpha = clamp(mix(u_params.alpha, 1.0 - u_params.adaptive_bias, motion), 0.0, 1.0);
    }

    out_color = mix(prev_color, curr_color, blend_alpha);
}
