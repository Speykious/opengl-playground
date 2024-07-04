#version 100
precision mediump float;

varying vec2 v_size;
varying vec2 v_uv;
varying float v_roundness;
varying float v_stroke_width;
varying vec4 v_fill_color;
varying vec4 v_stroke_color;

void main() {
    gl_FragColor = v_fill_color;
}
