#version 330 core
precision mediump float;

in vec2 v_uv;

out vec4 FragColor;

void main() {
    vec4 body = vec4(0.1, 0.1, 0.1, 0.9);
    vec4 border = vec4(1.0, 0.2, 0.2, 1.0);

    float f = fwidth(v_uv.x);

    vec4 mixed = mix(
        body,
        border,
        smoothstep(0.727 - f, 0.727 + f, v_uv.x)
    );

    float a = 1.0 - smoothstep(1.0 - 2.0 * f, 1.0, v_uv.x);

    FragColor = vec4(mixed.rgb, mixed.a * a);
}
