extern crate cgmath;
extern crate winit;
extern crate time;

#[macro_use]
extern crate vulkano;
#[macro_use]
extern crate vulkano_shader_derive;
extern crate vulkano_win;

use vulkano_win::VkSurfaceBuild;
use vulkano::sync::GpuFuture;

use std::sync::Arc;

extern crate rsv;

use rsv::shaders::*;
use rsv::primitives::*;
use rsv::renderer::*;

fn main() {
    let mut renderer = Renderer::default();

    loop {
        renderer = renderer.draw();
    }
}
