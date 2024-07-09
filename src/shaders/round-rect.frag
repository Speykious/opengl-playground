#version 430
precision mediump float;

struct GlslRect {
    vec2 size;
    float border_radius;
    float border_width;
    uint fill_color;
    uint stroke_color;
};

layout(std430, binding = 0) readonly buffer shared_buffer
{
    GlslRect rects[];
};

in vec2 v_uv;
in flat int v_rect_id;

out vec4 FragColor;

// Modified based on https://iquilezles.org/articles/distfunctions2d/
// That website is very handy
float sd_rounded_box(vec2 pos, vec2 size, float radius) {
    vec2 q = abs(pos) - size * 0.5 + radius;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - radius;
}

void main() {
    GlslRect rect = rects[v_rect_id];
    vec2 pos = v_uv * rect.size;

    float dist = sd_rounded_box(pos, rect.size, rect.border_radius);
    float delta = fwidth(dist);

    if (dist > 0.0) {
        discard;
    }

    vec4 fill_color = unpackUnorm4x8(rect.fill_color);
    vec4 stroke_color = unpackUnorm4x8(rect.stroke_color);

    FragColor = mix(
            mix(
                fill_color,
                stroke_color,
                smoothstep(-rect.border_width - delta, -rect.border_width, dist)
            ),
            vec4(stroke_color.rgb, 0.0),
            smoothstep(-delta, 0.0, dist)
        );
}
