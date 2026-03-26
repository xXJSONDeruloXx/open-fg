#version 450

layout(set = 0, binding = 0) uniform sampler2D u_prev_frame;
layout(set = 0, binding = 1) uniform sampler2D u_curr_frame;

layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 out_color;

void main() {
    vec4 prev_color = texture(u_prev_frame, v_uv);
    vec4 curr_color = texture(u_curr_frame, v_uv);
    out_color = mix(prev_color, curr_color, 0.5);
}
