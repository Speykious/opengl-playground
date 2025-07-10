#version 330
precision mediump float;

uniform mat4 u_mvp;

// last coordinate is used for distance
in vec4 position;

out float v_dist;

void main() {
    gl_Position = u_mvp * vec4(position.xyz, 1.0);
    v_dist = position.w;
}
