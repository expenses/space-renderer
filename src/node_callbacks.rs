use crate::bindless_textures::BindlessTextures;
use crate::{
    load_texture_view, BorrowedOrOwned, CommandBuffer, ComputePipeline, RenderPipeline, Resource,
    UserData,
};
use egui_wgpu_backend::ScreenDescriptor;
use glam::{Mat4, Vec3};
use rps_custom_backend::{ffi, rps, CmdCallbackContext};
use wgpu::util::DeviceExt;

pub unsafe extern "C" fn blit_srgb(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData, RenderPipeline>::new(context);
    let pipeline = &context.command_data;

    let source_view = *context.reinterpret_arg_as::<rps::ImageView>(0);
    let (source, _) = load_texture_view(&context, source_view);
    let dest_view = *context.reinterpret_arg_as::<rps::ImageView>(1);
    let (dest, _) = load_texture_view(&context, dest_view);

    let bind_group = pipeline.bind_group_layouts.create_bind_group(
        &context.user_data.device,
        0,
        &mut vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&source),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&context.user_data.sampler),
            },
        ],
    );

    let mut render_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &dest,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

    render_pass.set_pipeline(&pipeline.pipeline);
    render_pass.set_bind_group(0, &bind_group, &[]);
    render_pass.draw(0..3, 0..1);
}

pub unsafe extern "C" fn draw(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData, RenderPipeline>::new(context);
    let pipeline = &context.command_data;

    let image_view = *(context.args[0] as *const rps::ImageView);
    let depth_view = *(context.args[1] as *const rps::ImageView);

    let image_res = &context.resources[image_view.base.resource_id as usize];

    let (view, img_desc) = load_texture_view(&context, image_view);
    let (depth_view, _) = load_texture_view(&context, depth_view);

    let moon = &context.user_data.moon;

    let moon_bind_group = pipeline.bind_group_layouts.create_bind_group(
        &context.user_data.device,
        0,
        &mut vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureViewArray(
                    &context.user_data.bindless_textures.texture_view_array(),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&context.user_data.repeat_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: context
                    .user_data
                    .model_info_buffer
                    .buffer
                    .as_entire_binding(),
            },
        ],
    );

    let vertex_buffers = context.user_data.vertex_buffers.buffers.load();
    let index_buffer = &context.user_data.index_buffer.buffer();

    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    #[repr(C)]
    struct Instance {
        pub transform: Vec3,
        pub scale: f32,
        pub rotation: glam::Quat,
    }

    context.user_data.rotation += 1.0 / 60.0;

    let instance_buffer =
        context
            .user_data
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&[
                    Instance {
                        transform: Vec3::splat(0.0),
                        scale: 1.0,
                        rotation: glam::Quat::from_rotation_y(context.user_data.rotation),
                    },
                    Instance {
                        transform: Vec3::splat(2.0),
                        scale: 1.0,
                        rotation: Default::default(),
                    },
                ]),
                usage: wgpu::BufferUsages::VERTEX,
            });

    let mut render_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

    let camera_rig = &context.user_data.camera_rig;

    let view_matrix = Mat4::look_at_rh(
        camera_rig.final_transform.position,
        camera_rig.final_transform.position + camera_rig.final_transform.forward(),
        camera_rig.final_transform.up(),
    );

    let perspective_matrix = Mat4::perspective_infinite_reverse_rh(
        59.0_f32.to_radians(),
        img_desc.width as f32 / img_desc.height as f32,
        0.001,
    );

    let mut bytes = [0; 76];
    bytes[..64].copy_from_slice(&bytemuck::bytes_of(&(perspective_matrix * view_matrix)));
    bytes[64..].copy_from_slice(&bytemuck::bytes_of(&camera_rig.final_transform.position));

    render_pass.set_pipeline(&pipeline.pipeline);
    render_pass.set_vertex_buffer(0, vertex_buffers.position.slice(..));
    render_pass.set_vertex_buffer(1, vertex_buffers.uv.slice(..));
    render_pass.set_vertex_buffer(2, vertex_buffers.normal.slice(..));
    render_pass.set_vertex_buffer(3, vertex_buffers.material_id.slice(..));
    render_pass.set_vertex_buffer(4, instance_buffer.slice(..));
    render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    render_pass.set_bind_group(0, &moon_bind_group, &[]);
    render_pass.set_push_constants(
        wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        0,
        &bytes,
    );
    render_pass.draw_indexed(moon.indices.clone(), 0, 0..1);
    render_pass.draw_indexed(context.user_data.bloom.indices.clone(), 0, 1..2);

    //render_pass.draw(0..3, 0..1);
}

pub unsafe extern "C" fn downsample_initial(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData, ComputePipeline>::new(context);
    let pipeline = &context.command_data;

    let hdr_view = *(context.args[0] as *const rps::ImageView);
    let bloom_texture_view = *(context.args[1] as *const rps::ImageView);

    let hdr = &context.resources[hdr_view.base.resource_id as usize];
    let bloom_texture = &context.resources[bloom_texture_view.base.resource_id as usize];

    let img_desc = bloom_texture.desc.buffer_image.image;

    let hdr = &*(hdr.hRuntimeResource.ptr as *const Resource);
    let bloom_texture = &*(bloom_texture.hRuntimeResource.ptr as *const Resource);

    let hdr = hdr.as_texture_view(hdr_view);
    let bloom_texture = bloom_texture.as_texture_view(bloom_texture_view);

    let bind_group = pipeline.bind_group_layouts.create_bind_group(
        &context.user_data.device,
        0,
        &mut vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&hdr),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&context.user_data.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&bloom_texture),
            },
        ],
    );

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    compute_pass.set_pipeline(&pipeline.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.set_push_constants(0, bytemuck::cast_slice(&context.user_data.filter_constants));
    compute_pass.dispatch_workgroups(
        dispatch_count(img_desc.width, 8),
        dispatch_count(img_desc.height, 8),
        1,
    );
}

// Upsample or downsample.
pub unsafe extern "C" fn downsample(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData, ComputePipeline>::new(context);
    let pipeline = &context.command_data;

    let source_view = *(context.args[0] as *const rps::ImageView);
    let dest_view = *(context.args[1] as *const rps::ImageView);

    let (source, _) = load_texture_view(&context, source_view);
    let (dest, dest_desc) = load_texture_view(&context, dest_view);
    let dest_mip = dest_view.subresource_range.base_mip_level;

    let bind_group = pipeline.bind_group_layouts.create_bind_group(
        &context.user_data.device,
        0,
        &mut vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&source),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&context.user_data.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&dest),
            },
        ],
    );

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    compute_pass.set_pipeline(&pipeline.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.dispatch_workgroups(
        dispatch_count(dest_desc.width >> dest_mip, 8),
        dispatch_count(dest_desc.height >> dest_mip, 8),
        1,
    );
}
/*
pub unsafe extern "C" fn upsample(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData, ComputePipeline>::new(context);
    let pipeline = &context.command_data;

    let source_view = *(context.args[0] as *const rps::ImageView);
    let dest_view = *(context.args[1] as *const rps::ImageView);

    let (source, _) = load_texture_view(&context, source_view);
    let (dest, dest_desc) = load_texture_view(&context, dest_view);
    let dest_mip = dest_view.subresource_range.base_mip_level;

    let bind_group = pipeline
        .bind_group_layouts
        .create_bind_group(
            &context.user_data.device,
            0,
            &mut vec![
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&context.user_data.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&dest),
                },
            ],
        );

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    let dest_mip = dest_view.subresource_range.base_mip_level;

    compute_pass.set_pipeline(&pipeline.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.dispatch_workgroups(
        dispatch_count(dest_desc.width >> dest_mip, 8),
        dispatch_count(dest_desc.height >> dest_mip, 8),
        1,
    );
}
*/
pub unsafe extern "C" fn tonemap(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData, ComputePipeline>::new(context);
    let pipeline = &context.command_data;

    let hdr_view = *(context.args[0] as *const rps::ImageView);

    let (hdr, hdr_desc) = load_texture_view(&context, hdr_view);

    let bind_group = pipeline.bind_group_layouts.create_bind_group(
        &context.user_data.device,
        0,
        &mut vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&hdr),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(
                    &context
                        .user_data
                        .tonemap_tex
                        .create_view(&Default::default()),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&context.user_data.sampler),
            },
        ],
    );

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    compute_pass.set_pipeline(&pipeline.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.dispatch_workgroups(
        dispatch_count(hdr_desc.width, 8),
        dispatch_count(hdr_desc.height, 8),
        1,
    );
}

pub unsafe extern "C" fn compute_dof(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData, ComputePipeline>::new(context);
    let pipeline = &context.command_data;

    let depth_view = *(context.args[0] as *const rps::ImageView);
    let hdr_view = *(context.args[1] as *const rps::ImageView);
    let output_view = *(context.args[2] as *const rps::ImageView);

    let (depth, _) = load_texture_view(&context, depth_view);
    let (hdr, _) = load_texture_view(&context, hdr_view);
    let (output, output_desc) = load_texture_view(&context, output_view);

    let bind_group = pipeline.bind_group_layouts.create_bind_group(
        &context.user_data.device,
        0,
        &mut vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&depth),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&context.user_data.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&hdr),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&output),
            },
        ],
    );

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    compute_pass.set_pipeline(&pipeline.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.dispatch_workgroups(
        dispatch_count(output_desc.width, 8),
        dispatch_count(output_desc.height, 8),
        1,
    );
}

pub unsafe extern "C" fn render_skybox(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData, RenderPipeline>::new(context);
    let pipeline = &context.command_data;

    let image_view = *(context.args[0] as *const rps::ImageView);
    let depth_view = *(context.args[1] as *const rps::ImageView);

    let image_res = &context.resources[image_view.base.resource_id as usize];

    let (view, img_desc) = load_texture_view(&context, image_view);
    let (depth_view, _) = load_texture_view(&context, depth_view);

    let camera_rig = &context.user_data.camera_rig;

    let view_matrix = Mat4::look_at_rh(
        glam::Vec3::ZERO,
        camera_rig.final_transform.forward(),
        camera_rig.final_transform.up(),
    );

    let perspective_matrix = Mat4::perspective_infinite_reverse_rh(
        59.0_f32.to_radians(),
        img_desc.width as f32 / img_desc.height as f32,
        0.001,
    );

    let buffer = context
        .user_data
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            contents: bytemuck::bytes_of(&[perspective_matrix.inverse(), view_matrix.inverse()]),
            label: None,
            usage: wgpu::BufferUsages::UNIFORM,
        });

    let bind_group = pipeline.bind_group_layouts.create_bind_group(
        &context.user_data.device,
        0,
        &mut vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &context
                        .user_data
                        .cubemap
                        .create_view(&wgpu::TextureViewDescriptor {
                            dimension: Some(wgpu::TextureViewDimension::Cube),
                            ..Default::default()
                        }),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&context.user_data.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: buffer.as_entire_binding(),
            },
        ],
    );

    let mut render_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

    render_pass.set_pipeline(&pipeline.pipeline);
    render_pass.set_bind_group(0, &bind_group, &[]);
    render_pass.set_push_constants(
        wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        0,
        bytemuck::bytes_of(&context.user_data.skybox_boost),
    );
    render_pass.draw(0..3, 0..1);
}

pub unsafe extern "C" fn render_ui(context: *const rps::CmdCallbackContext) {
    let mut context = CmdCallbackContext::<CommandBuffer, UserData>::new(context);
    let image_view = *(context.args[0] as *const rps::ImageView);
    let (view, img_desc) = load_texture_view(&context, image_view);

    let user_data = &mut context.user_data;
    user_data.platform.begin_frame();
    let ctx = user_data.platform.context();
    egui::Window::new("Controls").show(&ctx, |ui| {
        ui.add(egui::widgets::DragValue::new(&mut user_data.skybox_boost));
        ui.add(egui::widgets::DragValue::new(&mut user_data.filter_constants[0]).speed(0.05));
        ui.add(egui::widgets::DragValue::new(&mut user_data.filter_constants[1]).speed(0.05));
    });

    let full_output = user_data.platform.end_frame(Some(&user_data.window));
    let paint_jobs = user_data.platform.context().tessellate(full_output.shapes);

    let screen_descriptor = ScreenDescriptor {
        physical_width: img_desc.width,
        physical_height: img_desc.height,
        scale_factor: user_data.window.scale_factor() as f32,
    };

    let tdelta: egui::TexturesDelta = full_output.textures_delta;
    user_data
        .egui_rpass
        .add_textures(&user_data.device, &user_data.queue, &tdelta)
        .expect("add texture ok");
    user_data.egui_rpass.update_buffers(
        &user_data.device,
        &user_data.queue,
        &paint_jobs,
        &screen_descriptor,
    );

    // Record all render passes.
    user_data
        .egui_rpass
        .execute(
            context.command_buffer.encoder.as_mut().unwrap(),
            &view,
            &paint_jobs,
            &screen_descriptor,
            None,
        )
        .unwrap();
}

pub unsafe extern "C" fn dof_downsample_with_coc(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData, ComputePipeline>::new(context);
    let pipeline = &context.command_data;

    let depth_view = *(context.args[0] as *const rps::ImageView);
    let hdr_view = *(context.args[1] as *const rps::ImageView);
    let output_view = *(context.args[2] as *const rps::ImageView);

    let (depth, _) = load_texture_view(&context, depth_view);
    let (hdr, _) = load_texture_view(&context, hdr_view);
    let (output, output_desc) = load_texture_view(&context, output_view);

    let bind_group = pipeline.bind_group_layouts.create_bind_group(
        &context.user_data.device,
        0,
        &mut vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&depth),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&context.user_data.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&hdr),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&output),
            },
        ],
    );

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    compute_pass.set_pipeline(&pipeline.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.dispatch_workgroups(
        dispatch_count(output_desc.width, 8),
        dispatch_count(output_desc.height, 8),
        1,
    );
}

pub unsafe extern "C" fn dof_x(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData, ComputePipeline>::new(context);
    let pipeline = &context.command_data;

    let hdr_view = *(context.args[0] as *const rps::ImageView);
    let output_view = *(context.args[1] as *const rps::ImageView);

    let (hdr, output_desc) = load_texture_view(&context, hdr_view);
    let (output, _) = load_texture_view(&context, output_view);

    let bind_group = pipeline.bind_group_layouts.create_bind_group(
        &context.user_data.device,
        0,
        &mut vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&context.user_data.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&hdr),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&output),
            },
        ],
    );

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    compute_pass.set_pipeline(&pipeline.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.dispatch_workgroups(
        dispatch_count(output_desc.width, 8),
        dispatch_count(output_desc.height, 8),
        1,
    );
}

pub unsafe extern "C" fn dof_y(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData, ComputePipeline>::new(context);
    let pipeline = &context.command_data;

    let hdr_view = *(context.args[0] as *const rps::ImageView);
    let output_view = *(context.args[1] as *const rps::ImageView);
    let horizontally_blurred_view = *(context.args[2] as *const rps::ImageView);

    let (hdr, _) = load_texture_view(&context, hdr_view);
    let (horizontally_blurred, _) = load_texture_view(&context, horizontally_blurred_view);
    let (output, output_desc) = load_texture_view(&context, output_view);

    let bind_group = pipeline.bind_group_layouts.create_bind_group(
        &context.user_data.device,
        0,
        &mut vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&context.user_data.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&hdr),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&horizontally_blurred),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&output),
            },
        ],
    );

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    compute_pass.set_pipeline(&pipeline.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.dispatch_workgroups(
        dispatch_count(output_desc.width, 8),
        dispatch_count(output_desc.height, 8),
        1,
    );
}

pub unsafe extern "C" fn fft_horizontal_forwards(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData, ComputePipeline>::new(context);
    let pipeline = &context.command_data;

    let output_view = *(context.args[0] as *const rps::ImageView);

    let (output, output_desc) = load_texture_view(&context, output_view);

    let bind_group = pipeline.bind_group_layouts.create_bind_group(
        &context.user_data.device,
        0,
        &mut vec![wgpu::BindGroupEntry {
            binding: 2,
            resource: wgpu::BindingResource::TextureView(&output),
        }],
    );

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    compute_pass.set_pipeline(&pipeline.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.dispatch_workgroups(output_desc.width, 1, 1);
}

pub unsafe extern "C" fn fft_vertical(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData, ComputePipeline>::new(context);
    let pipeline = &context.command_data;

    let output_view = *(context.args[0] as *const rps::ImageView);
    let forwards = *(context.args[1] as *const bool);

    let (output, output_desc) = load_texture_view(&context, output_view);

    let buffer = context
        .user_data
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            contents: bytemuck::bytes_of(&(forwards as u32 as f32).to_le_bytes()),
            label: None,
            usage: wgpu::BufferUsages::UNIFORM,
        });

    let bind_group = pipeline.bind_group_layouts.create_bind_group(
        &context.user_data.device,
        0,
        &mut vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&output),
            },
        ],
    );

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    compute_pass.set_pipeline(&pipeline.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.dispatch_workgroups(output_desc.width, 1, 1);
}

/*
pub unsafe extern "C" fn fft_horizontal_inverse(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData, ComputePipeline>::new(context);
    let pipeline = &context.command_data;

    let output_view = *(context.args[0] as *const rps::ImageView);

    let (output, output_desc) = load_texture_view(&context, output_view);

    let bind_group = pipeline
        .bind_group_layouts
        .create_bind_group(
            &context.user_data.device,
            0,
            &mut vec![
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&output),
                },
            ],
        );

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    compute_pass.set_pipeline(&pipeline.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.dispatch_workgroups(
        output_desc.width,
        1,
        1,
    );
}
*/

pub unsafe extern "C" fn blit_compute(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData, ComputePipeline>::new(context);
    let pipeline = &context.command_data;

    let source_view = *(context.args[0] as *const rps::ImageView);
    let output_view = *(context.args[1] as *const rps::ImageView);

    let (source, _) = load_texture_view(&context, source_view);
    let (output, output_desc) = load_texture_view(&context, output_view);

    let bind_group = pipeline.bind_group_layouts.create_bind_group(
        &context.user_data.device,
        0,
        &mut vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&source),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&context.user_data.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&output),
            },
        ],
    );

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    compute_pass.set_pipeline(&pipeline.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.dispatch_workgroups(
        dispatch_count(output_desc.width, 8),
        dispatch_count(output_desc.height, 8),
        1,
    );
}

pub unsafe extern "C" fn fft_convolute(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData, ComputePipeline>::new(context);
    let pipeline = &context.command_data;

    let source_view = *(context.args[0] as *const rps::ImageView);
    let kernel_view = *(context.args[1] as *const rps::ImageView);

    let (source, source_desc) = load_texture_view(&context, source_view);
    let (kernel, _) = load_texture_view(&context, kernel_view);

    let bind_group = pipeline.bind_group_layouts.create_bind_group(
        &context.user_data.device,
        0,
        &mut vec![
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&source),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&kernel),
            },
        ],
    );

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    compute_pass.set_pipeline(&pipeline.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.dispatch_workgroups(
        dispatch_count(source_desc.width, 8),
        dispatch_count(source_desc.height, 8),
        1,
    );
}

pub unsafe extern "C" fn fft_kernel_transform(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData, ComputePipeline>::new(context);
    let pipeline = &context.command_data;

    let kernel_view = *(context.args[0] as *const rps::ImageView);

    let (kernel, kernel_desc) = load_texture_view(&context, kernel_view);

    let bind_group = pipeline.bind_group_layouts.create_bind_group(
        &context.user_data.device,
        0,
        &mut vec![wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::TextureView(&kernel),
        }],
    );

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    compute_pass.set_pipeline(&pipeline.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.dispatch_workgroups(
        dispatch_count(kernel_desc.width, 8),
        dispatch_count(kernel_desc.height >> 1, 8),
        1,
    );
}

const fn dispatch_count(num: u32, group_size: u32) -> u32 {
    ((num - 1) / group_size) + 1
}
