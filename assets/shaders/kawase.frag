#version 330 core

uniform float u_distance;
uniform bool u_upsample;

uniform sampler2D u_tex;

in vec2 v_uv;

out vec4 FragColor;

vec4 downsample(in sampler2D tex, in vec2 uv, in vec2 halfpixel) {
    vec4 sum = texture(tex, uv) * 4.0;
    sum += texture(tex, uv - halfpixel);
    sum += texture(tex, uv + halfpixel);
    sum += texture(tex, uv + vec2(halfpixel.x, -halfpixel.y));
    sum += texture(tex, uv - vec2(halfpixel.x, -halfpixel.y));
    return sum / 8.0;
}

vec4 upsample(in sampler2D tex, in vec2 uv, in vec2 halfpixel) {
    vec4 sum = texture(tex, uv + vec2(-halfpixel.x * 2.0, 0.0));
    sum += texture(tex, uv + vec2(-halfpixel.x, halfpixel.y)) * 2.0;
    sum += texture(tex, uv + vec2(0.0, halfpixel.y * 2.0));
    sum += texture(tex, uv + vec2(halfpixel.x, halfpixel.y)) * 2.0;
    sum += texture(tex, uv + vec2(halfpixel.x * 2.0, 0.0));
    sum += texture(tex, uv + vec2(halfpixel.x, -halfpixel.y)) * 2.0;
    sum += texture(tex, uv + vec2(0.0, -halfpixel.y * 2.0));
    sum += texture(tex, uv + vec2(-halfpixel.x, -halfpixel.y)) * 2.0;
    return sum / 12.0;
}

void main() {
    if (u_upsample) {
        FragColor = upsample(u_tex, v_uv, (u_distance) / textureSize(u_tex, 0));
    } else {
        FragColor = downsample(u_tex, v_uv, (u_distance) / textureSize(u_tex, 0));
    }
}
