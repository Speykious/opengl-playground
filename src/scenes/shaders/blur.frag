#version 330
precision mediump float;

uniform sampler2D screenTexture;

in vec2 v_uv;

out vec4 FragColor;

// https://www.shadertoy.com/view/ltScRG

const int LOD = 2; // gaussian done on MIPmap at scale LOD
const int sLOD = 1 << LOD; // tile size = 2^LOD

const int samples = 40;
const float sigma = float(samples) * .25;

float gaussian(vec2 i) {
    return exp(-.5 * dot(i /= sigma, i)) / (6.28 * sigma * sigma);
}

vec4 blur(sampler2D samp, vec2 coord, vec2 scale) {
    vec4 output = vec4(0);
    int s = samples / sLOD;

    for (int i = 0; i < s * s; i++) {
        vec2 d = vec2(i % s, i / s) * float(sLOD) - float(samples) / 2.;
        output += gaussian(d) * textureLod(samp, coord + scale * d, float(LOD));
    }

    return output / output.a;
}

void main() {
    FragColor = blur(screenTexture, v_uv / iResolution.xy, 1. / iChannelResolution[0].xy);
}
