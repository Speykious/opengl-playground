#version 100
precision mediump float;

uniform mat4 mvp;

attribute vec3 pos;
attribute vec4 col;

varying vec4 v_color;

void main() {
    gl_Position = mvp * vec4(pos, 1.0);
    v_color = col;
}
