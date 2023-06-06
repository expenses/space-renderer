use glam::{Vec2, Vec3};
use winit::event::*;

use std::collections::HashSet;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use structopt::StructOpt;
use wgpu::util::DeviceExt;
use winit::window::Window;

mod accessors;
mod bindless_textures;
mod buffers;
mod builtin_callbacks;
mod model_loading;
mod node_callbacks;
mod pipelines;
mod reflection;
mod texture_loading;

use std::ops::Range;

use bindless_textures::BindlessTextures;
use egui_wgpu_backend::{RenderPass, ScreenDescriptor};
use egui_winit_platform::{Platform, PlatformDescriptor};
use model_loading::load_gltf;
use model_loading::{MaterialInfo, Model};
use pipelines::{ComputePipeline, RenderPipeline, ShaderSource};
use rps_custom_backend::{ffi, rps};
use texture_loading::load_ktx2;

type Callback = unsafe extern "C" fn(context: *const rps::CmdCallbackContext);

fn box_untyped<T>(ty: T) -> *mut c_void {
    Box::into_raw(Box::new(ty)) as _
}

struct UserData {
    rotation: f32,
    device: Arc<wgpu::Device>,
    sampler: wgpu::Sampler,
    repeat_sampler: wgpu::Sampler,
    camera_rig: dolly::rig::CameraRig,
    moon: Model,
    bloom: Model,
    index_buffer: buffers::IndexBuffer,
    vertex_buffers: buffers::VertexBuffers,
    tonemap_tex: wgpu::Texture,
    cubemap: wgpu::Texture,
    filter_constants: [f32; 2],
    skybox_boost: f32,
    platform: Platform,
    window: Window,
    egui_rpass: RenderPass,
    queue: wgpu::Queue,
    bindless_textures: BindlessTextures,
    model_info_buffer: buffers::VecGpuBuffer<MaterialInfo>,
}

struct CommandBuffer {
    encoder: Option<wgpu::CommandEncoder>,
}

#[derive(StructOpt)]
struct Opts {
    filename: PathBuf,
    entry_point: String,
}

pub fn bind_node_callback(
    subprogram: rps::Subprogram,
    entry_point: &str,
    callback: rps::PfnCmdCallback,
    user_context: *mut c_void,
) -> Result<(), rps::Result> {
    let entry_point = std::ffi::CString::new(entry_point).unwrap();

    unsafe {
        rps::program_bind_node_callback(
            subprogram,
            entry_point.as_ptr(),
            &rps::CmdCallback {
                pfn_callback: callback,
                user_context,
                ..Default::default()
            },
        )
    }
}

use reflection::ReflectionSettings;

fn main() -> anyhow::Result<()> {
    unsafe {
        let opts = Opts::from_args();

        let file_stem = opts.filename.file_stem().unwrap().to_str().unwrap();

        let lib = unsafe { libloading::Library::new(&opts.filename).unwrap() };
        let entry_name = format!("rpsl_M_{}_E_{}", file_stem, opts.entry_point);
        let entry = rps::load_dynamic_library_and_get_entry_point(&lib, &entry_name).unwrap();

        let start = std::time::Instant::now();

        let event_loop = winit::event_loop::EventLoop::new();
        let window = winit::window::Window::new(&event_loop).unwrap();

        let instance = wgpu::Instance::default();

        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .unwrap();

        dbg!(&adapter.get_info());

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::PUSH_CONSTANTS
                    | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                    | wgpu::Features::SPIRV_SHADER_PASSTHROUGH
                    | wgpu::Features::TEXTURE_BINDING_ARRAY
                    | wgpu::Features::PARTIALLY_BOUND_BINDING_ARRAY,
                limits: wgpu::Limits {
                    max_push_constant_size: 64 * 2,
                    max_sampled_textures_per_shader_stage: 4096,
                    max_texture_dimension_2d: 16384,
                    max_storage_textures_per_shader_stage: 6,
                    max_compute_invocations_per_workgroup: 512,
                    max_compute_workgroup_size_x: 512,
                    ..Default::default()
                },
                ..Default::default()
            },
            None,
        ))
        .unwrap();

        let device = Arc::new(device);

        let mut vertex_buffers = buffers::VertexBuffers::new(1024, &device);
        let mut index_buffer = buffers::IndexBuffer::new(1024, &device);

        let mut textures = BindlessTextures::new();

        let mut model_info_buffer = buffers::VecGpuBuffer::new(
            1024,
            &device,
            wgpu::BufferUsages::STORAGE,
            "Material Info buffer",
        );

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        let ((moon, bloom), (tex, cubemap)) = rayon::join(
            || {
                (
                    load_gltf(
                        "assets/moon.gltf",
                        &vertex_buffers,
                        &index_buffer,
                        &device,
                        &queue,
                        &mut textures,
                        &mut model_info_buffer,
                    ),
                    load_gltf(
                        "assets/bloom_example.gltf",
                        &vertex_buffers,
                        &index_buffer,
                        &device,
                        &queue,
                        &mut textures,
                        &mut model_info_buffer,
                    ),
                )
            },
            || {
                rayon::join(
                    || {
                        load_ktx2(
                            &std::fs::read("assets/tony-mc-mapface.ktx2").unwrap(),
                            &device,
                            &queue,
                        )
                    },
                    || load_ktx2(&std::fs::read("assets/hdr.ktx2").unwrap(), &device, &queue),
                )
            },
        );

        let moon = moon.unwrap();
        let bloom = bloom.unwrap();

        let kernel = load_ktx2(&std::fs::read("kernel3.ktx2").unwrap(), &device, &queue);

        let mut keyboard_state = KeyboardState::default();
        let mut fullscreen = false;

        //let mut cursor_grab = false;

        let mut camera_rig: dolly::rig::CameraRig = dolly::rig::CameraRig::builder()
            .with(dolly::drivers::Position::new(dolly::glam::Vec3::new(
                2.0, 4.0, 1.0,
            )))
            .with(dolly::drivers::YawPitch::new().pitch_degrees(-74.0))
            .with(dolly::drivers::Smooth::new_position_rotation(0.5, 0.5))
            .build();

        let size = window.inner_size();

        let mut config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        let mut egui_rpass = RenderPass::new(&device, swapchain_format, 1);
        let mut platform = Platform::new(PlatformDescriptor {
            physical_width: size.width as u32,
            physical_height: size.height as u32,
            scale_factor: window.scale_factor(),
            font_definitions: Default::default(),
            style: Default::default(),
        });

        let user_data_raw = box_untyped(UserData {
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            repeat_sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::Repeat,
                ..Default::default()
            }),
            device: device.clone(),
            queue,
            camera_rig,
            tonemap_tex: tex,
            cubemap,
            index_buffer,
            vertex_buffers,
            moon,
            bloom,
            filter_constants: [1.25; 2],
            skybox_boost: 50.0,
            platform,
            window,
            egui_rpass,
            bindless_textures: textures,
            model_info_buffer,
            rotation: 0.0,
        });

        let device_create_info = rps::DeviceCreateInfo::default();

        let rps_device = unsafe { rps::device_create(&device_create_info) }.unwrap();

        rps_custom_backend::add_callback_runtime(
            &rps_device,
            &device_create_info,
            ffi::Callbacks {
                clear_color: Some(builtin_callbacks::clear_color),
                create_resources: Some(builtin_callbacks::create_resources),
                destroy_runtime_resource_deferred: Some(
                    builtin_callbacks::destroy_runtime_resource_deferred,
                ),
                clear_depth_stencil: Some(builtin_callbacks::clear_depth_stencil),
                ..Default::default()
            },
            user_data_raw,
        )
        .unwrap();

        let queues = &[rps::QueueFlags::all()];

        let mut x = rps::RenderGraphCreateInfo {
            schedule_info: rps::RenderGraphCreateScheduleInfo {
                queue_infos: queues.as_ptr(),
                num_queues: queues.len() as u32,
                schedule_flags: rps::ScheduleFlags::DISABLE_DEAD_CODE_ELIMINATION,
            },
            main_entry_create_info: rps::ProgramCreateInfo {
                rpsl_entry_point: entry,
                default_node_callback: rps::CmdCallback {
                    pfn_callback: Some(rps_custom_backend::callbacks::cmd_callback_warn_unused),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let graph = unsafe { rps::render_graph_create(rps_device, &x) }.unwrap();

        let subprogram = rps::render_graph_get_main_entry(graph);

        let signature = rps::rpsl_entry_get_signature_desc(entry).unwrap();

        let node_descs =
            std::slice::from_raw_parts(signature.node_descs, signature.num_node_descs as usize);

        let mut node_names: HashSet<_> = node_descs
            .iter()
            .map(|node| std::ffi::CStr::from_ptr(node.name).to_str().unwrap())
            .collect();

        let callbacks: &[(&str, Callback, *mut c_void)] = &[
            (
                "blit_srgb",
                node_callbacks::blit_srgb,
                box_untyped(RenderPipeline::new(
                    &device,
                    &ShaderSource::Hlsl("shaders/blit_srgb.hlsl"),
                    "VSMain",
                    "PSMain",
                    &[Some(swapchain_format.into())],
                    None,
                    &[],
                    false,
                )),
            ),
            (
                "draw",
                node_callbacks::draw,
                box_untyped(RenderPipeline::new(
                    &device,
                    &ShaderSource::Hlsl("shaders/moon.hlsl"),
                    "VSMain",
                    "PSMain",
                    &[Some(wgpu::TextureFormat::Rgba16Float.into())],
                    Some(wgpu::DepthStencilState {
                        format: wgpu::TextureFormat::Depth32Float,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Greater,
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    &[
                        // position, normal, uv, lightmap uv
                        wgpu::VertexBufferLayout {
                            array_stride: 3 * 4,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                        },
                        wgpu::VertexBufferLayout {
                            array_stride: 2 * 4,
                            attributes: &wgpu::vertex_attr_array![1 => Float32x2],
                            step_mode: wgpu::VertexStepMode::Vertex,
                        },
                        wgpu::VertexBufferLayout {
                            array_stride: 3 * 4,
                            attributes: &wgpu::vertex_attr_array![2 => Float32x3],
                            step_mode: wgpu::VertexStepMode::Vertex,
                        },
                        wgpu::VertexBufferLayout {
                            array_stride: 4,
                            attributes: &wgpu::vertex_attr_array![3 => Uint32],
                            step_mode: wgpu::VertexStepMode::Vertex,
                        },
                        wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<[f32; 8]>() as u64,
                            attributes: &wgpu::vertex_attr_array![4 => Float32x3, 5 => Float32, 6 => Float32x4],
                            step_mode: wgpu::VertexStepMode::Instance,
                        },
                    ],
                    true,
                )),
            ),
            (
                "downsample_initial",
                node_callbacks::downsample_initial,
                box_untyped(ComputePipeline::new(
                    &device,
                    &ShaderSource::Hlsl("shaders/downsample_initial.hlsl"),
                    "downsample_initial",
                    &Default::default(),
                    false,
                )),
            ),
            (
                "downsample",
                node_callbacks::downsample,
                box_untyped(ComputePipeline::new(
                    &device,
                    &ShaderSource::Hlsl("shaders/downsample.hlsl"),
                    "downsample",
                    &Default::default(),
                    false,
                )),
            ),
            (
                "upsample",
                node_callbacks::downsample,
                box_untyped(ComputePipeline::new(
                    &device,
                    &ShaderSource::Hlsl("shaders/upsample.hlsl"),
                    "upsample",
                    &Default::default(),
                    false,
                )),
            ),
            (
                "tonemap",
                node_callbacks::tonemap,
                box_untyped(ComputePipeline::new(
                    &device,
                    &ShaderSource::Hlsl("shaders/tonemap.hlsl"),
                    "tonemap",
                    &Default::default(),
                    false,
                )),
            ),
            (
                "compute_dof",
                node_callbacks::compute_dof,
                box_untyped(ComputePipeline::new(
                    &device,
                    &ShaderSource::Hlsl("shaders/compute_dof.hlsl"),
                    "compute_dof",
                    &ReflectionSettings {
                        override_sampled_texture_ty: Some((0, wgpu::TextureSampleType::Depth)),
                    },
                    true,
                )),
            ),
            (
                "render_skybox",
                node_callbacks::render_skybox,
                box_untyped(RenderPipeline::new(
                    &device,
                    &ShaderSource::Hlsl("shaders/skybox.hlsl"),
                    "VSMain",
                    "PSMain",
                    &[Some(wgpu::TextureFormat::Rgba16Float.into())],
                    Some(wgpu::DepthStencilState {
                        format: wgpu::TextureFormat::Depth32Float,
                        depth_write_enabled: false,
                        depth_compare: wgpu::CompareFunction::Equal,
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    &[],
                    false,
                )),
            ),
            (
                "render_ui",
                node_callbacks::render_ui,
                box_untyped(666.6665_f32),
            ),
            (
                "dof_downsample_with_coc",
                node_callbacks::dof_downsample_with_coc,
                box_untyped(ComputePipeline::new(
                    &device,
                    &ShaderSource::Hlsl("shaders/dof_downsample_with_coc.hlsl"),
                    "dof_downsample_with_coc",
                    &ReflectionSettings {
                        override_sampled_texture_ty: Some((0, wgpu::TextureSampleType::Depth)),
                    },
                    true,
                )),
            ),
            (
                "dof_x",
                node_callbacks::dof_x,
                box_untyped(ComputePipeline::new(
                    &device,
                    &ShaderSource::Hlsl("shaders/dof_x.hlsl"),
                    "dof_x",
                    &Default::default(),
                    false,
                )),
            ),
            (
                "dof_y",
                node_callbacks::dof_y,
                box_untyped(ComputePipeline::new(
                    &device,
                    &ShaderSource::Hlsl("shaders/dof_y.hlsl"),
                    "dof_y",
                    &Default::default(),
                    false,
                )),
            ),
            (
                "fft_horizontal_forwards",
                node_callbacks::fft_horizontal_forwards,
                box_untyped(ComputePipeline::new(
                    &device,
                    &ShaderSource::Hlsl("shaders/FFTConvolutionBloom/Assets/Shaders/FFTCS.compute"),
                    "TwoForOneFFTForwardHorizontal",
                    &Default::default(),
                    true,
                )),
            ),
            (
                "blit_compute",
                node_callbacks::blit_compute,
                box_untyped(ComputePipeline::new(
                    &device,
                    &ShaderSource::Hlsl("shaders/blit_compute.hlsl"),
                    "blit_compute",
                    &Default::default(),
                    false,
                )),
            ),
            (
                "fft_vertical",
                node_callbacks::fft_vertical,
                box_untyped(ComputePipeline::new(
                    &device,
                    &ShaderSource::Hlsl("shaders/FFTConvolutionBloom/Assets/Shaders/FFTCS.compute"),
                    "FFTVertical",
                    &Default::default(),
                    true,
                )),
            ),
            (
                "fft_horizontal_inverse",
                node_callbacks::fft_horizontal_forwards,
                box_untyped(ComputePipeline::new(
                    &device,
                    &ShaderSource::Hlsl("shaders/FFTConvolutionBloom/Assets/Shaders/FFTCS.compute"),
                    "TwoForOneFFTInverseHorizontal",
                    &Default::default(),
                    true,
                )),
            ),
            (
                "fft_convolute",
                node_callbacks::fft_convolute,
                box_untyped(ComputePipeline::new(
                    &device,
                    &ShaderSource::Hlsl("shaders/FFTConvolutionBloom/Assets/Shaders/FFTCS.compute"),
                    "Convolution",
                    &Default::default(),
                    true,
                )),
            ),
            (
                "fft_kernel_transform",
                node_callbacks::fft_kernel_transform,
                box_untyped(ComputePipeline::new(
                    &device,
                    &ShaderSource::Hlsl("shaders/FFTConvolutionBloom/Assets/Shaders/FFTCS.compute"),
                    "KernelTransform",
                    &Default::default(),
                    true,
                )),
            ),
        ];

        node_names.remove("clear_color");
        node_names.remove("clear_depth_stencil");

        for (name, callback, data) in callbacks {
            if node_names.remove(name) {
                bind_node_callback(subprogram, name, Some(*callback), *data).unwrap()
            }
        }

        dbg!(node_names);

        let mut completed_frame_index = u64::max_value();
        let mut frame_index = 0;
        let mut first_time = true;
        let start_time = Instant::now();

        event_loop.run(move |event, _, control_flow| {
            let user_data = unsafe { &mut *(user_data_raw as *mut UserData) };
            user_data.platform.handle_event(&event);

            match event {
                winit::event::Event::WindowEvent { event, .. } => match event {
                    winit::event::WindowEvent::KeyboardInput { input, .. } => {
                        let pressed = input.state == ElementState::Pressed;

                        match input.virtual_keycode {
                            Some(VirtualKeyCode::W | VirtualKeyCode::Up) => {
                                keyboard_state.forwards = pressed;
                            }
                            Some(VirtualKeyCode::A | VirtualKeyCode::Left) => {
                                keyboard_state.left = pressed;
                            }
                            Some(VirtualKeyCode::S | VirtualKeyCode::Down) => {
                                keyboard_state.backwards = pressed;
                            }
                            Some(VirtualKeyCode::D | VirtualKeyCode::Right) => {
                                keyboard_state.right = pressed;
                            }
                            Some(VirtualKeyCode::G) => {
                                if pressed {
                                    keyboard_state.cursor_grab = !keyboard_state.cursor_grab;

                                    if keyboard_state.cursor_grab {
                                        // Try both methods of grabbing the cursor.
                                        let result = user_data
                                            .window
                                            .set_cursor_grab(winit::window::CursorGrabMode::Locked)
                                            .or_else(|_| {
                                                user_data.window.set_cursor_grab(
                                                    winit::window::CursorGrabMode::Confined,
                                                )
                                            });

                                        if let Err(error) = result {
                                            eprintln!(
                                            "Got an error when trying to set the cursor grab: {}",
                                            error
                                        );
                                        }
                                    } else {
                                        // This can't fail.
                                        let _ = user_data
                                            .window
                                            .set_cursor_grab(winit::window::CursorGrabMode::None);
                                    }
                                    user_data
                                        .window
                                        .set_cursor_visible(!keyboard_state.cursor_grab);
                                }
                            }
                            Some(VirtualKeyCode::LControl | VirtualKeyCode::RControl) => {
                                keyboard_state.control = pressed
                            }
                            Some(VirtualKeyCode::F) => {
                                if pressed && keyboard_state.control {
                                    fullscreen = !fullscreen;

                                    user_data.window.set_fullscreen(if fullscreen {
                                        Some(winit::window::Fullscreen::Borderless(Some(
                                            user_data.window.current_monitor().unwrap(),
                                        )))
                                    } else {
                                        None
                                    })
                                }
                            }
                            _ => {}
                        }
                    }
                    winit::event::WindowEvent::Resized(size) => {
                        // Reconfigure the surface with the new size
                        config.width = size.width;
                        config.height = size.height;
                        surface.configure(&user_data.device, &config);
                        // On macos the window needs to be redrawn manually after resizing
                        user_data.window.request_redraw();
                    }
                    winit::event::WindowEvent::CloseRequested => {
                        *control_flow = winit::event_loop::ControlFlow::Exit
                    }
                    _ => {}
                },
                winit::event::Event::DeviceEvent { event, .. } => match event {
                    winit::event::DeviceEvent::MouseMotion {
                        delta: (delta_x, delta_y),
                    } => {
                        if keyboard_state.cursor_grab {
                            user_data
                                .camera_rig
                                .driver_mut::<dolly::drivers::YawPitch>()
                                .rotate_yaw_pitch(-0.1 * delta_x as f32, -0.1 * delta_y as f32);
                        }
                    }
                    _ => {}
                },
                winit::event::Event::MainEventsCleared => {
                    {
                        let forwards =
                            keyboard_state.forwards as i32 - keyboard_state.backwards as i32;
                        let right = keyboard_state.right as i32 - keyboard_state.left as i32;

                        let move_vec = user_data.camera_rig.final_transform.rotation
                            * Vec3::new(right as f32, 0.0, -forwards as f32).clamp_length_max(1.0);

                        let delta_time = 1.0 / 60.0;
                        let speed = 3.0;

                        user_data
                            .camera_rig
                            .driver_mut::<dolly::drivers::Position>()
                            .translate(move_vec * delta_time * speed);

                        user_data.camera_rig.update(delta_time);
                    }

                    user_data.window.request_redraw();
                }
                winit::event::Event::RedrawRequested(_) => {
                    user_data
                        .platform
                        .update_time(start_time.elapsed().as_secs_f64());

                    let back_buffer = rps::ResourceDesc {
                        ty: rps::ResourceType::IMAGE_2D,
                        temporal_layers: 1,
                        flags: Default::default(),
                        buffer_image: rps::ResourceBufferImageDesc {
                            image: rps::ResourceImageDesc {
                                width: config.width,
                                height: config.height,
                                mip_levels: 1,
                                sample_count: 1,
                                format: map_wgpu_format_to_rps(swapchain_format),
                                depth_or_array_layers: 1,
                            },
                        },
                    };

                    let kernel_img = rps::ResourceDesc {
                        ty: rps::ResourceType::IMAGE_2D,
                        temporal_layers: 1,
                        flags: Default::default(),
                        buffer_image: rps::ResourceBufferImageDesc {
                            image: rps::ResourceImageDesc {
                                width: kernel.width(),
                                height: kernel.height(),
                                mip_levels: 1,
                                sample_count: 1,
                                format: map_wgpu_format_to_rps(kernel.format()),
                                depth_or_array_layers: 1,
                            },
                        },
                    };

                    let args: &[rps::Constant] = &[
                        (&back_buffer) as *const rps::ResourceDesc as _,
                        (&kernel_img) as *const rps::ResourceDesc as _,
                    ];

                    let frame = surface
                        .get_current_texture()
                        .expect("Failed to acquire next swap chain texture");

                    let backbuffer_ptr = box_untyped(Resource::SurfaceFrame(
                        frame.texture.create_view(&Default::default()),
                    ));

                    let kernel_img_ptr = box_untyped(Resource::SurfaceFrame(
                        kernel.create_view(&Default::default()),
                    ));

                    let arg_resources = &[
                        (&backbuffer_ptr) as *const *mut c_void as _,
                        (&kernel_img_ptr) as *const *mut c_void as _,
                    ];

                    let update_info = rps::RenderGraphUpdateInfo {
                        frame_index,
                        gpu_completed_frame_index: completed_frame_index,
                        diagnostic_flags: if first_time {
                            rps::DiagnosticFlags::empty()
                        } else {
                            rps::DiagnosticFlags::empty()
                        },
                        num_args: args.len() as u32,
                        args: args.as_ptr(),
                        arg_resources: arg_resources.as_ptr(),
                        ..Default::default()
                    };

                    first_time = false;

                    rps::render_graph_update(graph, &update_info).unwrap();

                    let layout = rps::render_graph_get_batch_layout(graph).unwrap();

                    for batch in layout.cmd_batches() {
                        let encoder = user_data.device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor { label: None },
                        );

                        let mut cb = CommandBuffer {
                            encoder: Some(encoder),
                        };

                        let cb_ptr = &cb as *const CommandBuffer;

                        rps::render_graph_record_commands(
                            graph,
                            &rps::RenderGraphRecordCommandInfo {
                                user_context: user_data_raw as *mut std::ffi::c_void,
                                cmd_buffer: rps::RuntimeCommandBuffer::from_raw(cb_ptr as _),
                                frame_index,
                                cmd_begin_index: batch.cmd_begin,
                                num_cmds: batch.num_cmds,
                                flags: Default::default(),
                            },
                        )
                        .unwrap();

                        let encoder = cb.encoder.take().unwrap();

                        user_data.queue.submit(Some(encoder.finish()));
                    }

                    completed_frame_index = frame_index;
                    frame_index += 1;

                    frame.present();
                }
                _ => {}
            }
        });
    }
}

enum Resource {
    SurfaceFrame(wgpu::TextureView),
    Texture(wgpu::Texture),
}

impl Resource {
    pub fn as_texture_view(
        &self,
        image_view: rps::ImageView,
    ) -> BorrowedOrOwned<wgpu::TextureView> {
        match self {
            Self::Texture(texture) => {
                let texture_view =
                    texture.create_view(&map_image_view_to_texture_view_desc(image_view));
                BorrowedOrOwned::Owned(texture_view)
            }
            Self::SurfaceFrame(texture_view) => BorrowedOrOwned::Borrowed(texture_view),
        }
    }

    pub fn as_texture_unwrap(&self) -> &wgpu::Texture {
        match self {
            Self::Texture(texture) => texture,
            Self::SurfaceFrame(texture_view) => panic!(),
        }
    }
}

enum BorrowedOrOwned<'a, T> {
    Owned(T),
    Borrowed(&'a T),
}

impl<'a, T> std::ops::Deref for BorrowedOrOwned<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        match self {
            Self::Owned(owned) => owned,
            Self::Borrowed(borrowed) => borrowed,
        }
    }
}

pub fn map_wgpu_format_to_rps(format: wgpu::TextureFormat) -> rps::Format {
    match format {
        wgpu::TextureFormat::Bgra8UnormSrgb => rps::Format::B8G8R8A8_UNORM_SRGB,
        wgpu::TextureFormat::Rgba8UnormSrgb => rps::Format::R8G8B8A8_UNORM_SRGB,
        other => panic!("{:?}", other),
    }
}

pub fn map_rps_format_to_wgpu(format: rps::Format) -> Option<wgpu::TextureFormat> {
    Some(match format {
        rps::Format::B8G8R8A8_UNORM_SRGB => wgpu::TextureFormat::Bgra8UnormSrgb,
        rps::Format::R16G16B16A16_FLOAT => wgpu::TextureFormat::Rgba16Float,
        rps::Format::R32G32B32A32_FLOAT => wgpu::TextureFormat::Rgba32Float,
        rps::Format::D32_FLOAT => wgpu::TextureFormat::Depth32Float,
        rps::Format::UNKNOWN => return None,
        rps::Format::R32_FLOAT => wgpu::TextureFormat::R32Float,
        rps::Format::R16_FLOAT => wgpu::TextureFormat::R16Float,
        rps::Format::R16G16_FLOAT => wgpu::TextureFormat::Rg16Float,
        rps::Format::R9G9B9E5_SHAREDEXP => wgpu::TextureFormat::Rgb9e5Ufloat,
        other => panic!("{:?}", other),
    })
}

#[derive(Default)]
struct KeyboardState {
    forwards: bool,
    right: bool,
    left: bool,
    backwards: bool,
    cursor_grab: bool,
    control: bool,
}

fn map_image_view_to_texture_view_desc(
    image_view: rps::ImageView,
) -> wgpu::TextureViewDescriptor<'static> {
    wgpu::TextureViewDescriptor {
        label: None,
        base_mip_level: image_view.subresource_range.base_mip_level as u32,
        mip_level_count: Some(image_view.subresource_range.mip_levels as u32),
        base_array_layer: image_view.subresource_range.base_array_layer,
        array_layer_count: Some(image_view.subresource_range.array_layers),
        format: map_rps_format_to_wgpu(image_view.base.view_format),
        dimension: match image_view.base.flags {
            render_pipeline_shaders::ResourceViewFlags::NONE => {
                if image_view.subresource_range.array_layers == 1 {
                    Some(wgpu::TextureViewDimension::D2)
                } else {
                    None
                }
            }
            other => todo!("{:?}", other),
        },
        aspect: match image_view.component_mapping {
            50462976 => wgpu::TextureAspect::All,
            other => todo!("{}", other),
        },
    }
}

use rps_custom_backend::CmdCallbackContext;

unsafe fn load_texture_view<'a, CmdCtx>(
    context: &CmdCallbackContext<CommandBuffer, UserData, CmdCtx>,
    view: rps::ImageView,
) -> (
    BorrowedOrOwned<'a, wgpu::TextureView>,
    ffi::cpp::ResourceImageDescPacked,
) {
    let resource = &context.resources[view.base.resource_id as usize];
    let image_desc = resource.desc.buffer_image.image;

    let wgpu_resource = &*(resource.hRuntimeResource.ptr as *const Resource);

    (
        wgpu_resource.as_texture_view(view),
        resource.desc.buffer_image.image,
    )
}
