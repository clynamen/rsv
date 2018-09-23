extern crate vulkano_win;

pub mod simple_point_vertex {
    #[derive(VulkanoShader)]
    #[ty = "vertex"]
    #[src = "
#version 450

layout(location = 0) in vec3 position;


layout(set = 0, binding = 0) uniform Data {
    mat4 world;
    mat4 view;
    mat4 proj;
} uniforms;

void main() {
    mat4 worldview = uniforms.view * uniforms.world;
    gl_Position = uniforms.proj * worldview * vec4(position, 1.0);
    gl_PointSize = 1/gl_Position.z * 2.0 ;
}
"]
    #[allow(dead_code)]
    struct Dummy;
}

pub mod simple_point_fragment {
    #[derive(VulkanoShader)]
    #[ty = "fragment"]
    #[src = "
#version 450

layout(location = 0) out vec4 f_color;

void main() {
    vec3 point_color = vec3(1.0, 0.0, 0.0);
    f_color = vec4(point_color, 1.0);
}
"]
    #[allow(dead_code)]
    struct Dummy;
}
