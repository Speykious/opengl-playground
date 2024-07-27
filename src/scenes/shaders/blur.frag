#version 330 core
precision mediump float;

uniform float u_kernel[256];
uniform int u_kernel_size;

uniform vec2 u_direction;
uniform vec2 u_screen_size;

in vec2 v_uv;

out vec4 FragColor;

uniform sampler2D u_screen_texture;

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

    if (u_kernel_size >= 1) {
        vec4 col = vec4(0.0, 0.0, 0.0, 0.0);

        for (int i = -u_kernel_size; i < u_kernel_size; i++) {
            col += premult(texture(u_screen_texture, v_uv + u_direction * delta * float(i))) * u_kernel[abs(i)];
        }

        FragColor = unpremult(col);
    } else {
        FragColor = texture(u_screen_texture, v_uv);
    }
}
