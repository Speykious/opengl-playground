#version 330 core
precision mediump float;

in vec2 v_uv;

out vec4 FragColor;

uniform sampler2D u_texture;

void main() {
    FragColor = vec4(texture(u_texture, v_uv).rgb, 1.0);
}
