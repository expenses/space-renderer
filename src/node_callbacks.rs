use crate::{load_texture_view, BorrowedOrOwned, CommandBuffer, Resource, UserData};
use glam::Mat4;
use rps_custom_backend::{ffi, rps, CmdCallbackContext};

pub unsafe extern "C" fn blit(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData>::new(context);

    let source_view = *context.reinterpret_arg_as::<rps::ImageView>(0);
    let (source, _) = load_texture_view(&context, source_view);
    let dest_view = *context.reinterpret_arg_as::<rps::ImageView>(1);
    let (dest, _) = load_texture_view(&context, dest_view);

    let bind_group = context
        .user_data
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &context.user_data.blit_pipeline.bind_group_layouts[&0],
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&context.user_data.sampler),
                },
            ],
        });

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

    render_pass.set_pipeline(&context.user_data.blit_pipeline.pipeline);
    render_pass.set_bind_group(0, &bind_group, &[]);
    render_pass.draw(0..3, 0..1);
}

pub unsafe extern "C" fn draw(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData>::new(context);

    let image_view = *(context.args[0] as *const rps::ImageView);
    let depth_view = *(context.args[1] as *const rps::ImageView);

    let image_res = &context.resources[image_view.base.resource_id as usize];

    let (view, img_desc) = load_texture_view(&context, image_view);
    let (depth_view, _) = load_texture_view(&context, depth_view);

    let (_, index_range, tex) = &context.user_data.gltf;
    let (_, moon_index_range, _) = &context.user_data.moon;
    

    let bind_group = context
        .user_data
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &context.user_data.pipeline_3d.bind_group_layouts[&0],
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&tex),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&context.user_data.sampler),
                },
            ],
        });

        let moon_bind_group = context
        .user_data
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &context.user_data.moon_pipeline.bind_group_layouts[&0],
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&context.user_data.moon_colour.create_view(&Default::default())),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&context.user_data.repeat_sampler),
                },
            ],
        });

    let vertex_buffers = context.user_data.vertex_buffers.buffers.load();
    let index_buffer = &context.user_data.index_buffer.buffer();


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

    render_pass.set_pipeline(&context.user_data.pipeline_3d.pipeline);
    render_pass.set_bind_group(0, &bind_group, &[]);
    render_pass.set_vertex_buffer(0, vertex_buffers.position.slice(..));
    render_pass.set_vertex_buffer(1, vertex_buffers.uv.slice(..));
    render_pass.set_vertex_buffer(2, vertex_buffers.normal.slice(..));
    render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    render_pass.set_push_constants(
        wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        0,
        bytemuck::bytes_of(&(perspective_matrix * view_matrix)),
    );
    //render_pass.draw_indexed(index_range.clone(), 0, 0..1);
    render_pass.set_pipeline(&context.user_data.moon_pipeline.pipeline);
    render_pass.set_bind_group(0, &moon_bind_group, &[]);
    render_pass.set_push_constants(
        wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        0,
        bytemuck::bytes_of(&(perspective_matrix * view_matrix)),
    );
    render_pass.draw_indexed(moon_index_range.clone(), 0, 0..1);
    //render_pass.draw(0..3, 0..1);
}

pub unsafe extern "C" fn downsample_initial(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData>::new(context);

    let hdr_view = *(context.args[0] as *const rps::ImageView);
    let bloom_texture_view = *(context.args[1] as *const rps::ImageView);
    let constants = *(context.args[2] as *const [f32; 2]);

    let hdr = &context.resources[hdr_view.base.resource_id as usize];
    let bloom_texture = &context.resources[bloom_texture_view.base.resource_id as usize];

    let img_desc = bloom_texture.desc.buffer_image.image;

    let hdr = &*(hdr.hRuntimeResource.ptr as *const Resource);
    let bloom_texture = &*(bloom_texture.hRuntimeResource.ptr as *const Resource);

    let hdr = hdr.as_texture_view(hdr_view);
    let bloom_texture = bloom_texture.as_texture_view(bloom_texture_view);

    let bind_group = context
        .user_data
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &context.user_data.downsample_initial.bind_group_layouts[&0],
            entries: &[
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
        });

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    compute_pass.set_pipeline(&context.user_data.downsample_initial.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.set_push_constants(0, bytemuck::cast_slice(&constants));
    compute_pass.dispatch_workgroups(
        dispatch_count(img_desc.width, 8),
        dispatch_count(img_desc.height, 8),
        1,
    );
}

pub unsafe extern "C" fn downsample(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData>::new(context);

    let source_view = *(context.args[0] as *const rps::ImageView);
    let dest_view = *(context.args[1] as *const rps::ImageView);

    let (source, _) = load_texture_view(&context, source_view);
    let (dest, dest_desc) = load_texture_view(&context, dest_view);
    let dest_mip = dest_view.subresource_range.base_mip_level;

    let bind_group = context
        .user_data
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &context.user_data.downsample.bind_group_layouts[&0],
            entries: &[
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
        });

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    compute_pass.set_pipeline(&context.user_data.downsample.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.dispatch_workgroups(
        dispatch_count(dest_desc.width >> dest_mip, 8),
        dispatch_count(dest_desc.height >> dest_mip, 8),
        1,
    );
}

pub unsafe extern "C" fn upsample(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData>::new(context);

    let source_view = *(context.args[0] as *const rps::ImageView);
    let dest_view = *(context.args[1] as *const rps::ImageView);

    let (source, _) = load_texture_view(&context, source_view);
    let (dest, dest_desc) = load_texture_view(&context, dest_view);
    let dest_mip = dest_view.subresource_range.base_mip_level;

    let bind_group = context
        .user_data
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &context.user_data.upsample.bind_group_layouts[&0],
            entries: &[
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
        });

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    let dest_mip = dest_view.subresource_range.base_mip_level;

    compute_pass.set_pipeline(&context.user_data.upsample.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.dispatch_workgroups(
        dispatch_count(dest_desc.width >> dest_mip, 8),
        dispatch_count(dest_desc.height >> dest_mip, 8),
        1,
    );
}


pub unsafe extern "C" fn tonemap(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData>::new(context);

    let hdr_view = *(context.args[0] as *const rps::ImageView);

    let (hdr, hdr_desc) = load_texture_view(&context, hdr_view);

    let bind_group = context
        .user_data
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &context.user_data.tonemap.bind_group_layouts[&0],
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&hdr),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&context.user_data.tonemap_tex.create_view(&Default::default())),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&context.user_data.sampler),
                },
            ],
        });

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    compute_pass.set_pipeline(&context.user_data.tonemap.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.dispatch_workgroups(
        dispatch_count(hdr_desc.width, 8),
        dispatch_count(hdr_desc.height, 8),
        1,
    );
}


pub unsafe extern "C" fn linearize_depth(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData>::new(context);

    let source_view = *(context.args[0] as *const rps::ImageView);
    //let hdr_view = *(context.args[1] as *const rps::ImageView);
    let output_view = *(context.args[1] as *const rps::ImageView);

    let (source, _) = load_texture_view(&context, source_view);
    //let (hdr, _) = load_texture_view(&context, hdr_view);
    let (output, output_desc) = load_texture_view(&context, output_view);

    let bind_group = context
        .user_data
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &context.user_data.linearize_depth.bind_group_layouts[&0],
            entries: &[
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
        });

    let mut compute_pass = context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

    compute_pass.set_pipeline(&context.user_data.linearize_depth.pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    compute_pass.dispatch_workgroups(
        dispatch_count(output_desc.width, 8),
        dispatch_count(output_desc.height, 8),
        1,
    );
}

pub unsafe extern "C" fn render_skybox(context: *const rps::CmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData>::new(context);

    let image_view = *(context.args[0] as *const rps::ImageView);
    let depth_view = *(context.args[1] as *const rps::ImageView);

    let image_res = &context.resources[image_view.base.resource_id as usize];

    let (view, img_desc) = load_texture_view(&context, image_view);
    let (depth_view, _) = load_texture_view(&context, depth_view);

    let bind_group = context
        .user_data
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &context.user_data.skybox_pipeline.bind_group_layouts[&0],
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&context.user_data.cubemap.create_view(&wgpu::TextureViewDescriptor {
                        dimension: Some(wgpu::TextureViewDimension::Cube),
                        ..Default::default()
                    })),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&context.user_data.sampler),
                },
            ],
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
                    store: false,
                }),
                stencil_ops: None,
            }),
        });

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

    render_pass.set_pipeline(&context.user_data.skybox_pipeline.pipeline);
    render_pass.set_bind_group(0, &bind_group, &[]);
    render_pass.set_push_constants(
        wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        0,
        bytemuck::bytes_of(&[perspective_matrix.inverse(), view_matrix.inverse()]),
    );
    render_pass.draw(0..3, 0..1);
}


const fn dispatch_count(num: u32, group_size: u32) -> u32 {
    ((num - 1) / group_size) + 1
}
