use crate::accessors::PrimitiveReader;
use crate::bindless_textures::BindlessTextures;
use crate::buffers;
use crate::buffers::VecGpuBuffer;
use crate::texture_loading::load_ktx2;
use base64::Engine;
use glam::{Vec2, Vec3, Vec4};
use goth_gltf::default_extensions::Extensions;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use wgpu::util::DeviceExt;

#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
pub struct MaterialInfo {
    pub base_color_factor: Vec4,
    pub emissive_factor: Vec3,
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub albedo_texture: u32,
    pub normal_texture: u32,
    pub emissive_texture: u32,
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub position: Vec3,
    pub uv: Vec2,
}

fn collect_buffer_view_map(
    path: &std::path::Path,
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

        if uri.starts_with("data") {
            let (_mime_type, data) = uri
                .split_once(',')
                .ok_or_else(|| anyhow::anyhow!("Failed to get data uri split"))?;
            log::warn!("Loading buffers from embedded base64 is inefficient. Consider moving the buffers into a seperate file.");
            buffer_map.insert(
                index,
                Cow::Owned(base64::engine::general_purpose::STANDARD.decode(data)?),
            );
        } else {
            let mut path = std::path::PathBuf::from(path);
            path.set_file_name(uri);
            buffer_map.insert(index, Cow::Owned(std::fs::read(&path).unwrap()));
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

pub struct Model {
    pub indices: Range<u32>,
    pub vertices: Range<u32>,
    pub textures: Range<u32>,
    pub material_infos: Range<u32>,
}

pub fn load_gltf<P: std::convert::AsRef<std::path::Path> + Sync>(
    path: P,
    vertex_buffers: &buffers::VertexBuffers,
    index_buffer: &buffers::IndexBuffer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bindless_textures: &mut BindlessTextures,
    material_info_buffer: &mut VecGpuBuffer<MaterialInfo>,
) -> anyhow::Result<Model> {
    let bytes = std::fs::read(path.as_ref()).unwrap();

    let (gltf, glb_buffer) = goth_gltf::Gltf::from_bytes(&bytes)?;
    //let node_tree = gltf_helpers::NodeTree::new(&gltf);

    //let buffer_blob = gltf.blob.as_ref().unwrap();

    let mut buffer_view_map = collect_buffer_view_map(path.as_ref(), &gltf, glb_buffer).unwrap();

    let texture_views = gltf
        .images
        .par_iter()
        .map(|image| {
            if let Some(uri) = &image.uri {
                let mut path = PathBuf::from(path.as_ref());
                path.set_file_name(uri);
                load_ktx2(&std::fs::read(&path).unwrap(), device, queue)
                    .create_view(&Default::default())
            } else {
                panic!()
            }
        })
        .collect::<Vec<_>>();

    let textures_range = bindless_textures.push(texture_views);

    let first_texture = textures_range.start;

    let mut material_infos = Vec::new();

    for material in &gltf.materials {
        material_infos.push(MaterialInfo {
            base_color_factor: material.pbr_metallic_roughness.base_color_factor.into(),
            albedo_texture: material
                .pbr_metallic_roughness
                .base_color_texture
                .as_ref()
                .map(|info| first_texture + info.index as u32)
                .unwrap_or(u32::max_value()),
            metallic_factor: material.pbr_metallic_roughness.metallic_factor,
            roughness_factor: material.pbr_metallic_roughness.roughness_factor,
            normal_texture: material
                .normal_texture
                .as_ref()
                .map(|info| first_texture + info.index as u32)
                .unwrap_or(u32::max_value()),
            emissive_factor: Vec3::from(material.emissive_factor)
                * material
                    .extensions
                    .khr_materials_emissive_strength
                    .map(|ext| ext.emissive_strength)
                    .unwrap_or(1.0),
            emissive_texture: material
                .emissive_texture
                .as_ref()
                .map(|info| first_texture + info.index as u32)
                .unwrap_or(u32::max_value()),
        });
    }

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    let material_info_range =
        material_info_buffer.push(&material_infos, &device, &queue, &mut encoder);

    let mut indices = Vec::new();
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut material_ids = Vec::new();

    for mesh in &gltf.meshes {
        for primitive in &mesh.primitives {
            let material_id = primitive.material.unwrap_or(0);

            let reader = PrimitiveReader::new(&gltf, primitive, &buffer_view_map);

            let read_indices = reader.read_indices().unwrap().unwrap();

            let num_vertices = positions.len() as u32;

            indices.extend(read_indices.iter().map(|index| index + num_vertices));

            let prim_positions = reader.read_positions().unwrap().unwrap();

            positions.extend_from_slice(&prim_positions);
            uvs.extend_from_slice(&reader.read_uvs().unwrap().unwrap());
            normals.extend_from_slice(&reader.read_normals().unwrap().unwrap());
            material_ids.extend(
                std::iter::repeat(material_info_range.start + material_id as u32)
                    .take(prim_positions.len()),
            );
        }
    }

    let vertex_range = vertex_buffers.insert(
        &positions,
        &normals,
        &uvs,
        &material_ids,
        device,
        queue,
        &mut encoder,
    );
    for mut index in &mut indices {
        *index += vertex_range.start;
    }

    let index_range = index_buffer.insert(&indices, device, queue, &mut encoder);

    queue.submit(Some(encoder.finish()));

    Ok(Model {
        indices: index_range,
        vertices: vertex_range,
        textures: textures_range,
        material_infos: material_info_range,
    })
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
