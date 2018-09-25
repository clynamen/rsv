extern crate winit;
//extern crate time;
extern crate vulkano;
extern crate vulkano_win;
extern crate cgmath;

use std::sync::Arc;
use std::vec;
use std::f32;
use std::time;
use std::ops::Deref;

use self::vulkano_win::VkSurfaceBuild;
use vulkano::sync::GpuFuture;
use super::shaders::*;
use primitives::*;
use vulkano::pipeline::*;
use vulkano::framebuffer::*;

// const  VERTICES : Vec<Vertex> = vec![
//             Vertex { position: ( 1.0,  1.0, -1.0) },   
//             Vertex { position: ( 1.0, -1.0, -1.0) },   
//             Vertex { position: (-1.0, -1.0, -1.0) },   
//             Vertex { position: (-1.0,  1.0, -1.0) },
//             Vertex { position: ( 1.0,  1.0,  1.0) },   
//             Vertex { position: ( 1.0, -1.0,  1.0) },   
//             Vertex { position: (-1.0, -1.0,  1.0) },   
//             Vertex { position: (-1.0,  1.0,  1.0) }
//         ];

pub struct InstanceContainer {
    instance : Arc<vulkano::instance::Instance>
}

pub struct VulkanObjects {
    instance : Arc<vulkano::instance::Instance>,
    // physical_dev : vulkano::instance::PhysicalDevice<'t>,
    // queue_family : vulkano::instance::QueueFamily<'t>,
    device_ext : vulkano::device::DeviceExtensions,
    logical_dev : Arc<vulkano::device::Device>,
    queue : Arc<vulkano::device::Queue>,
    pipeline: Arc<GraphicsPipelineAbstract + Sync + Send>,
    swapchain: Arc<vulkano::swapchain::Swapchain<winit::Window>>,
    swapchain_images: Vec<Arc<vulkano::image::SwapchainImage<winit::Window>>>,
    depth_buffer : Arc<vulkano::image::AttachmentImage<vulkano::format::D16Unorm>>,
    uniform_buffer : vulkano::buffer::cpu_pool::CpuBufferPool<simple_point_vertex::ty::Data>,
    proj : cgmath::Matrix4<f32>,
    vertex_buffer : Arc<vulkano::buffer::CpuAccessibleBuffer<[Vertex]>>,
    renderpass: Arc<RenderPassAbstract + Sync + Send>,
    framebuffers : Option<Vec<Arc<FramebufferAbstract + Sync + Send>>>,
}

pub struct WindowObjects<T> where T : Sync, T: Send {
    events_loop : winit::EventsLoop,
    surface : Arc<vulkano::swapchain::Surface<T>>
} 

pub struct Renderer {
    vk : VulkanObjects,
    win : WindowObjects<winit::Window>,
    previous_frame: Box<GpuFuture>,
    recreate_swapchain : bool,
    // dimensions : [u32, 2],
    dynamic_state: vulkano::command_buffer::DynamicState,
    rotation_start : time::Instant,
    view : cgmath::Matrix4<f32>,
    scale : cgmath::Matrix4<f32>,
}

impl  Renderer {


    pub fn default() -> Renderer {
        let extensions = vulkano_win::required_extensions();
        let instance = vulkano::instance::Instance::new(None, &extensions, None).expect("failed to create instance");
        let physical  = vulkano::instance::PhysicalDevice::enumerate(&instance)
                                .next().expect("no device available");
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


        // let vertex_buffer = vulkano::buffer::cpu_access::CpuAccessibleBuffer
        //                             ::from_iter(device.clone(), vulkano::buffer::BufferUsage::all(), 
        //                             VERTICES.iter().cloned())
        //                             .expect("failed to create buffer");

        let mut proj = cgmath::perspective(cgmath::Rad(f32::consts::FRAC_PI_2), { dimensions[0] as f32 / dimensions[1] as f32 }, 0.01, 100.0);
        let view = cgmath::Matrix4::look_at(cgmath::Point3::new(0.3, 0.3, 1.0), cgmath::Point3::new(0.0, 0.0, 0.0), cgmath::Vector3::new(0.0, -1.0, 0.0));
        let scale = cgmath::Matrix4::from_scale(0.1);

        // let uniform_buffer = vulkano::buffer::cpu_pool::CpuBufferPool::<simple_point_vertex::ty::Data>
        //                         ::new(device.clone(), vulkano::buffer::BufferUsage::all());


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

        let bufDef = vulkano::pipeline::vertex::SingleBufferDefinition::<Vertex>::new();


        let pipeline = Arc::new(vulkano::pipeline::GraphicsPipeline::start()
            .vertex_input(bufDef)
            // .vertex_input(vulkano::pipeline::vertex::SingleBufferDefinition::new())
            .vertex_shader(vs.main_entry_point(), ())
            .point_list()
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(fs.main_entry_point(), ()) .depth_stencil_simple_depth()
            .render_pass(vulkano::framebuffer::Subpass::from(renderpass.clone(), 0).unwrap())
            .build(device.clone())
                                .unwrap());

        let mut framebuffers: Option<Vec<Arc<vulkano::framebuffer::FramebufferAbstract + Sync + Send>>> = None;

        let mut recreate_swapchain = false;
        let mut depth_buffer = vulkano::image::attachment::AttachmentImage::transient(device.clone(), dimensions, vulkano::format::D16Unorm).unwrap();

        if framebuffers.is_none() {
            framebuffers = Some(images.iter().map(|image| {
                Arc::new(vulkano::framebuffer::Framebuffer::start(renderpass.clone())
                         .add(image.clone()).unwrap()
                         .add(depth_buffer.clone()).unwrap()
                         .build().unwrap()) as Arc<FramebufferAbstract + Sync + Send>
            }).collect::<Vec<_>>());
        }

        let mut previous_frame = Box::new(vulkano::sync::now(device.clone())) as Box<GpuFuture>;
        let rotation_start = time::Instant::now();

        let mut dynamic_state = vulkano::command_buffer::DynamicState {
            line_width: None,
            viewports: Some(vec![vulkano::pipeline::viewport::Viewport {
                origin: [0.0, 0.0],
                dimensions: [dimensions[0] as f32, dimensions[1] as f32],
                depth_range: 0.0 .. 1.0,
            }]),
            scissors: None,
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

        let mut proj = cgmath::perspective(cgmath::Rad(f32::consts::FRAC_PI_2), { dimensions[0] as f32 / dimensions[1] as f32 }, 0.01, 100.0);
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
            .vertex_input(vulkano::pipeline::vertex::SingleBufferDefinition::<Vertex>::new())
            .vertex_shader(vs.main_entry_point(), ())
            .point_list()
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(fs.main_entry_point(), ()) .depth_stencil_simple_depth()
            .render_pass(vulkano::framebuffer::Subpass::from(renderpass.clone(), 0).unwrap())
            .build(device.clone())
                                .unwrap());


        let mut recreate_swapchain = false;

        let mut previous_frame = Box::new(vulkano::sync::now(device.clone())) as Box<dyn GpuFuture>;
        let rotation_start = time::Instant::now();

        let mut dynamic_state = vulkano::command_buffer::DynamicState {
            line_width: None,
            viewports: Some(vec![vulkano::pipeline::viewport::Viewport {
                origin: [0.0, 0.0],
                dimensions: [dimensions[0] as f32, dimensions[1] as f32],
                depth_range: 0.0 .. 1.0,
            }]),
            scissors: None,
        };

        let vulkan_objects = VulkanObjects {
            instance: instance.clone(),
            // physical_dev: physical,
            // queue_family: queue_family,
            device_ext: device_ext,
            logical_dev: device.clone(),
            queue : queue,
            swapchain: swapchain.clone(),
            swapchain_images : images.clone(),
            pipeline: pipeline,
            proj: proj,
            depth_buffer: depth_buffer,
            uniform_buffer: uniform_buffer,
            vertex_buffer : vertex_buffer,
            renderpass: renderpass,
            framebuffers: framebuffers
        };

        let window_objects = WindowObjects {
            events_loop: events_loop,
            surface: surface,
        };

        let renderer : Renderer = Renderer {
            vk : vulkan_objects,
            win : window_objects, 
            previous_frame: previous_frame,
            recreate_swapchain : recreate_swapchain,
            dynamic_state: dynamic_state,
            rotation_start: rotation_start,
            scale: scale,
            view: view

        };

        renderer
    }

    pub fn draw(mut self :  Renderer ) ->  Renderer  {
        self.previous_frame.cleanup_finished();
        // let mut framebuffers: Option<Vec<Arc<vulkano::framebuffer::FramebufferAbstract + Sync + Send>>> = None;

        loop {
            if self.recreate_swapchain {

                let physical  = vulkano::instance::PhysicalDevice::enumerate(&self.vk.instance)
                                        .next().expect("no device available");
                let mut dimensions;
                dimensions = self.win.surface.capabilities(physical)
                    .expect("failed to get surface capabilities")
                    .current_extent.unwrap_or([1024, 768]);
                
                let (new_swapchain, new_images) = match self.vk.swapchain.recreate_with_dimension(dimensions) {
                    Ok(r) => r,
                    Err(vulkano::swapchain::SwapchainCreationError::UnsupportedDimensions) => {
                        // panic!("big error")
                        break
                    },
                    Err(err) => panic!("{:?}", err)
                };

                self.vk.swapchain = new_swapchain;
                self.vk.swapchain_images = new_images;

                let depth_buffer  =  vulkano::image::attachment::AttachmentImage::transient(self.vk.logical_dev.clone(), dimensions, vulkano::format::D16Unorm).unwrap();
                self.vk.depth_buffer = depth_buffer;

                self.vk.framebuffers = None;

                self.vk.proj = cgmath::perspective(cgmath::Rad(f32::consts::FRAC_PI_2), { dimensions[0] as f32 / dimensions[1] as f32 }, 0.01, 100.0);

                self.dynamic_state.viewports = Some(vec![vulkano::pipeline::viewport::Viewport {
                    origin: [0.0, 0.0],
                    dimensions: [dimensions[0] as f32, dimensions[1] as f32],
                    depth_range: 0.0 .. 1.0,
                }]);

                self.recreate_swapchain = false;
            }

            if self.vk.framebuffers.is_none() {
                self.vk.framebuffers = Some(self.vk.swapchain_images.iter().map(|image| {
                    Arc::new(vulkano::framebuffer::Framebuffer::start(self.vk.renderpass.clone())
                            .add(image.clone()).unwrap()
                            .add(self.vk.depth_buffer.clone()).unwrap()
                            .build().unwrap())  as Arc<FramebufferAbstract + Sync + Send>
                }).collect::<Vec<_>>());
            }

            let uniform_buffer_subbuffer = {
                let elapsed = self.rotation_start.elapsed();
                let rotation = elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 / 1_000_000_000.0;
                let rotation = cgmath::Matrix3::from_angle_y(cgmath::Rad(rotation as f32));

                let uniform_data = simple_point_vertex::ty::Data {
                    world : cgmath::Matrix4::from(rotation).into(),
                    view : (self.view * self.scale).into(),
                    proj : self.vk.proj.into(),
                };

                self.vk.uniform_buffer.next(uniform_data).unwrap()
            };

            let set = Arc::new(vulkano::descriptor::descriptor_set::PersistentDescriptorSet::start(self.vk.pipeline.clone(), 0)
                .add_buffer(uniform_buffer_subbuffer).unwrap()
                .build().unwrap()
            );

            let (image_num, acquire_future) = match vulkano::swapchain::acquire_next_image(self.vk.swapchain.clone(),
                                                                                        None) {
                Ok(r) => r,
                Err(vulkano::swapchain::AcquireError::OutOfDate) => {
                    self.recreate_swapchain = true;
                    break;
                    // panic!("out of date")
                },
                Err(err) => panic!("{:?}", err)
            };

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
                                        ::from_iter(self.vk.logical_dev.clone(), vulkano::buffer::BufferUsage::all(), VERTICES.iter().cloned())
                                        .expect("failed to create buffer");

            let primaryOneTimeSubmit  = vulkano::command_buffer::AutoCommandBufferBuilder::primary_one_time_submit( self.vk.logical_dev.clone(), 
                    self.vk.queue.family());

            if(self.vk.framebuffers.is_none()) {
                // println!("framebuffers are none");
            } else {
                // println!("framebuffers are not none");
            }

            let command_buffer = primaryOneTimeSubmit.unwrap()
                .begin_render_pass(
                    self.vk.framebuffers.as_ref().unwrap()[image_num].clone(), false,
                    vec![
                        [0.0, 0.0, 0.0, 1.0].into(),
                        1f32.into()
                    ]).unwrap()
                .draw(
                    self.vk.pipeline.clone(),
                    &self.dynamic_state,

                    // self.vk.vertex_buffer.clone(), 
                    vec![vertex_buffer],

                    set.clone(), ()).unwrap()
                .end_render_pass().unwrap()
                .build().unwrap();
            
            // let prev_frame : GpuFuture  = self.previous_frame.deref();
            let future = self.previous_frame.join(acquire_future)
                .then_execute(self.vk.queue.clone(), command_buffer).unwrap()
                .then_swapchain_present(self.vk.queue.clone(), self.vk.swapchain.clone(), image_num)
                .then_signal_fence_and_flush();

            match future {
                Ok(future) => {
                    self.previous_frame = Box::new(future);
                }
                Err(vulkano::sync::FlushError::OutOfDate) => {
                    self.recreate_swapchain = true;
                    self.previous_frame = Box::new(vulkano::sync::now(self.vk.logical_dev.clone()));
                }
                Err(e) => {
                    println!("{:?}", e);
                    self.previous_frame = Box::new(vulkano::sync::now(self.vk.logical_dev.clone()));
                }
            }

            let mut done = false;
            self.win.events_loop.poll_events(|ev| {
                match ev {
                    winit::Event::WindowEvent { event: winit::WindowEvent::CloseRequested, .. } => done = true,
                    _ => ()
                }
            });

            break;
        }
        // if done { return; }
        self
    }

}