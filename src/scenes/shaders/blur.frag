#version 330 core

uniform vec2 u_direction;

uniform sampler2D u_screen_texture;

in vec2 v_uv;

out vec4 FragColor;

const int SAMPLE_COUNT = 32;
const float sigma = float(SAMPLE_COUNT - 1) / 7.0;

float gaussian(in float x) {
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

// Transparency-aware blur
vec4 blur(in sampler2D image, vec2 direction, vec2 uv) {
    vec4 result = premult(texture(image, uv)) * gaussian(0.0);
    for (int i = 1; i < SAMPLE_COUNT / 2; ++i) {
        vec2 offset = direction * float(i) / textureSize(image, 1);
        float weight = gaussian(float(i));

        result += premult(texture(image, uv + offset)) * weight;
        result += premult(texture(image, uv - offset)) * weight;
    }
    return unpremult(result);
}

void main() {
    if (u_direction == vec2(0.0)) {
        FragColor = texture(u_screen_texture, v_uv);
    } else {
        FragColor = blur(u_screen_texture, u_direction, v_uv);
    }
}
