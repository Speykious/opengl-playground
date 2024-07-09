#version 430
precision mediump float;

uniform mat4 u_mvp;

in vec2 position;

out vec2 v_uv;
out flat int v_square_id;

const vec2[4] uvs = vec2[4](
        vec2(-0.5, -0.5),
        vec2(-0.5, 0.5),
        vec2(0.5, 0.5),
        vec2(0.5, -0.5)
    );

void main() {
    gl_Position = u_mvp * vec4(position, 0.0, 1.0);
    v_uv = uvs[gl_VertexID % 4];
    v_square_id = gl_VertexID / 4;
}
