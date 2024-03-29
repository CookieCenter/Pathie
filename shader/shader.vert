#version 450
#extension GL_ARB_separate_shader_objects : enable
#extension GL_EXT_scalar_block_layout : enable
//#extension GL_EXT_debug_printf : enable

layout (location = 0) in vec4 in_pos;
layout (location = 1) in vec4 in_pos_on_edge;
layout (location = 2) in vec2 in_uv;
layout (location = 3) in uint in_loc_idx;

layout (location = 0) out vec4 screen_pos;
layout (location = 1) flat out vec4 pos_on_edge;
layout (location = 2) out vec4 world_pos; // pos_on_edge + local_pos
layout (location = 3) out vec2 out_uv;
layout (location = 4) flat out uint loc_idx;

#define is_leaf(node) ((node & 16777216) > 0)
#define is_subdiv(node) ((node & 33554432) > 0)
#define child_idx(node, mask) ((parent & 65535) + mask)

layout (set = 0, binding = 0) uniform Uniform {
    mat4 view_proj;
    vec4 pos;

    vec4 cam_pos;
    vec4 cam_front;
    vec4 cam_up;
    vec4 look_dir;

    vec2 res;
    vec2 mouse_delta;
    vec2 mouse_pos;

    float root_span;
    uint time;

    uint padding[2];
} uniform_buffer;

void main() {
    gl_Position = uniform_buffer.view_proj * in_pos;
    screen_pos = gl_Position;
    pos_on_edge = in_pos_on_edge;
    world_pos = in_pos;
    out_uv = in_uv;
    loc_idx = in_loc_idx;
}
