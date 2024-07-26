#version 330
precision mediump float;

uniform mat4 u_mvp;

in vec2 position;

out vec2 v_uv;

const vec2[4] uvs = vec2[4](
        vec2(0.0, 0.0),
        vec2(0.0, 1.0),
        vec2(1.0, 1.0),
        vec2(1.0, 0.0)
    );

void main() {
    gl_Position = u_mvp * vec4(position, 0.0, 1.0);
    v_uv = uvs[gl_VertexID % 4];
}
