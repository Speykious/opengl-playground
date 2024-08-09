#version 330 core

uniform vec2 u_direction;
uniform int u_kernel_size;

uniform sampler2D u_screen_texture;

in vec2 v_uv;

out vec4 FragColor;

float gaussian(in float x, in float sigma) {
    return 0.39894 * exp(-0.5 * x * x / (sigma * sigma)) / sigma;
}

vec4 premult(in vec4 color) {
    return vec4(color.rgb * color.a, color.a);
}

vec4 unpremult(in vec4 color) {
    // Prevent division by zero
    if (color.a == 0.0)
        return vec4(0.0);

    return vec4(color.rgb / color.a, color.a);
}

// Transparency-aware blur
vec4 blur(in sampler2D image, in vec2 direction, in vec2 uv) {
    float sigma = float(u_kernel_size - 1) / 4.0;

    vec4 result = premult(texture(image, uv)) * gaussian(0.0, sigma);
    for (int i = 1; i < u_kernel_size; ++i) {
        vec2 offset = direction * float(i) / textureSize(image, 1);
        float weight = gaussian(float(i), sigma);

        result += premult(texture(image, uv + offset)) * weight;
        result += premult(texture(image, uv - offset)) * weight;
    }
    return unpremult(result);
}

void main() {
    if (u_kernel_size <= 1) {
        FragColor = texture(u_screen_texture, v_uv);
    } else {
        FragColor = blur(u_screen_texture, u_direction, v_uv);
    }
}
