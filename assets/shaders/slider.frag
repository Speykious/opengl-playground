#version 330 core
precision mediump float;

in vec2 v_uv;

out vec4 FragColor;

void main() {
    FragColor = vec4(v_uv, 1.0, 1.0);
}
