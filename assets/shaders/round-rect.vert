#version 330
precision mediump float;

uniform mat4 u_mvp;

in vec2 position;
in vec2 size;
in int fill_color;
in int stroke_color;
in float border_radius;
in float border_width;
in float intensity;

out vec2 v_uv;
out vec2 v_size;
flat out int v_fill_color;
flat out int v_stroke_color;
out float v_border_radius;
out float v_border_width;
out float v_intensity;

const vec2[4] uvs = vec2[4](
        vec2(-0.5, -0.5),
        vec2(-0.5, 0.5),
        vec2(0.5, 0.5),
        vec2(0.5, -0.5)
    );

void main() {
    gl_Position = u_mvp * vec4(position, 0.0, 1.0);
    v_uv = uvs[gl_VertexID % 4];
    v_size = size;
    v_fill_color = fill_color;
    v_stroke_color = stroke_color;
    v_border_radius = border_radius;
    v_border_width = border_width;
    v_intensity = intensity;
}
