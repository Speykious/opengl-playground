#version 330 core

uniform sampler2D u_texture;

in vec2 v_uv;

out vec4 FragColor;

// uniform pdf rand [0;1[
vec4 hash43n(vec3 p) {
    p = fract(p * vec3(5.3987, 5.4421, 6.9371));
    p += dot(p.yzx, p.xyz + vec3(21.5351, 14.3137, 15.3247));
    return fract(vec4(p.x * p.y * 95.4307, p.x * p.y * 97.5901, p.x * p.z * 93.8369, p.y * p.z * 91.6931));
}

// Color dithering
// https://pixelmager.github.io/linelight/banding.html
vec4 dither(vec4 c) {
    vec4 r0f = hash43n(vec3(gl_FragCoord.xy, 7.27));
    vec4 rnd = r0f - 0.5; // symmetric rpdf
    vec4 t = step(vec4(0.5 / 255.0), c) * step(c, vec4(1.0 - 0.5 / 255.0));
    rnd += t * (r0f.yzwx - 0.5); // symmetric tpdf

    vec4 target_dither_amplitude = vec4(1.0, 1.0, 1.0, 10.0);
    vec4 max_dither_amplitude = max(vec4(1.0 / 255.0), min(c, 1.0 - c)) * 255.0;
    vec4 dither_amplitude = min(vec4(target_dither_amplitude), max_dither_amplitude);
    rnd *= dither_amplitude;

    return c + rnd / 255.0;
}

void main() {
    FragColor = dither(texture(u_texture, v_uv));
}
