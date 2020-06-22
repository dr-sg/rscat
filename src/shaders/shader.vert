#version 450

layout (location = 0) in vec4 vertex_pos;
layout (location = 1) in vec4 vertex_colour;
layout (location = 2) in float size;

layout (location = 0) out vec4 fragment_colour;

layout(set=0, binding=0)
uniform CameraUniform {
    vec4 camera_pos;
    mat4 view_proj;
};

void main()
{
    fragment_colour = vertex_colour;
    gl_Position = view_proj * vertex_pos;
    float range = distance(vertex_pos, camera_pos);
    float screen_size = (size/range)*(size/range);
    gl_PointSize = screen_size;
}