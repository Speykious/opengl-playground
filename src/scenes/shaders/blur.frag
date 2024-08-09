#version 330 core

uniform vec2 u_direction;
uniform int u_kernel_size;

uniform sampler2D u_screen_texture;

in vec2 v_uv;

out vec4 FragColor;

// uniform pdf rand [0;1[
vec4 hash43n(vec3 p) {
    p = fract(p * vec3(5.3987, 5.4421, 6.9371));
    p += dot(p.yzx, p.xyz + vec3(21.5351, 14.3137, 15.3247));
    return fract(vec4(p.x * p.y * 95.4307, p.x * p.y * 97.5901, p.x * p.z * 93.8369, p.y * p.z * 91.6931));
}

// https://pixelmager.github.io/linelight/banding.html
vec4 dither(vec4 c) {
    // color dithering
    vec4 r0f = hash43n(vec3(gl_FragCoord.xy, 7.27));
    vec4 rnd = r0f - 0.5; // symmetric rpdf
    vec4 t = step(vec4(0.5 / 255.0), c) * step(c, vec4(1.0 - 0.5 / 255.0));
    rnd += t * (r0f.yzwx - 0.5); // symmetric tpdf

    vec4 target_dither_amplitude = vec4(1.0, 1.0, 1.0, 10.0);
    vec4 max_dither_amplitude = max(vec4(1.0 / 255.0), min(c, 1.0 - c)) * 255.0;
    vec4 dither_amplitude = min(vec4(target_dither_amplitude), max_dither_amplitude);
    rnd *= dither_amplitude;

    return rnd;
}

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

    vec4 result = (texture(image, uv)) * gaussian(0.0, sigma);
    for (int i = 1; i < u_kernel_size; ++i) {
        vec2 offset = direction * float(i) / textureSize(image, 1);
        float weight = gaussian(float(i), sigma);

        result += (texture(image, uv + offset)) * weight;
        result += (texture(image, uv - offset)) * weight;
    }
    return (result);
}

void main() {
    if (u_kernel_size <= 2) {
        FragColor = texture(u_screen_texture, v_uv);
    } else {
        vec4 col = blur(u_screen_texture, u_direction, v_uv);

        // add RGB-noise to dither color
        vec4 rnd = hash43n(vec3(gl_FragCoord.xy, 0.0)); // uniform noise [0;1[
        col += (rnd.xyzw + rnd.yzwx - 1.0) / 255.0; // symmetric tpdf noise 8bit, [-1;1[
        FragColor = col;
    }
}
