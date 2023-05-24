use std::borrow::Cow;
use wgpu::util::DeviceExt;

pub fn load_ktx2(bytes: &[u8], device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
    let reader = ktx2::Reader::new(bytes).unwrap();

    let header = reader.header();

    let mut bytes = vec![
        0;
        reader
            .levels()
            .map(|level| level.uncompressed_byte_length as usize)
            .sum()
    ];
    let mut offset = 0;

    for level in reader.levels() {
        match header.supercompression_scheme {
            Some(ktx2::SupercompressionScheme::Zstandard) => {
                zstd::bulk::decompress_to_buffer(level.data, &mut bytes[offset..]).unwrap();
            }
            None => bytes[offset..level.data.len()].copy_from_slice(&level.data),
            other => panic!("{:?}", other),
        }
        offset += level.uncompressed_byte_length as usize;
    }

    device.create_texture_with_data(
        &queue,
        &wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: header.pixel_width,
                height: header.pixel_height,
                depth_or_array_layers: header.pixel_depth.max(1).max(header.face_count),
            },
            mip_level_count: header.level_count,
            sample_count: 1,
            dimension: if header.pixel_depth > 1 {
                wgpu::TextureDimension::D3
            } else {
                wgpu::TextureDimension::D2
            },
            format: match header.format.unwrap() {
                ktx2::Format::E5B9G9R9_UFLOAT_PACK32 => wgpu::TextureFormat::Rgb9e5Ufloat,
                ktx2::Format::R8G8B8A8_SRGB => wgpu::TextureFormat::Rgba8UnormSrgb,
                ktx2::Format::R8G8B8A8_UNORM => wgpu::TextureFormat::Rgba8Unorm,
                other => panic!("{:?}", other),
            },
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        &bytes,
    )
}
