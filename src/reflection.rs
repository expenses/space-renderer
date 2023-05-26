use spirq::ty::ImageFormat;
use std::collections::{BTreeMap, HashMap};

fn map_dim(dim: spirv::Dim, is_array: bool) -> wgpu::TextureViewDimension {
    match (dim, is_array) {
        (spirv::Dim::Dim2D, true) => wgpu::TextureViewDimension::D2Array,
        (spirv::Dim::Dim2D, _) => wgpu::TextureViewDimension::D2,
        (spirv::Dim::Dim3D, _) => wgpu::TextureViewDimension::D3,
        (spirv::Dim::DimCube, _) => wgpu::TextureViewDimension::Cube,
        other => panic!("{:?}", other),
    }
}

#[derive(Clone)]
pub struct ReflectionSettings {
    pub override_sampled_texture_ty: Option<(u32, wgpu::TextureSampleType)>,
}

impl Default for ReflectionSettings {
    fn default() -> Self {
        Self {
            override_sampled_texture_ty: None,
        }
    }
}

pub struct Reflection {
    pub bindings: BTreeMap<u32, BTreeMap<u32, wgpu::BindGroupLayoutEntry>>,
    pub entry_points: Vec<spirq::EntryPoint>,
    pub max_push_constant_size: usize,
}

impl Reflection {}

pub fn reflect(bytes: &[u8], settings: &ReflectionSettings) -> Reflection {
    let entry_points = spirq::ReflectConfig::new()
        .ref_all_rscs(true)
        .spv(bytes)
        .reflect()
        .unwrap();

    let mut settings = settings.clone();

    let mut bindings: BTreeMap<u32, BTreeMap<u32, wgpu::BindGroupLayoutEntry>> = BTreeMap::new();
    let mut max_push_constant_size = 0;

    for entry_point in &entry_points {
        let shader_stage = match entry_point.exec_model {
            spirq::ExecutionModel::Vertex => wgpu::ShaderStages::VERTEX,
            spirq::ExecutionModel::Fragment => wgpu::ShaderStages::FRAGMENT,
            spirq::ExecutionModel::GLCompute => wgpu::ShaderStages::COMPUTE,
            other => panic!("{:?}", other),
        };

        for var in &entry_point.vars {
            match &var {
                spirq::reflect::Variable::Descriptor {
                    desc_bind,
                    desc_ty,
                    ty,
                    nbind,
                    ..
                } => {
                    let set_bindings = bindings.entry(desc_bind.set()).or_default();
                    let binding = set_bindings.entry(desc_bind.bind()).or_insert_with(|| {
                        wgpu::BindGroupLayoutEntry {
                            binding: desc_bind.bind(),
                            visibility: shader_stage,
                            ty: match (ty, desc_ty) {
                                (
                                    spirq::ty::Type::SampledImage(ty),
                                    spirq::reflect::DescriptorType::SampledImage(),
                                ) => wgpu::BindingType::Texture {
                                    multisampled: ty.is_multisampled,
                                    sample_type: match &ty.scalar_ty {
                                        spirq::ty::ScalarType::Float(4) => match settings
                                            .override_sampled_texture_ty
                                        {
                                            Some((binding, ty)) if binding == desc_bind.bind() => {
                                                settings.override_sampled_texture_ty = None;
                                                ty
                                            }
                                            _ => {
                                                wgpu::TextureSampleType::Float { filterable: true }
                                            }
                                        },
                                        other => panic!("{:?}", other),
                                    },
                                    view_dimension: map_dim(ty.dim, ty.is_array),
                                },
                                (
                                    spirq::ty::Type::StorageImage(ty),
                                    spirq::reflect::DescriptorType::StorageImage(access),
                                ) => wgpu::BindingType::StorageTexture {
                                    view_dimension: map_dim(ty.dim, ty.is_array),
                                    format: match ty.fmt {
                                        ImageFormat::Rgba16f => wgpu::TextureFormat::Rgba16Float,
                                        ImageFormat::R16f => wgpu::TextureFormat::R16Float,
                                        ImageFormat::Rg16f => wgpu::TextureFormat::Rg16Float,
                                        ImageFormat::Rgba32f => wgpu::TextureFormat::Rgba32Float,
                                        other => panic!("{:?}", other),
                                    },
                                    access: match access {
                                        spirq::reflect::AccessType::ReadWrite => {
                                            wgpu::StorageTextureAccess::ReadWrite
                                        }
                                        other => panic!("{:?}", other),
                                    },
                                },
                                (
                                    spirq::ty::Type::Sampler(),
                                    spirq::reflect::DescriptorType::Sampler(),
                                ) => {
                                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
                                }
                                (
                                    spirq::ty::Type::Struct(ty),
                                    spirq::reflect::DescriptorType::UniformBuffer(),
                                ) => wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Uniform,
                                    has_dynamic_offset: false,
                                    min_binding_size: Some(
                                        std::num::NonZeroU64::new(ty.nbyte() as u64).unwrap(),
                                    ),
                                },
                                (
                                    spirq::ty::Type::Struct(ty),
                                    spirq::reflect::DescriptorType::StorageBuffer(access),
                                ) => wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage {
                                        read_only: *access == spirq::reflect::AccessType::ReadOnly,
                                    },
                                    has_dynamic_offset: false,
                                    min_binding_size: std::num::NonZeroU64::new(ty.nbyte() as u64),
                                },
                                other => panic!("{:?}", other),
                            },
                            count: match *nbind {
                                0 => Some(std::num::NonZeroU32::new(4096).unwrap()),
                                1 => None,
                                other => Some(std::num::NonZeroU32::new(other).unwrap()),
                            },
                        }
                    });

                    binding.visibility |= shader_stage;
                }
                spirq::reflect::Variable::PushConstant { ty, .. } => {
                    max_push_constant_size = max_push_constant_size.max(ty.nbyte().unwrap_or(0));
                }
                _ => {}
            }
        }
    }

    Reflection {
        bindings,
        entry_points,
        max_push_constant_size,
    }
}
pub fn merge_bind_group_layout_entries(
    a: &BTreeMap<u32, BTreeMap<u32, wgpu::BindGroupLayoutEntry>>,
    b: &BTreeMap<u32, BTreeMap<u32, wgpu::BindGroupLayoutEntry>>,
) -> BTreeMap<u32, BTreeMap<u32, wgpu::BindGroupLayoutEntry>> {
    let mut output = a.clone();

    for (location, entries) in b {
        let merged = output.entry(*location).or_default();

        for (binding, merging_entry) in entries {
            let mut entry = merged.entry(*binding).or_insert_with(|| *merging_entry);
            entry.visibility |= merging_entry.visibility;
        }
    }

    output
}
