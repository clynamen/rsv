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
    // The start of this example is exactly the same as `triangle`. You should read the
    // `triangle` example if you haven't done so yet.

    let extensions = vulkano_win::required_extensions();
    let instance = vulkano::instance::Instance::new(None, &extensions, None).expect("failed to create instance");

    let physical = vulkano::instance::PhysicalDevice::enumerate(&instance)
                            .next().expect("no device available");
    println!("Using device: {} (type: {:?})", physical.name(), physical.ty());

    let mut events_loop = winit::EventsLoop::new();
    let surface = winit::WindowBuilder::new().build_vk_surface(&events_loop, instance.clone()).unwrap();

    let mut dimensions;

    let queue_family = physical.queue_families().find(|&q| q.supports_graphics() &&
                                                   surface.is_supported(q).unwrap_or(false))
                                                .expect("couldn't find a graphical queue family");

    let device_ext = vulkano::device::DeviceExtensions {
        khr_swapchain: true,
        .. vulkano::device::DeviceExtensions::none()
    };

    let (device, mut queues) = vulkano::device::Device::new(physical, physical.supported_features(),
                                                            &device_ext, [(queue_family, 0.5)].iter().cloned())
                               .expect("failed to create device");
    let queue = queues.next().unwrap();

    let (mut swapchain, mut images) = {
        let caps = surface.capabilities(physical).expect("failed to get surface capabilities");

        dimensions = caps.current_extent.unwrap_or([1024, 768]);

        let usage = caps.supported_usage_flags;
        let format = caps.supported_formats[0].0;
        let alpha = caps.supported_composite_alpha.iter().next().unwrap();

        vulkano::swapchain::Swapchain::new(device.clone(), surface.clone(), caps.min_image_count, format, dimensions, 1,
                                           usage, &queue, vulkano::swapchain::SurfaceTransform::Identity,
                                           alpha,
                                           vulkano::swapchain::PresentMode::Fifo, true, None).expect("failed to create swapchain")
    };


    let mut depth_buffer = vulkano::image::attachment::AttachmentImage::transient(device.clone(), dimensions, vulkano::format::D16Unorm).unwrap();

    let VERTICES = vec![
        Vertex { position: ( 1.0,  1.0, -1.0) },   
        Vertex { position: ( 1.0, -1.0, -1.0) },   
        Vertex { position: (-1.0, -1.0, -1.0) },   
        Vertex { position: (-1.0,  1.0, -1.0) },
        Vertex { position: ( 1.0,  1.0,  1.0) },   
        Vertex { position: ( 1.0, -1.0,  1.0) },   
        Vertex { position: (-1.0, -1.0,  1.0) },   
        Vertex { position: (-1.0,  1.0,  1.0) }
    ];

    let vertex_buffer = vulkano::buffer::cpu_access::CpuAccessibleBuffer
                                ::from_iter(device.clone(), vulkano::buffer::BufferUsage::all(), VERTICES.iter().cloned())
                                .expect("failed to create buffer");

    let mut proj = cgmath::perspective(cgmath::Rad(std::f32::consts::FRAC_PI_2), { dimensions[0] as f32 / dimensions[1] as f32 }, 0.01, 100.0);
    let view = cgmath::Matrix4::look_at(cgmath::Point3::new(0.3, 0.3, 1.0), cgmath::Point3::new(0.0, 0.0, 0.0), cgmath::Vector3::new(0.0, -1.0, 0.0));
    let scale = cgmath::Matrix4::from_scale(0.1);

    let uniform_buffer = vulkano::buffer::cpu_pool::CpuBufferPool::<simple_point_vertex::ty::Data>
                               ::new(device.clone(), vulkano::buffer::BufferUsage::all());

    let vs = simple_point_vertex::Shader::load(device.clone()).expect("failed to create shader module");
    let fs = simple_point_fragment::Shader::load(device.clone()).expect("failed to create shader module");

    let renderpass = Arc::new(
        single_pass_renderpass!(device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: swapchain.format(),
                    samples: 1,
                },
                depth: {
                    load: Clear,
                    store: DontCare,
                    format: vulkano::format::Format::D16Unorm,
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {depth}
            }
        ).unwrap()
    );

    let pipeline = Arc::new(vulkano::pipeline::GraphicsPipeline::start()
        .vertex_input(vulkano::pipeline::vertex::SingleBufferDefinition::new())
        .vertex_shader(vs.main_entry_point(), ())
        .point_list()
        .viewports_dynamic_scissors_irrelevant(1)
        .fragment_shader(fs.main_entry_point(), ()) .depth_stencil_simple_depth()
        .render_pass(vulkano::framebuffer::Subpass::from(renderpass.clone(), 0).unwrap())
        .build(device.clone())
                            .unwrap());
    let mut framebuffers: Option<Vec<Arc<vulkano::framebuffer::Framebuffer<_,_>>>> = None;

    let mut recreate_swapchain = false;

    let mut previous_frame = Box::new(vulkano::sync::now(device.clone())) as Box<GpuFuture>;
    let rotation_start = std::time::Instant::now();

    let mut dynamic_state = vulkano::command_buffer::DynamicState {
        line_width: None,
        viewports: Some(vec![vulkano::pipeline::viewport::Viewport {
            origin: [0.0, 0.0],
            dimensions: [dimensions[0] as f32, dimensions[1] as f32],
            depth_range: 0.0 .. 1.0,
        }]),
        scissors: None,
    };

    loop {
        previous_frame.cleanup_finished();

        if recreate_swapchain {

        dimensions = surface.capabilities(physical)
            .expect("failed to get surface capabilities")
            .current_extent.unwrap_or([1024, 768]);
            
            let (new_swapchain, new_images) = match swapchain.recreate_with_dimension(dimensions) {
                Ok(r) => r,
                Err(vulkano::swapchain::SwapchainCreationError::UnsupportedDimensions) => {
                    continue;
                },
                Err(err) => panic!("{:?}", err)
            };

            swapchain = new_swapchain;
            images = new_images;

            depth_buffer = vulkano::image::attachment::AttachmentImage::transient(device.clone(), dimensions, vulkano::format::D16Unorm).unwrap();

            framebuffers = None;

            proj = cgmath::perspective(cgmath::Rad(std::f32::consts::FRAC_PI_2), { dimensions[0] as f32 / dimensions[1] as f32 }, 0.01, 100.0);

            dynamic_state.viewports = Some(vec![vulkano::pipeline::viewport::Viewport {
                origin: [0.0, 0.0],
                dimensions: [dimensions[0] as f32, dimensions[1] as f32],
                depth_range: 0.0 .. 1.0,
            }]);

            recreate_swapchain = false;
        }

        if framebuffers.is_none() {
            framebuffers = Some(images.iter().map(|image| {
                Arc::new(vulkano::framebuffer::Framebuffer::start(renderpass.clone())
                         .add(image.clone()).unwrap()
                         .add(depth_buffer.clone()).unwrap()
                         .build().unwrap())
            }).collect::<Vec<_>>());
        }

        let uniform_buffer_subbuffer = {
            let elapsed = rotation_start.elapsed();
            let rotation = elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 / 1_000_000_000.0;
            let rotation = cgmath::Matrix3::from_angle_y(cgmath::Rad(rotation as f32));

            let uniform_data = simple_point_vertex::ty::Data {
                world : cgmath::Matrix4::from(rotation).into(),
                view : (view * scale).into(),
                proj : proj.into(),
            };

            uniform_buffer.next(uniform_data).unwrap()
        };

        let set = Arc::new(vulkano::descriptor::descriptor_set::PersistentDescriptorSet::start(pipeline.clone(), 0)
            .add_buffer(uniform_buffer_subbuffer).unwrap()
            .build().unwrap()
        );

        let (image_num, acquire_future) = match vulkano::swapchain::acquire_next_image(swapchain.clone(),
                                                                                       None) {
            Ok(r) => r,
            Err(vulkano::swapchain::AcquireError::OutOfDate) => {
                recreate_swapchain = true;
                continue;
            },
            Err(err) => panic!("{:?}", err)
        };

        let command_buffer = vulkano::command_buffer::AutoCommandBufferBuilder::primary_one_time_submit(device.clone(), queue.family()).unwrap()
            .begin_render_pass(
                framebuffers.as_ref().unwrap()[image_num].clone(), false,
                vec![
                    [0.0, 0.0, 0.0, 1.0].into(),
                    1f32.into()
                ]).unwrap()
            .draw(
                pipeline.clone(),
                &dynamic_state,
                vertex_buffer.clone(), 
                set.clone(), ()).unwrap()
            .end_render_pass().unwrap()
            .build().unwrap();
        
        let future = previous_frame.join(acquire_future)
            .then_execute(queue.clone(), command_buffer).unwrap()
            .then_swapchain_present(queue.clone(), swapchain.clone(), image_num)
            .then_signal_fence_and_flush();

        match future {
            Ok(future) => {
                previous_frame = Box::new(future) as Box<_>;
            }
            Err(vulkano::sync::FlushError::OutOfDate) => {
                recreate_swapchain = true;
                previous_frame = Box::new(vulkano::sync::now(device.clone())) as Box<_>;
            }
            Err(e) => {
                println!("{:?}", e);
                previous_frame = Box::new(vulkano::sync::now(device.clone())) as Box<_>;
            }
        }

        let mut done = false;
        events_loop.poll_events(|ev| {
            match ev {
                winit::Event::WindowEvent { event: winit::WindowEvent::CloseRequested, .. } => done = true,
                _ => ()
            }
        });
        if done { return; }
    }
}
