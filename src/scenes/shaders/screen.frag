#version 330 core
precision mediump float;

in vec2 v_uv;

out vec4 FragColor;

uniform sampler2D screen_texture;

void main()
{
    FragColor = vec4(vec3(1.0 - texture(screen_texture, v_uv)), 1.0);
}
