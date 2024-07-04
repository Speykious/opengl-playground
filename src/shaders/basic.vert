#version 100
precision mediump float;

uniform mat4 u_mvp;

attribute vec2 position;
attribute vec2 size;
attribute vec2 uv;
attribute float roundness;
attribute float stroke_width;
attribute vec4 fill_color;
attribute float stroke_color;

varying vec2 v_size;
varying vec2 v_uv;
varying float v_roundness;
varying float v_stroke_width;
varying vec4 v_fill_color;
varying vec4 v_stroke_color;

void main() {
    gl_Position = u_mvp * vec4(position, 0.0, 1.0);

    v_size = size;
    v_uv = uv;
    v_roundness = roundness;
    v_stroke_width = stroke_width;
    v_fill_color = fill_color;
    v_stroke_color = stroke_color;
}
