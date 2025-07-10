#version 330 core
precision mediump float;

uniform vec4 u_border_color = vec4(0.92, 0.95, 0.98, 1.0);
uniform float u_border_width = 7.27;
uniform float u_radius;
uniform float u_texture_progress = 0.0;

in float v_dist;

out vec4 FragColor;

uniform sampler2D u_texture;

void main() {
    float f = fwidth(v_dist);
    float border_dist = 1.0 - u_border_width / u_radius;

    vec4 body = texture(u_texture, vec2(v_dist, u_texture_progress));

    vec4 mixed = mix(
        body,
        u_border_color,
        smoothstep(border_dist - f, border_dist + f, v_dist)
    );

    float edge_cutter = 1.0 - smoothstep(1.0 - 2.0 * f, 1.0, v_dist);

    FragColor = vec4(mixed.rgb, mixed.a * edge_cutter);
}
