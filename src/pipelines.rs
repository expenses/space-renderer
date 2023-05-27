use crate::reflection;
use spirq::ty::{ScalarType, Type, VectorType};
use std::collections::{BTreeMap, HashSet};

pub enum ShaderSource<'a> {
    Spirv(&'a str),
    Hlsl(&'a str),
}

impl<'a> ShaderSource<'a> {
    fn load(&self, entry_point: &str, profile: &str) -> Vec<u8> {
        match self {
            Self::Spirv(filename) => std::fs::read(filename).unwrap(),
            Self::Hlsl(filename) => {
                let text = std::fs::read_to_string(filename).unwrap();

                match hassle_rs::compile_hlsl(
                    filename,
                    &text,
                    entry_point,
                    profile,
                    &["-spirv", "-HV", "2021", "-WX"],
                    &[],
                ) {
                    Ok(module) => module,
                    Err(error) => {
                        panic!("{}", error);
                    }
                }
            }
        }
    }

    fn as_str(&self) -> &'a str {
        match self {
            Self::Spirv(filename) => filename,
            Self::Hlsl(filename) => filename,
        }
    }
}

fn load_shader_from_bytes(device: &wgpu::Device, bytes: &[u8], raw: bool) -> wgpu::ShaderModule {
    if raw {
        unsafe {
            device.create_shader_module_spirv(&wgpu::ShaderModuleDescriptorSpirV {
                label: None,
                source: wgpu::util::make_spirv_raw(bytes),
            })
        }
    } else {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::util::make_spirv(bytes),
        })
    }
}

pub struct BindGroupLayouts {
    inner: std::collections::BTreeMap<u32, (wgpu::BindGroupLayout, HashSet<u32>)>,
}

impl BindGroupLayouts {
    pub fn new(
        device: &wgpu::Device,
        bindings: &BTreeMap<u32, BTreeMap<u32, wgpu::BindGroupLayoutEntry>>,
    ) -> Self {
        let mut bind_group_layouts = std::collections::BTreeMap::new();

        for (id, entries) in bindings.iter() {
            bind_group_layouts.insert(
                *id,
                (
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: None,
                        entries: &entries.values().cloned().collect::<Vec<_>>(),
                    }),
                    entries
                        .values()
                        .map(|entry| entry.binding)
                        .collect::<HashSet<_>>(),
                ),
            );
        }

        Self {
            inner: bind_group_layouts,
        }
    }

    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        set: u32,
        entries: &mut Vec<wgpu::BindGroupEntry>,
    ) -> wgpu::BindGroup {
        let (layout, binding_ids) = &self.inner[&set];
        entries.retain(|entry| binding_ids.contains(&entry.binding));
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &entries,
        })
    }
}

pub struct ComputePipeline {
    pub bind_group_layouts: BindGroupLayouts,
    pub pipeline: wgpu::ComputePipeline,
}

impl ComputePipeline {
    pub fn new(
        device: &wgpu::Device,
        shader: &ShaderSource,
        entry_point: &str,
        reflection_settings: &reflection::ReflectionSettings,
        raw_spirv: bool,
    ) -> Self {
        assert_eq!(raw_spirv, false);

        let shader_bytes = shader.load(entry_point, "cs_6_0");

        let reflection = reflection::reflect(&shader_bytes, reflection_settings);

        assert_eq!(reflection.entry_points.len(), 1);

        let mut bind_group_layouts = BindGroupLayouts::new(device, &reflection.bindings);

        let bind_group_layout_refs: Vec<_> = bind_group_layouts
            .inner
            .values()
            .map(|(bgl, _)| bgl)
            .collect();

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &bind_group_layout_refs,
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::COMPUTE,
                range: 0..reflection.max_push_constant_size as u32,
            }],
        });

        Self {
            bind_group_layouts,
            pipeline: device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(shader.as_str()),
                layout: Some(&pipeline_layout),
                module: &load_shader_from_bytes(device, &shader_bytes, raw_spirv),
                entry_point,
            }),
        }
    }
}

pub struct RenderPipeline {
    pub bind_group_layouts: BindGroupLayouts,
    pub pipeline: wgpu::RenderPipeline,
}

impl RenderPipeline {
    pub fn new(
        device: &wgpu::Device,
        shader: &ShaderSource,
        vertex_entry_point: &str,
        fragment_entry_point: &str,
        targets: &[Option<wgpu::ColorTargetState>],
        depth_stencil: Option<wgpu::DepthStencilState>,
        vertex_buffer_layouts: &[wgpu::VertexBufferLayout],
        raw_spirv: bool,
    ) -> Self {
        assert_eq!(raw_spirv, false);

        let vertex_shader_bytes = shader.load(vertex_entry_point, "vs_6_0");
        let fragment_shader_bytes = shader.load(fragment_entry_point, "ps_6_0");

        let vertex_reflection = reflection::reflect(&vertex_shader_bytes, &Default::default());

        let fragment_reflection = reflection::reflect(&fragment_shader_bytes, &Default::default());

        let bindings = reflection::merge_bind_group_layout_entries(
            &vertex_reflection.bindings,
            &fragment_reflection.bindings,
        );

        let mut bind_group_layouts = BindGroupLayouts::new(device, &bindings);

        let bind_group_layout_refs: Vec<_> = bind_group_layouts
            .inner
            .values()
            .map(|(bgl, _)| bgl)
            .collect();

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &bind_group_layout_refs,
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                range: 0..vertex_reflection
                    .max_push_constant_size
                    .max(fragment_reflection.max_push_constant_size)
                    as u32,
            }],
        });

        Self {
            bind_group_layouts,
            pipeline: device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&shader.as_str()),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &load_shader_from_bytes(device, &vertex_shader_bytes, raw_spirv),
                    entry_point: vertex_entry_point,
                    buffers: vertex_buffer_layouts,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &load_shader_from_bytes(device, &fragment_shader_bytes, raw_spirv),
                    entry_point: fragment_entry_point,
                    targets,
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            }),
        }
    }
}
