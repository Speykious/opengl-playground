#version 330
precision mediump float;

in vec2 v_uv;
in vec2 v_size;
in vec4 v_fill_color;
in vec4 v_stroke_color;
in float v_border_radius;
in float v_border_width;
in float v_intensity;

out vec4 FragColor;

// Modified based on https://iquilezles.org/articles/distfunctions2d/
// That website is very handy
float sd_rounded_box(vec2 pos, vec2 size, float radius) {
    vec2 q = abs(pos) - size * 0.5 + radius;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - radius;
}

void main() {
    vec2 pos = v_uv * v_size;

    float dist = sd_rounded_box(pos, v_size, v_border_radius);
    float delta = fwidth(dist);

    if (dist > 0.0) {
        discard;
    }

    vec4 frag_color = mix(
            mix(
                v_fill_color,
                v_stroke_color,
                smoothstep(-v_border_width - delta, -v_border_width, dist)
            ),
            vec4(v_stroke_color.rgb, 0.0),
            smoothstep(-delta, 0.0, dist)
        );

    FragColor = vec4(frag_color.rgb * v_intensity, frag_color.a);
}
