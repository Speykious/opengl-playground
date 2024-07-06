#version 330
precision mediump float;

in vec2 v_size;
in vec2 v_uv;
in float v_roundness;
in float v_stroke_width;
in vec4 v_fill_color;
in vec4 v_stroke_color;

out vec4 FragColor;

// Modified based on https://iquilezles.org/articles/distfunctions2d/
// That website is very handy
float sd_rounded_box(vec2 pos, vec2 size, float radius) {
    vec2 q = abs(pos) - size * 0.5 + radius;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - radius;
}

void main() {
    vec2 pos = (v_uv - vec2(0.5)) * v_size;

    float dist = sd_rounded_box(pos, v_size, v_roundness);
    float delta = fwidth(dist);

    FragColor = mix(
        mix(
            v_stroke_color,
            v_fill_color,
            smoothstep(-v_stroke_width - delta, -v_stroke_width, dist)
        ),
        vec4(0.0),
        smoothstep(-delta, 0.0, dist)
    );
}
