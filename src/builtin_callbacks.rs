use crate::{
    load_texture_view, map_rps_format_to_wgpu, BorrowedOrOwned, CommandBuffer, Resource, UserData,
};
use rps_custom_backend::{
    array_ref_to_mut_slice, ffi,
    rps::{self, AccessFlags, ResourceType},
    vector_to_slice, CmdCallbackContext,
};
use std::ffi::c_void;

pub unsafe extern "C" fn record_commands(
    render_graph: *const c_void,
    record_info: *const ffi::RpsRenderGraphRecordCommandInfo,
) -> ffi::RpsResult {
    let record_info = *(record_info as *const rps::RenderGraphRecordCommandInfo);
    let render_graph = &*(render_graph as *const ffi::cpp::RenderGraph);
    let runtime_cmd_infos = vector_to_slice(&render_graph.m_runtimeCmdInfos);
    let cmd_infos = vector_to_slice(&render_graph.m_cmds);

    for runtime_cmd_info in &runtime_cmd_infos[record_info.cmd_begin_index as usize
        ..record_info.cmd_begin_index as usize + record_info.num_cmds as usize]
    {
        if runtime_cmd_info.isTransition() != 0 {
            continue;
        }

        let cmd_info = &cmd_infos[runtime_cmd_info.cmdId() as usize];

        //dbg!(*cmd_info.pCmdDecl);

        //println!("{} - {}", runtime_cmd_info.isTransition(), runtime_cmd_info.cmdId());
    }

    rps::Result::OK.into_raw()
}

pub unsafe extern "C" fn create_resources(
    _context: *const c_void,
    array: *mut c_void,
    user_data: *mut c_void,
) -> ffi::RpsResult {
    let user_data = &mut *(user_data as *mut UserData);

    let arr = array as *mut ffi::cpp::ArrayRef<ffi::cpp::ResourceInstance, u64>;

    let resources = unsafe { array_ref_to_mut_slice(&mut *arr) };

    for mut resource in resources.iter_mut() {
        if resource.isExternal() {
            continue;
        }

        if !resource.isPendingCreate() {
            continue;
        }

        let access = resource.allAccesses._base;

        let access: rps::AccessAttr = std::mem::transmute(access);

        let access_flags = access.access_flags;

        match ResourceType::from_raw(resource.desc.type_()) {
            ResourceType::IMAGE_2D => {
                let mut usage = wgpu::TextureUsages::empty();

                if access_flags.intersects(
                    AccessFlags::RENDER_TARGET
                        | AccessFlags::DEPTH_WRITE
                        | AccessFlags::STENCIL_WRITE,
                ) {
                    usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
                }

                if access_flags.contains(AccessFlags::SHADER_RESOURCE) {
                    usage |= wgpu::TextureUsages::TEXTURE_BINDING;
                }

                if access_flags.contains(AccessFlags::UNORDERED_ACCESS) {
                    usage |= wgpu::TextureUsages::STORAGE_BINDING;
                }

                if usage.is_empty() {
                    eprintln!("Texture has no usages. access: {:?}", access_flags);
                    usage |= wgpu::TextureUsages::TEXTURE_BINDING;
                }

                let image = resource.desc.buffer_image.image;

                let texture = user_data.device.create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width: image.width,
                        height: image.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: image.mipLevels(),
                    sample_count: image.sampleCount(),
                    dimension: wgpu::TextureDimension::D2,
                    format: map_rps_format_to_wgpu(rps::Format::from_raw(image.format())).unwrap(),
                    view_formats: &[
                        map_rps_format_to_wgpu(rps::Format::from_raw(image.format())).unwrap(),
                    ],
                    usage,
                });

                resource.allocPlacement.heapId = 0;
                resource.hRuntimeResource.ptr =
                    Box::into_raw(Box::new(Resource::Texture(texture))) as _;
                resource.prevFinalAccess = resource.initialAccess;
                resource.set_isPendingCreate(false);
            }
            _ => panic!(),
        }
    }

    rps::Result::OK.into_raw()
}

pub unsafe extern "C" fn destroy_runtime_resource_deferred(
    resource: *mut c_void,
    _user_data: *mut c_void,
) {
    let mut resource = &mut *(resource as *mut ffi::cpp::ResourceInstance);

    let _ = Box::from_raw(resource.hRuntimeResource.ptr as *mut Resource);

    resource.hRuntimeResource.ptr = std::ptr::null_mut();
}

pub unsafe extern "C" fn clear_color(context: *const ffi::RpsCmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData>::new(context as _);

    let image_view = *context.reinterpret_arg_as::<rps::ImageView>(0);

    let clear_value = context.reinterpret_arg_as::<rps::ClearValue>(1);
    let clear_value = clear_value.color.float32;

    let (view, _) = load_texture_view(&context, image_view);

    context
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
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: clear_value[0] as f64,
                        g: clear_value[1] as f64,
                        b: clear_value[2] as f64,
                        a: clear_value[3] as f64,
                    }),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });
}

pub unsafe extern "C" fn clear_depth_stencil(context: *const ffi::RpsCmdCallbackContext) {
    let context = CmdCallbackContext::<CommandBuffer, UserData>::new(context as _);

    let image_view = *context.reinterpret_arg_as::<rps::ImageView>(0);
    let flags = *context.reinterpret_arg_as::<rps::ClearFlags>(1);
    let depth_value = *context.reinterpret_arg_as::<f32>(2);

    assert_eq!(flags, rps::ClearFlags::DEPTH);

    let mut depth_ops = if flags.contains(rps::ClearFlags::DEPTH) {
        Some(wgpu::Operations {
            load: wgpu::LoadOp::Clear(depth_value),
            store: true,
        })
    } else {
        None
    };

    let (view, _) = load_texture_view(&context, image_view);

    context
        .command_buffer
        .encoder
        .as_mut()
        .unwrap()
        .begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &view,
                depth_ops: depth_ops,
                stencil_ops: None,
            }),
        });
}
