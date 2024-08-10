#version 330 core

uniform vec2 u_direction;
uniform int u_kernel_size;

uniform sampler2D u_tex;

in vec2 v_uv;

out vec4 FragColor;

// https://en.wikipedia.org/wiki/Scale_space_implementation#The_sampled_Gaussian_kernel
// float G(in float n, in float t) = exp(-(n * n) / (2 * t)) / sqrt(2 * PI * t)
// float G(in float n, in float t) = exp(-(n * n) / (2 * t)) / (sqrt(2 * PI) * sqrt(t))
// float G(in float n, in float σ) = exp(-(n * n) / (2 * σ * σ)) / (sqrt(2 * PI) * sqrt(σ * σ))
// float G(in float n, in float σ) = (1.0 / sqrt(2 * PI)) * exp(-(n * n) / (2 * σ * σ)) / σ
// float G(in float n, in float σ) = (1.0 / sqrt(2 * PI)) * exp(-0.5 * n * n / (σ * σ)) / σ
const float INV_SQRT_2PI = 0.398942280401;
float gaussian(in float x, in float sigma) {
    return INV_SQRT_2PI * exp(-0.5 * x * x / (sigma * sigma)) / sigma;
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
    // "A common choice is to set M to a constant C times the standard deviation of the Gaussian kernel:"
    // "M = Cσ + 1   where C is often chosen somewhere between 3 and 6."
    // -> therefore M = Cσ + 1 <=> Cσ = M - 1 <=> σ = (M - 1) / C
    float sigma = float(u_kernel_size - 1) / 4.0;

    vec4 result = premult(texture(image, uv)) * gaussian(0.0, sigma);
    for (int i = 1; i <= u_kernel_size; ++i) {
        vec2 offset = direction * float(i) / textureSize(image, 0);
        float weight = gaussian(float(i), sigma);

        result += premult(texture(image, uv + offset)) * weight;
        result += premult(texture(image, uv - offset)) * weight;
    }
    return unpremult(result);
}

void main() {
    if (u_kernel_size <= 2) {
        FragColor = texture(u_tex, v_uv);
    } else {
        FragColor = blur(u_tex, u_direction, v_uv);
    }
}
