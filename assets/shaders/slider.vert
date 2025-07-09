#version 330
precision mediump float;

uniform mat4 u_mvp;

in vec3 position;
in vec2 uv;

out vec2 v_uv;

void main() {
    gl_Position = u_mvp * vec4(position, 1.0);
    v_uv = uv;
}
