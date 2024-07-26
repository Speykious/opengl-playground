#version 330 core
precision mediump float;

in vec2 v_uv;

out vec4 FragColor;

uniform sampler2D u_screen_texture;

void main()
{
    vec4 tx = texture(u_screen_texture, v_uv);
    FragColor = vec4(vec3(1.0 - tx.rgb), tx.a);
}
