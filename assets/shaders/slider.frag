#version 330 core
precision mediump float;

in vec2 v_uv;

out vec4 FragColor;

void main() {
    vec3 body = vec3(0.1);
    vec3 color = vec3(1.0, 0.2, 0.2);

    float f = fwidth(v_uv.x);

    vec3 mixed = mix(
        body - v_uv.x * 0.1,
        color,
        smoothstep(0.727 - f, 0.727 + f, v_uv.x)
    );

    FragColor = vec4(mixed, 1.0);
}
