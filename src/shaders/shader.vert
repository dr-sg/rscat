#version 450

layout (location = 0) in vec4 vPos;
layout (location = 1) in vec4 vColour;
layout (location = 2) in float size;

layout (location = 0) out vec4 outColour;

layout(set=0, binding=0)
uniform CameraUniform {
    vec4 camera_pos;
    mat4 view_proj;
};

void main()
{
    outColour = vColour;
    gl_Position = view_proj * vPos;
    float range = distance(vPos, camera_pos);
    gl_PointSize = (size/range)*(size/range);
}