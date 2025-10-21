#version 330
precision mediump float;

in vec2 v_uv;
in vec2 v_size;
in vec4 v_border_radius;
in vec4 v_border_width;
flat in int v_fill_color;
flat in int v_stroke_color;
in float v_intensity;

out vec4 FragColor;

// Modified based on https://iquilezles.org/articles/distfunctions2d/
// That website is very handy
float sdRoundBox(vec2 point, vec2 size, float radius) {
    vec2 q = abs(point) - size + radius;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - radius;
}

// Modified based on https://iquilezles.org/articles/distfunctions2d/
float sdEllipse(in vec2 p, in vec2 ab) {
    if (ab.x == ab.y) return length(p) - ab.x;

    p = abs(p);
    if (p.x > p.y) {
        p = p.yx;
        ab = ab.yx;
    }
    float l = ab.y * ab.y - ab.x * ab.x;
    float m = ab.x * p.x / l;
    float m2 = m * m;
    float n = ab.y * p.y / l;
    float n2 = n * n;
    float c = (m2 + n2 - 1.0) / 3.0;
    float c3 = c * c * c;
    float q = c3 + m2 * n2 * 2.0;
    float d = c3 + m2 * n2;
    float g = m + m * n2;
    float co;
    if (d < 0.0) {
        float h = acos(q / c3) / 3.0;
        float s = cos(h);
        float t = sin(h) * sqrt(3.0);
        float rx = sqrt(-c * (s + t + 2.0) + m2);
        float ry = sqrt(-c * (s - t + 2.0) + m2);
        co = (ry + sign(l) * rx + abs(g) / (rx * ry) - m) / 2.0;
    } else {
        float h = 2.0 * m * n * sqrt(d);
        float s = sign(q + h) * pow(abs(q + h), 1.0 / 3.0);
        float u = sign(q - h) * pow(abs(q - h), 1.0 / 3.0);
        float rx = -s - u - c * 4.0 + 2.0 * m2;
        float ry = (s - u) * sqrt(3.0);
        float rm = sqrt(rx * rx + ry * ry);
        co = (ry / sqrt(rm - rx) + 2.0 * g / rm - m) / 2.0;
    }
    vec2 r = ab * vec2(co, sqrt(1.0 - co * co));
    return length(r - p) * sign(p.y - r.y);
}

// I basically brute-forced my way into math to come up with a SDF for this
// https://www.shadertoy.com/view/w3ByzG
float sdRoundBorderInner44(in vec2 point, in vec2 size, in float radius, in vec4 borders) {
    // corner ellipse computations
    float dTRBL = -100000.0;
    {
        vec2 ellipseCenter;
        vec2 ellipsePoint;
        vec2 ab;

        // top right corner
        ellipseCenter = vec2(1.0, 1.0) * (size - radius);
        ellipsePoint = point - ellipseCenter;
        ab = radius - borders.yx;
        if (ellipsePoint.x > 0.0 && ellipsePoint.y > 0.0 && min(ab.x, ab.y) > 0.0)
            dTRBL = max(dTRBL, -sdEllipse(ellipsePoint, ab));

        // top left corner
        ellipseCenter = vec2(-1.0, 1.0) * (size - radius);
        ellipsePoint = point - ellipseCenter;
        ab = radius - borders.wx;
        if (ellipsePoint.x < 0.0 && ellipsePoint.y > 0.0 && min(ab.x, ab.y) > 0.0)
            dTRBL = max(dTRBL, -sdEllipse(ellipsePoint, ab));

        // bottom right corner
        ellipseCenter = vec2(1.0, -1.0) * (size - radius);
        ellipsePoint = point - ellipseCenter;
        ab = radius - borders.yz;
        if (ellipsePoint.x > 0.0 && ellipsePoint.y < 0.0 && min(ab.x, ab.y) > 0.0)
            dTRBL = max(dTRBL, -sdEllipse(ellipsePoint, ab));

        // top left corner
        ellipseCenter = vec2(-1.0, -1.0) * (size - radius);
        ellipsePoint = point - ellipseCenter;
        ab = radius - borders.wz;
        if (ellipsePoint.x < 0.0 && ellipsePoint.y < 0.0 && min(ab.x, ab.y) > 0.0)
            dTRBL = max(dTRBL, -sdEllipse(ellipsePoint, ab));
    }

    // interior edges
    vec2 borderPosOffset  = vec2(borders.y - borders.w, borders.x - borders.z);
    vec2 borderSizeOffset = vec2(borders.y + borders.w, borders.x + borders.z);
    vec2 innerCenterPoint = point + borderPosOffset / 2.0;
    float dX = -sdRoundBox(innerCenterPoint, size - abs(borderSizeOffset) / 2.0, 0.0);

    float dInner = dX - 10000.0 > dTRBL ? dX : dTRBL;
    return dInner;
}

vec4 int_to_color(int c) {
    return vec4(
        float((c >> 24) & 0xff),
        float((c >> 16) & 0xff),
        float((c >>  8) & 0xff),
        float((c >>  0) & 0xff)
    ) / 255.0;
}

void main() {
    vec2 pos = v_uv * v_size;
    vec2 hSize = v_size / 2.0;
    vec4 radius = v_border_radius;
    vec4 borders = v_border_width;

    radius.xy = (pos.x > 0.0) ? radius.xy : radius.wz;
    radius.x = (pos.y > 0.0) ? radius.x : radius.y;
    float collapsedRadius = radius.x;

    float dOuter = sdRoundBox(pos, hSize, collapsedRadius);
    float dInner = sdRoundBorderInner44(pos, hSize, collapsedRadius, borders);
    float dist = max(dOuter, dInner);

    float delta = 0.05;

    if (dOuter > 0.0) {
        discard;
    }

    vec4 fill_color = int_to_color(v_fill_color);
    vec4 stroke_color = int_to_color(v_stroke_color);

    vec4 frag_color = mix(
            mix(
                fill_color,
                stroke_color,
                smoothstep(-delta, delta, -dInner)
            ),
            vec4(stroke_color.rgb, 0.0),
            smoothstep(-delta, delta, dOuter)
        );

    FragColor = vec4(frag_color.rgb * v_intensity, frag_color.a);
}
