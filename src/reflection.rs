use std::collections::{BTreeMap, HashMap};

fn map_dim(dim: spirv::Dim) -> wgpu::TextureViewDimension {
    match dim {
        spirv::Dim::Dim2D => wgpu::TextureViewDimension::D2,
        spirv::Dim::Dim3D => wgpu::TextureViewDimension::D3,
        spirv::Dim::DimCube => wgpu::TextureViewDimension::Cube,
        other => panic!("{:?}", other),
    }
}

#[derive(Clone)]
pub struct ReflectionSettings {
    pub sampled_texture_sample_type: wgpu::TextureSampleType
}

impl Default for ReflectionSettings {
    fn default() -> Self {
        Self {
            sampled_texture_sample_type: wgpu::TextureSampleType::Float { filterable: true }
        }
    }
}

pub struct Reflection {
    pub bindings: BTreeMap<u32, BTreeMap<u32, wgpu::BindGroupLayoutEntry>>,
    pub entry_points: Vec<spirq::EntryPoint>,
    pub max_push_constant_size: usize,
}

impl Reflection {
    
}

pub fn reflect(bytes: &[u8], settings: &ReflectionSettings) -> Reflection {
    let entry_points = spirq::ReflectConfig::new().spv(bytes).reflect().unwrap();

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
            } =>
            {
                let set_bindings = bindings.entry(desc_bind.set()).or_default();
                let binding = set_bindings.entry(desc_bind.bind()).or_insert_with(|| {
                    wgpu::BindGroupLayoutEntry {
                        binding: desc_bind.bind(),
                        visibility: shader_stage,
                        ty: match (ty, desc_ty) {
                            (spirq::ty::Type::SampledImage(ty), spirq::reflect::DescriptorType::SampledImage()) => wgpu::BindingType::Texture {
                                multisampled: ty.is_multisampled,
                                sample_type: match &ty.scalar_ty {
                                    spirq::ty::ScalarType::Float(4) => {
                                        let ty = settings.sampled_texture_sample_type;
                                        settings = Default::default();
                                        ty
                                    },
                                    other => panic!("{:?}", other),
                                },
                                view_dimension: map_dim(ty.dim),
                            },
                            (spirq::ty::Type::StorageImage(ty), spirq::reflect::DescriptorType::StorageImage(access)) => {
                                wgpu::BindingType::StorageTexture {
                                    view_dimension: map_dim(ty.dim),
                                    format: match ty.fmt {
                                        spirq::ty::ImageFormat::Rgba16f => wgpu::TextureFormat::Rgba16Float,
                                        spirq::ty::ImageFormat::R16f => wgpu::TextureFormat::R16Float,
                                        other => panic!("{:?}", other),
                                    },
                                    access: match access {
                                        spirq::reflect::AccessType::ReadWrite => wgpu::StorageTextureAccess::ReadWrite,
                                        other => panic!("{:?}", other),
                                    },
                                }
                            }
                            (spirq::ty::Type::Sampler(), spirq::reflect::DescriptorType::Sampler()) => {
                                wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
                            }
                            other => panic!("{:?}", other),
                        },
                        count: if *nbind > 1 {
                            Some(std::num::NonZeroU32::new(*nbind).unwrap())
                        } else {
                            None
                        },
                    }
                });

                binding.visibility |= shader_stage;
            },
            spirq::reflect::Variable::PushConstant {
                ty,
                ..
            } => {
                max_push_constant_size = max_push_constant_size.max(ty.nbyte().unwrap_or(0));
            },
            _ => {}
        }
        }
    }

    Reflection {
        bindings,
        entry_points,
        max_push_constant_size
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
