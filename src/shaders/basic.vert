#version 430
precision mediump float;

uniform mat4 u_mvp;

in vec2 position;
in vec2 uv;

out vec2 v_uv;
out flat int v_shared_idx;

void main() {
    gl_Position = u_mvp * vec4(position, 0.0, 1.0);
    v_uv = uv;
    v_shared_idx = gl_VertexID / 4;
}
