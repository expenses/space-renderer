use crate::reflection;

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

                hassle_rs::compile_hlsl(filename, &text, entry_point, profile, &["-spirv"], &[])
                    .unwrap()
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

pub struct ComputePipeline {
    pub bind_group_layouts: std::collections::BTreeMap<u32, wgpu::BindGroupLayout>,
    pub pipeline: wgpu::ComputePipeline,
}

impl ComputePipeline {
    pub fn new(device: &wgpu::Device, shader: &ShaderSource, entry_point: &str, reflection_settings: &reflection::ReflectionSettings) -> Self {
        let shader_bytes = shader.load(entry_point, "cs_6_0");

        let reflection = reflection::reflect(&shader_bytes, reflection_settings);

        assert_eq!(reflection.entry_points.len(), 1);

        let mut bind_group_layouts = std::collections::BTreeMap::new();

        for (id, entries) in reflection.bindings.iter() {
            bind_group_layouts.insert(
                *id,
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &entries.values().cloned().collect::<Vec<_>>(),
                }),
            );
        }

        let bind_group_layout_refs: Vec<_> = bind_group_layouts.values().collect();

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
                module: &unsafe {device.create_shader_module_spirv(&wgpu::ShaderModuleDescriptorSpirV {
                    label: None,
                    source: wgpu::util::make_spirv_raw(&shader_bytes),
                })},
                entry_point,
            }),
        }
    }
}

pub struct RenderPipeline {
    pub bind_group_layouts: std::collections::BTreeMap<u32, wgpu::BindGroupLayout>,
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
    ) -> Self {
        let vertex_shader_bytes = shader.load(vertex_entry_point, "vs_6_0");
        let fragment_shader_bytes = shader.load(fragment_entry_point, "ps_6_0");

        let vertex_reflection = 
        reflection::reflect(&vertex_shader_bytes, &Default::default());

        let mut attrs = Vec::new();
        let mut offset = 0;

        for var in &vertex_reflection.entry_points[0].vars {
            if let spirq::reflect::Variable::Input {location, ty, ..} = var {
                assert_eq!(location.loc() as usize, attrs.len());
                assert_eq!(location.comp(), 0);
                let format = match ty {
                    spirq::ty::Type::Vector(spirq::ty::VectorType { scalar_ty: spirq::ty::ScalarType::Float(4), nscalar: 3}) => wgpu::VertexFormat::Float32x3,
                    spirq::ty::Type::Vector(spirq::ty::VectorType { scalar_ty: spirq::ty::ScalarType::Float(4), nscalar: 2}) => wgpu::VertexFormat::Float32x2,
                    other => panic!("{:?}", other),
                };
                attrs.push(wgpu::VertexAttribute {
                    shader_location: location.loc(),
                    offset: 0,
                    format  
                });
                offset += format.size();
            }
        }

        let mut vertex_buffer_layouts = Vec::new();

        if offset != 0 {
            for attr in &attrs {
                vertex_buffer_layouts.push(wgpu::VertexBufferLayout {
                    array_stride: attr.format.size(),
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: std::slice::from_ref(attr)
                });
            }
            
        }

        let fragment_reflection = 
        reflection::reflect(&fragment_shader_bytes, &Default::default());

        let bindings = reflection::merge_bind_group_layout_entries(&vertex_reflection.bindings, &fragment_reflection.bindings);
        
        let mut bind_group_layouts = std::collections::BTreeMap::new();

        for (id, entries) in bindings.iter() {
            bind_group_layouts.insert(
                *id,
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &entries.values().cloned().collect::<Vec<_>>(),
                }),
            );
        }

        let bind_group_layout_refs: Vec<_> = bind_group_layouts.values().collect();

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &bind_group_layout_refs,
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                range: 0..vertex_reflection.max_push_constant_size.max(fragment_reflection.max_push_constant_size) as u32,
            }],
        });

        Self {
            bind_group_layouts,
            pipeline: device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&shader.as_str()),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &unsafe {device.create_shader_module_spirv(&wgpu::ShaderModuleDescriptorSpirV {
                        label: None,
                        source: wgpu::util::make_spirv_raw(&vertex_shader_bytes),
                    })},
                    entry_point: vertex_entry_point,
                    buffers: &vertex_buffer_layouts,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &unsafe {device.create_shader_module_spirv(&wgpu::ShaderModuleDescriptorSpirV {
                        label: None,
                        source: wgpu::util::make_spirv_raw(&fragment_shader_bytes),
                    })},
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
