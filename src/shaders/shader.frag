#version 450

layout (location = 0) in vec4 frag_colour;
layout (location = 0) out vec4 pixel_colour;
layout (depth_greater) out float gl_FragDepth;

void main()
{
    vec2 center = vec2(0.5, 0.5);
    float radius = distance(center, gl_PointCoord);
    if (radius < 0.5) {
        pixel_colour = frag_colour;
        gl_FragDepth = gl_FragCoord.z;
    } else {
        pixel_colour = vec4(0, 0, 0, 1);
        gl_FragDepth = 1;
    }
}