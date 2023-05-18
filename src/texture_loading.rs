use wgpu::util::DeviceExt;

pub fn load_ktx2(bytes: &[u8], device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
    let reader = ktx2::Reader::new(bytes).unwrap();

    let header = reader.header();

    dbg!(header);

    let mut layers: Vec<u8> = reader.levels().flat_map(|level| {
        match header.supercompression_scheme {
            Some(ktx2::SupercompressionScheme::Zstandard) => zstd::stream::decode_all(level).unwrap(),
            None => level.to_vec(),
            other => panic!("{:?}", other)
        }
    }).collect();

    device.create_texture_with_data(&queue,
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
                other => panic!("{:?}", other)
            },
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[]
        },
        &layers
    )
}
