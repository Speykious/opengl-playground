#version 330 core
precision mediump float;

uniform int u_samples;
uniform vec2 u_direction;
uniform vec2 u_screen_size;

in vec2 v_uv;

out vec4 FragColor;

uniform sampler2D u_screen_texture;

float gaussian(float x, float sigma) {
    return 0.39894 * exp(-0.5 * x * x / (sigma * sigma)) / sigma;
}

vec4 premult(vec4 color) {
    return vec4(color.rgb * color.a, color.a);
}

vec4 unpremult(vec4 color) {
    // Prevent division by zero
    if (color.a == 0.0)
        return vec4(0.0);

    return vec4(color.rgb / color.a, color.a);
}

void main() {
    vec2 delta = vec2(1.0) / u_screen_size;

    if (u_samples >= 2) {
        vec4 col = vec4(0.0, 0.0, 0.0, 0.0);

        int hsamples = u_samples / 2;
        float sigma = float(u_samples) * 0.25;
        for (int i = -hsamples; i < hsamples; i++) {
            col += premult(texture(u_screen_texture, v_uv + u_direction * delta * float(i))) * gaussian(i, sigma);
        }

        FragColor = unpremult(col);
    } else {
        FragColor = texture(u_screen_texture, v_uv);
    }
}
