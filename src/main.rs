use winit::event_loop::{EventLoop,ControlFlow};
use winit::window::{WindowBuilder, Window};
use winit::dpi::LogicalSize;
use winit::event::Event;
use winit::event::WindowEvent;

use std::sync::Arc;
use std::collections::HashSet;

use vulkano_win::VkSurfaceBuild;

use vulkano::instance::{
        Instance, 
        InstanceExtensions, 
        ApplicationInfo,
        Version,
        layers_list,
        PhysicalDevice,
        PhysicalDeviceType,
    };

use vulkano::instance::debug::{
        DebugCallback,
        MessageType,
        MessageSeverity,
    };

use vulkano::device::{
        Device,
        DeviceExtensions,
        Queue,
        Features,
    };

use vulkano::swapchain::{
        Surface,
        Capabilities,
        ColorSpace,
        SupportedPresentModes,
        PresentMode,
        Swapchain,
        CompositeAlpha,
        FullscreenExclusive,
        acquire_next_image,
        AcquireError,
    };

use vulkano::format::Format;

use vulkano::image::{ImageUsage, swapchain::SwapchainImage};

use vulkano::sync::{self, SharingMode,GpuFuture};

use vulkano::pipeline::{
        GraphicsPipeline,
        vertex::BufferlessDefinition,
        vertex::BufferlessVertices,
        viewport::Viewport,
    };

use vulkano::framebuffer::{
        RenderPassAbstract,
        Subpass,
        FramebufferAbstract,
        Framebuffer,
    };

use vulkano::descriptor::PipelineLayoutAbstract;

use vulkano::command_buffer::{
        AutoCommandBuffer,
        AutoCommandBufferBuilder,
        DynamicState,
        SubpassContents,
    };

use vulkano::single_pass_renderpass;

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

const VALIDATION_LAYERS: &[&str] = &[
];

fn device_extensions() -> DeviceExtensions {
    DeviceExtensions {
        khr_swapchain: true,
        .. vulkano::device::DeviceExtensions::none()
    }
}

#[cfg(all(debug_assertions))]
const ENABLE_VALIDATION_LAYERS: bool = true;
#[cfg(not(debug_assertions))]
const ENABLE_VALIDATION_LAYERS: bool = false;

struct QueueFamilyIndices {
    graphics_family: i32,
    present_family: i32,
}

impl QueueFamilyIndices {
    fn new() -> Self {
        Self { graphics_family: -1, present_family: -1 }
    }

    fn is_complete(&self) -> bool {
        self.graphics_family >= 0 && self.present_family >= 0
    }
}

type ConcreteGraphicsPipeline = GraphicsPipeline<
                                    BufferlessDefinition, 
                                    Box<dyn PipelineLayoutAbstract + Send + Sync + 'static>, 
                                    Arc<dyn RenderPassAbstract + Send + Sync + 'static>
                                >;

struct HelloTriangleApplication {
    instance: Arc<Instance>,
    #[allow(unused)]
    debug_callback: Option<DebugCallback>,

    surface: Arc<Surface<Window>>,

    physical_device_index: usize,
    device: Arc<Device>,

    graphics_queue: Arc<Queue>,
    present_queue: Arc<Queue>,

    swap_chain: Arc<Swapchain<Window>>,
    swap_chain_images: Vec<Arc<SwapchainImage<Window>>>,

    render_pass: Arc<dyn RenderPassAbstract + Send + Sync>,
    graphics_pipeline: Arc<ConcreteGraphicsPipeline>,

    swap_chain_framebuffers: Vec<Arc<dyn FramebufferAbstract + Send + Sync>>,

    command_buffers: Vec<Arc<AutoCommandBuffer>>,

    previous_frame_end: Option<Box<dyn GpuFuture>>,
    recreate_swap_chain: bool,
}

impl HelloTriangleApplication {
    pub fn initialize(events_loop: &EventLoop<()>) -> Self {
        let instance = Self::create_instance();
        let debug_callback = Self::setup_debug_callback(&instance);

        let surface = Self::create_surface_exterior_evl(&instance, &events_loop);

        let physical_device_index = Self::pick_physical_device(&instance, &surface);
        let (device, graphics_queue, present_queue) = Self::create_logical_device(
                    &instance, &surface, physical_device_index);

        let (swap_chain, swap_chain_images) = Self::create_swap_chain(
                                                &instance,
                                                &surface,
                                                physical_device_index,
                                                &device,
                                                &graphics_queue,
                                                &present_queue,
                                                None
                                            );

        let render_pass = Self::create_render_pass(&device, swap_chain.format());
        let graphics_pipeline = Self::create_graphics_pipeline(
                                        &device, 
                                        swap_chain.dimensions(),
                                        &render_pass
                                    );

        let swap_chain_framebuffers = Self::create_framebuffers(&swap_chain_images, &render_pass);

        let previous_frame_end = Some(Self::create_sync_objects(&device));

        let mut app = Self {
            instance,
            debug_callback,

            // events_loop,
            surface,

            physical_device_index,
            device,

            graphics_queue,
            present_queue,

            swap_chain,
            swap_chain_images,

            render_pass,
            graphics_pipeline,

            swap_chain_framebuffers,

            command_buffers: vec![],

            previous_frame_end,
            recreate_swap_chain: false,
        };

        app.create_command_buffers();
        app
    }

    fn create_surface_exterior_evl(instance: &Arc<Instance>, events_loop: &EventLoop<()>) -> Arc<Surface<Window>> {
        let surface = WindowBuilder::new()
                        .with_title("Vulkan")
                        .with_inner_size(LogicalSize::new(f64::from(WIDTH), f64::from(HEIGHT)))
                        .build_vk_surface(events_loop, instance.clone())
                        .expect("failed to create window surface!");

        surface
    }

    fn create_instance() -> Arc<Instance> {

        if ENABLE_VALIDATION_LAYERS && !Self::check_validation_layer_support() {
            println!("Validation layers requested, but not available");
        }

        let supported_extensions = InstanceExtensions::supported_by_core()
                .expect("failed to retrieve supported extensions");
        println!("Supported extensions: {:?}", supported_extensions);

        let app_info = ApplicationInfo {
            application_name: Some("Hello Triangle".into()),
            application_version: Some(Version {major: 1, minor: 0, patch: 0}),
            engine_name: Some("No Engine".into()),
            engine_version: Some(Version {major: 1, minor: 0, patch: 0}),
        };

        let required_extensions = Self::get_required_extensions();

        if ENABLE_VALIDATION_LAYERS && Self::check_validation_layer_support() {
            Instance::new(Some(&app_info), &required_extensions, VALIDATION_LAYERS.iter().cloned())
                .expect("failed to create vulkan instance!")
        } else {
            Instance::new(Some(&app_info), &required_extensions, None)
                .expect("failed to create vulkan instance")
        }

    }

    fn check_validation_layer_support() -> bool {
        let layers: Vec<_> = layers_list().unwrap().map(|l| l.name().to_owned()).collect();
        VALIDATION_LAYERS.iter()
            .all(|layer_name| layers.contains(&layer_name.to_string()))
    }

    fn get_required_extensions() -> InstanceExtensions {
        let mut extensions = vulkano_win::required_extensions();
        if ENABLE_VALIDATION_LAYERS {
            extensions.ext_debug_utils = true;
        }

        extensions
    }

    fn setup_debug_callback(instance: &Arc<Instance>) -> Option<DebugCallback> {
        if !ENABLE_VALIDATION_LAYERS {
            return None
        }

        let msg_type = MessageType {
            general: true,
            validation: true,
            performance: true,
        };

        let msg_severity = MessageSeverity {
            error: true,
            warning: true,
            information: true,
            verbose: true,
        };

        DebugCallback::new(&instance, msg_severity, msg_type, |msg| {
            println!("validation layer: {:?}", msg.description);
        }).ok()
    }

    fn pick_physical_device(instance: &Arc<Instance>, surface: &Arc<Surface<Window>>) -> usize {

        let selected = PhysicalDevice::enumerate(&instance)
            .filter(|device| Self::rate_device_suitability(surface, &device).is_some())
            .max_by_key(|device| Self::rate_device_suitability(surface, &device).unwrap())
            .expect("failed to find a suitable GPU!");

        println!("Physical device selected: {}", selected.name());

        selected.index()

    }

    fn rate_device_suitability(surface: &Arc<Surface<Window>>, device: &PhysicalDevice) -> Option<u32> {
        let mut score = 0;

        match device.ty() {
            PhysicalDeviceType::DiscreteGpu => {
                score += 1000;
            },
            _ => {}
        };

        score += device.limits().max_image_dimension_2d();

        if !Self::is_device_suitable(surface, &device) {
            None
        } else {
            Some(score)
        }

    }

    fn is_device_suitable(surface: &Arc<Surface<Window>>, device: &PhysicalDevice) -> bool {
        let indices = Self::find_queue_families(surface, device);
        let extensions_supported = Self::check_device_extension_support(device);

        let swap_chain_adequate = if extensions_supported {
            let capabilities = surface.capabilities(*device)
                                .expect("failed to get surface capabilities");
            !capabilities.supported_formats.is_empty() && 
                capabilities.present_modes.iter().next().is_some()
        } else {
            false
        };

        indices.is_complete() && extensions_supported && swap_chain_adequate
    }

    fn check_device_extension_support(device: &PhysicalDevice) -> bool {
        let available_extensions = DeviceExtensions::supported_by_device(*device);
        let device_extensions = device_extensions();
        available_extensions.intersection(&device_extensions) == device_extensions
    }

    fn choose_swap_surface_format(available_formats: &[(Format, ColorSpace)]) -> (Format, ColorSpace) {
        *available_formats.iter()
            .find(|(format, color_space)|
                    *format == Format::B8G8R8A8Unorm && *color_space == ColorSpace::SrgbNonLinear
                ).unwrap_or_else(|| &available_formats[0])

    }

    fn choose_swap_present_mode(available_present_modes: SupportedPresentModes) -> PresentMode {
        if available_present_modes.mailbox {
            PresentMode::Mailbox
        } else if available_present_modes.immediate {
                PresentMode::Immediate
        } else {
                PresentMode::Fifo
        }
    }

    fn choose_swap_extent(capabilities: &Capabilities) -> [u32; 2] {
        if let Some(current_extent) = capabilities.current_extent {
            return current_extent
        } else {
            let mut actual_extent = [WIDTH, HEIGHT];
            actual_extent[0] = capabilities.min_image_extent[0]
                .max(capabilities.max_image_extent[0].min(actual_extent[0]));
            actual_extent[1] = capabilities.min_image_extent[1]
                .max(capabilities.max_image_extent[1].min(actual_extent[1]));
            actual_extent
        }
    }

    fn create_swap_chain(
        instance: &Arc<Instance>,
        surface: &Arc<Surface<Window>>,
        physical_device_index: usize,
        device: &Arc<Device>,
        graphics_queue: &Arc<Queue>,
        present_queue: &Arc<Queue>,
        old_swapchain: Option<Arc<Swapchain<Window>>>,
    ) -> (Arc<Swapchain<Window>>, Vec<Arc<SwapchainImage<Window>>>) {

        let physical_device = PhysicalDevice::from_index(&instance, physical_device_index).unwrap();
        let capabilities = surface.capabilities(physical_device)
                            .expect("failed to get surface capabilities");

        let surface_format = Self::choose_swap_surface_format(&capabilities.supported_formats);
        let present_mode = Self::choose_swap_present_mode(capabilities.present_modes);
        let extent = Self::choose_swap_extent(&capabilities);

        let mut image_count = capabilities.min_image_count + 1;
        if capabilities.max_image_count.is_some() && image_count > capabilities.max_image_count.unwrap() {
            image_count = capabilities.max_image_count.unwrap();
        }

        let image_usage = ImageUsage {
            color_attachment: true,
            .. ImageUsage::none()
        };

        let indices = Self::find_queue_families(&surface, &physical_device);

        let sharing: SharingMode = if indices.graphics_family != indices.present_family {
            vec![graphics_queue, present_queue].as_slice().into()
        } else {
            graphics_queue.into()
        };

        match old_swapchain {
            None => {
                let (swap_chain, images) = Swapchain::new(
                    device.clone(),
                    surface.clone(),
                    image_count,
                    surface_format.0,
                    extent,
                    1,
                    image_usage,
                    sharing,
                    capabilities.current_transform,
                    CompositeAlpha::Opaque,
                    present_mode,
                    FullscreenExclusive::Default,
                    true,
                    surface_format.1
                ).expect("failed to create swap chain!");
        
                (swap_chain, images)
            },
            Some(old) => {
                let (swap_chain, images) = Swapchain::with_old_swapchain(
                    device.clone(),
                    surface.clone(),
                    image_count,
                    surface_format.0,
                    extent,
                    1,
                    image_usage,
                    sharing,
                    capabilities.current_transform,
                    CompositeAlpha::Opaque,
                    present_mode,
                    FullscreenExclusive::Default,
                    true,
                    surface_format.1,
                    old
                ).expect("failed to create swap chain!");
        
                (swap_chain, images)
            }
        }

        
    }

    fn create_render_pass(
        device: &Arc<Device>, 
        color_format: Format
    ) -> Arc<dyn RenderPassAbstract + Send + Sync> {
        Arc::new(single_pass_renderpass!(device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: color_format,
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {}
            }
        ).unwrap())
    }

    fn create_graphics_pipeline(
        device: &Arc<Device>,
        swap_chain_extent: [u32; 2],
        render_pass: &Arc<dyn RenderPassAbstract + Send + Sync>,
    ) -> Arc<ConcreteGraphicsPipeline> {
        mod vertex_shader {
            vulkano_shaders::shader! {
                ty: "vertex",
                path: "src/shader_base.vert"
            }
        }

        mod fragment_shader {
            vulkano_shaders::shader! {
                ty: "fragment",
                path: "src/shader_base.frag"
            }
        }

        let vert_shader_module = vertex_shader::Shader::load(device.clone())
                .expect("failed to create vertex shader module!");

        let frag_shader_module = fragment_shader::Shader::load(device.clone())
                .expect("failed to create fragment shader module!");

        let dimensions = [swap_chain_extent[0] as f32, swap_chain_extent[1] as f32];
        let viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions,
            depth_range: 0.0 .. 1.0,
        };

        Arc::new(GraphicsPipeline::start()
            .vertex_input(BufferlessDefinition {})
            .vertex_shader(vert_shader_module.main_entry_point(), ())
            .triangle_list()
            .primitive_restart(false)
            .viewports(vec![viewport])
            .fragment_shader(frag_shader_module.main_entry_point(), ())
            .depth_clamp(false)
            .polygon_mode_fill()
            .line_width(1.0)
            .cull_mode_back()
            .front_face_clockwise()
            .blend_pass_through()
            .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
            .build(device.clone())
            .unwrap()
        )
    }

    fn create_framebuffers(
        swap_chain_images: &[Arc<SwapchainImage<Window>>],
        render_pass: &Arc<dyn RenderPassAbstract + Send + Sync>
    ) -> Vec<Arc<dyn FramebufferAbstract + Send + Sync>> {
        swap_chain_images.iter()
            .map(|image| {
                let fba: Arc<dyn FramebufferAbstract + Send + Sync> = Arc::new(Framebuffer::start(render_pass.clone())
                    .add(image.clone()).unwrap()
                    .build().unwrap());
                fba
            }).collect::<Vec<_>>()
    }

    fn create_command_buffers(&mut self) {
        let queue_family = self.graphics_queue.family();
        self.command_buffers = self.swap_chain_framebuffers.iter()
            .map(|framebuffer| {
                let vertices = BufferlessVertices { vertices: 3, instances: 1};

                let mut builder = AutoCommandBufferBuilder::primary_simultaneous_use(self.device.clone(), queue_family)
                         .expect("failed to initialise command buffer builder");
                builder.begin_render_pass(framebuffer.clone(), SubpassContents::Inline, vec![[0.0, 0.0, 0.0, 1.0].into()])
                         .expect("failed to begin render pass")
                         .draw(self.graphics_pipeline.clone(), &DynamicState::none(), vertices, (), ())
                         .expect("failed to draw")
                         .end_render_pass()
                         .expect("failed to end render pass");

                let command_buffer = builder.build().expect("failed to build render pass");
                Arc::new(command_buffer)
            }).collect()
    }

    fn create_sync_objects(device: &Arc<Device>) -> Box<dyn GpuFuture> {
        Box::new(sync::now(device.clone())) as Box<dyn GpuFuture>
    }

    fn find_queue_families(surface: &Arc<Surface<Window>>, device: &PhysicalDevice) -> QueueFamilyIndices {
        let mut indices = QueueFamilyIndices::new();

        for (i, queue_family) in device.queue_families().enumerate() {
            if queue_family.supports_graphics() {
                indices.graphics_family = i as i32;
            }

            if surface.is_supported(queue_family).unwrap() {
                indices.present_family = i as i32;
            }

            if indices.is_complete() {
                break;
            }
        }

        indices
    }

    fn create_logical_device(
        instance: &Arc<Instance>,
        surface: &Arc<Surface<Window>>,
        physical_device_index: usize,
    ) -> (Arc<Device>, Arc<Queue>, Arc<Queue>) {

        let physical_device = PhysicalDevice::from_index(&instance, physical_device_index).unwrap();
        let indices = Self::find_queue_families(&surface, &physical_device);

        let families = [indices.graphics_family, indices.present_family];
        use std::iter::FromIterator;
        let unique_queue_families: HashSet<&i32> = HashSet::from_iter(families.iter());

        let queue_priority = 1.0;
        let queue_families = unique_queue_families.iter().map(|i| {
            (physical_device.queue_families().nth(**i as usize).unwrap(), queue_priority)
        });

        let (device, mut queues) = Device::new(
                                        physical_device, 
                                        &Features::none(),
                                        &device_extensions(),
                                        queue_families    
                                    ).expect("failed to create logical device!");

        let graphics_queue = queues.next().unwrap();
        let present_queue = queues.next().unwrap_or_else(|| graphics_queue.clone());

        (device, graphics_queue, present_queue)

    }

    fn draw_frame(&mut self) {
        // let (image_index, _suboptimal, acquire_future) = acquire_next_image(self.swap_chain.clone(), None).unwrap();

        self.previous_frame_end.as_mut().unwrap().cleanup_finished();

        if self.recreate_swap_chain {
            self.recreate_swap_chain();
            self.recreate_swap_chain = false;
        }

        let (image_index, suboptimal, acquire_future) = 
            match acquire_next_image(self.swap_chain.clone(), None) {
                Ok(r) => r,
                Err(AcquireError::OutOfDate) => {
                    self.recreate_swap_chain = true;
                    return;
                },
                Err(err) => panic!("failed to acquire next image {:?}", err)
            };

        if suboptimal {
            self.recreate_swap_chain = true;
        }

        let command_buffer = self.command_buffers[image_index].clone();

        let future = self.previous_frame_end.take().unwrap()
            .join(acquire_future)
            .then_execute(self.graphics_queue.clone(), command_buffer)
            .unwrap()
            .then_swapchain_present(self.present_queue.clone(), self.swap_chain.clone(), image_index)
            .then_signal_fence_and_flush();

        match future {
            Ok(future) => {
                self.previous_frame_end = Some(Box::new(future) as Box<_>);
            }
            Err(vulkano::sync::FlushError::OutOfDate) => {
                self.recreate_swap_chain = true;
                self.previous_frame_end = Some(Box::new(vulkano::sync::now(self.device.clone())) as Box<_>);
            },
            Err(e) => {
                println!("{:?}",e);
                self.previous_frame_end = Some(Box::new(vulkano::sync::now(self.device.clone())) as Box<_>);
            }
        }
    }

    fn recreate_swap_chain(&mut self) {
        let (new_swap_chain, new_images) = Self::create_swap_chain(
                                        &self.instance, 
                                        &self.surface,
                                        self.physical_device_index,
                                        &self.device,
                                        &self.graphics_queue,
                                        &self.present_queue,
                                        Some(self.swap_chain.clone())
                                    );

        self.swap_chain = new_swap_chain;
        self.swap_chain_images = new_images;

        self.render_pass = Self::create_render_pass(&self.device, self.swap_chain.format());
        self.graphics_pipeline = Self::create_graphics_pipeline(&self.device, self.swap_chain.dimensions(),&self.render_pass);

        self.swap_chain_framebuffers = Self::create_framebuffers(&self.swap_chain_images, &self.render_pass);
        self.create_command_buffers();
    }

    fn main_loop(mut self, events_loop: EventLoop<()>) {
        events_loop.run(move |event, _, control_flow| {
            match event {
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                    *control_flow = ControlFlow::Exit;
                },
                Event::RedrawEventsCleared => {
                    self.draw_frame();
                },
                _ => ()
            }
        })
    }
}

fn main() {

    let events_loop: EventLoop<()> = EventLoop::new();

    let app = HelloTriangleApplication::initialize(&events_loop);
    app.main_loop(events_loop);

}