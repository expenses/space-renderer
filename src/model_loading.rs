use wgpu::util::DeviceExt;
use glam::{Vec2, Vec3};
use std::ops::Range;
use crate::buffers;
use crate::accessors::PrimitiveReader;
use std::collections::HashMap;
use goth_gltf::default_extensions::Extensions;
use base64::Engine;

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub position: Vec3,
    pub uv: Vec2,
}


fn collect_buffer_view_map(
    gltf: &goth_gltf::Gltf<Extensions>,
    glb_buffer: Option<&[u8]>,
) -> anyhow::Result<HashMap<usize, Vec<u8>>> {
    use std::borrow::Cow;

    let mut buffer_map = HashMap::new();

    if let Some(glb_buffer) = glb_buffer {
        buffer_map.insert(0, Cow::Borrowed(glb_buffer));
    }

    for (index, buffer) in gltf.buffers.iter().enumerate() {
        if buffer
            .extensions
            .ext_meshopt_compression
            .as_ref()
            .map(|ext| ext.fallback)
            .unwrap_or(false)
        {
            continue;
        }

        let uri = match &buffer.uri {
            Some(uri) => uri,
            None => continue,
        };

        let url = url::Url::options().base_url(/*Some(root_url)*/None).parse(uri)?;

        if url.scheme() == "data" {
            let (_mime_type, data) = url
                .path()
                .split_once(',')
                .ok_or_else(|| anyhow::anyhow!("Failed to get data uri split"))?;
            log::warn!("Loading buffers from embedded base64 is inefficient. Consider moving the buffers into a seperate file.");
            buffer_map.insert(
                index,
                Cow::Owned(base64::engine::general_purpose::STANDARD.decode(data)?),
            );
        } else {
            /*buffer_map.insert(
                index,
                Cow::Owned(context.http_client.fetch_bytes(&url, None).await?),
            );*/
            panic!()
        }
    }

    let mut buffer_view_map = HashMap::new();

    for (i, buffer_view) in gltf.buffer_views.iter().enumerate() {
        if let Some(buffer) = buffer_map.get(&buffer_view.buffer) {
            buffer_view_map.insert(
                i,
                buffer[buffer_view.byte_offset..buffer_view.byte_offset + buffer_view.byte_length]
                    .to_vec(),
            );
        }
    }

    Ok(buffer_view_map)
}

pub fn load_gltf_from_bytes(
    bytes: &[u8],
    vertex_buffers: &buffers::VertexBuffers,
    index_buffer: &buffers::IndexBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<(Range<u32>, Range<u32>, wgpu::TextureView)> {
    let (gltf, glb_buffer) = goth_gltf::Gltf::from_bytes(&bytes)?;
    //let node_tree = gltf_helpers::NodeTree::new(&gltf);


    //let buffer_blob = gltf.blob.as_ref().unwrap();

    let mut buffer_view_map = collect_buffer_view_map(&gltf, glb_buffer).unwrap();

    let mut indices = Vec::new();
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();

    for mesh in &gltf.meshes {

        for primitive in &mesh.primitives {
            let reader = PrimitiveReader::new(&gltf, primitive, &buffer_view_map);

            let read_indices = reader.read_indices().unwrap().unwrap();

            let num_vertices = positions.len() as u32;

            indices.extend(read_indices.iter().map(|index| index + num_vertices));

            positions.extend_from_slice(&reader.read_positions().unwrap().unwrap());
            uvs.extend_from_slice(&reader.read_uvs().unwrap().unwrap());
            normals.extend_from_slice(&reader.read_normals().unwrap().unwrap());
            
        }
    }

    for image in &gltf.images {
        if let Some(buffer_view) = image.buffer_view {
            
        }
        dbg!(image);
    }

    let mut encoder = device.create_command_encoder(
        &wgpu::CommandEncoderDescriptor { label: None },
    );

    let vertex_range = vertex_buffers.insert(&positions, &normals, &uvs, device, queue, &mut encoder);
    for mut index in &mut indices {
        *index += vertex_range.start;
    }

    let index_range = index_buffer.insert(&indices, device, queue, &mut encoder);

    dbg!(&vertex_range, &index_range);

    queue.submit(Some(encoder.finish()));

    //let material = gltf.materials().next().unwrap();

    let texture = /*if let Some(texture) = material.emissive_texture() {*/ 
/*
        load_texture_from_gltf(
            texture.texture(),
            "emissive texture",
            buffer_blob,
            device,
            queue,
        )?
    } else {
        */
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        }).create_view(&Default::default())/*
    }*/;

    Ok((vertex_range, index_range, texture))
}

/*
fn load_texture_from_gltf(
    texture: gltf::texture::Texture,
    label: &str,
    buffer_blob: &[u8],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<wgpu::TextureView> {
    let texture_view = match texture.source().source() {
        gltf::image::Source::View { view, .. } => view,
        _ => {
            return Err(anyhow::anyhow!(
                "Image source is a uri which we don't support"
            ))
        }
    };

    let texture_start = texture_view.offset();
    let texture_end = texture_start + texture_view.length();
    let texture_bytes = &buffer_blob[texture_start..texture_end];

    let decoded_bytes =
        image::load_from_memory_with_format(texture_bytes, image::ImageFormat::Png)?;

    let decoded_rgba8 = decoded_bytes.to_rgba8();

    Ok(device
        .create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: decoded_rgba8.width(),
                    height: decoded_rgba8.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            &*decoded_rgba8,
        )
        .create_view(&Default::default()))
}
*/
