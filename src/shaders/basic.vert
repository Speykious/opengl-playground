#version 330
precision mediump float;

uniform mat4 u_mvp;

in vec2 position;
in vec2 size;
in vec2 uv;
in float roundness;
in float stroke_width;
in vec4 fill_color;
in vec4 stroke_color;

out vec2 v_size;
out vec2 v_uv;
out float v_roundness;
out float v_stroke_width;
out vec4 v_fill_color;
out vec4 v_stroke_color;

void main() {
    gl_Position = u_mvp * vec4(position, 0.0, 1.0);

    v_size = size;
    v_uv = uv;
    v_roundness = roundness;
    v_stroke_width = stroke_width;
    v_fill_color = fill_color;
    v_stroke_color = stroke_color;
}
