#version 330 core
precision mediump float;

uniform int u_samples;
uniform vec2 u_direction;
uniform vec2 u_screen_size;

in vec2 v_uv;

out vec4 FragColor;

uniform sampler2D u_screen_texture;

float gaussian(float i, float sigma) {
    return exp(-.5 * (abs(i) / sigma)) / (3.14 * sigma);
}

void main() {
    vec2 delta = vec2(1.0) / u_screen_size;

    vec4 col = vec4(0.0, 0.0, 0.0, 1.0);

    int hsamples = u_samples / 2;
    float sigma = float(u_samples) * .25;
    for (int i = -hsamples; i < hsamples; i++) {
        col += texture(u_screen_texture, v_uv + u_direction * delta * float(i)) * gaussian(i, sigma);
    }

    FragColor = col;
}
