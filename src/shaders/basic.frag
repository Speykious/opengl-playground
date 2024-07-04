#version 100
precision mediump float;

varying vec2 v_size;
varying vec2 v_uv;
varying float v_roundness;
varying float v_stroke_width;
varying vec4 v_fill_color;
varying vec4 v_stroke_color;

// Modified based on https://iquilezles.org/articles/distfunctions2d/
// That website is very handy
float sd_rounded_box(vec2 pos, vec2 size, float radius) {
    vec2 q = abs(pos) - size * 0.5 + radius;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - radius;
}

void main() {
    vec2 pos = (v_uv - vec2(0.5)) * v_size;

    float d = sd_rounded_box(pos, v_size, v_roundness);

    if (d > 0.0)
        gl_FragColor = vec4(0.0);
    else if (d > -v_stroke_width)
        gl_FragColor = v_stroke_color;
    else
        gl_FragColor = v_fill_color;
}
