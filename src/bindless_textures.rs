use std::ops::Range;

pub struct BindlessTextures {
    views: Vec<wgpu::TextureView>,
}

impl BindlessTextures {
    pub fn new() -> Self {
        Self { views: Vec::new() }
    }

    pub fn push(&mut self, mut views: Vec<wgpu::TextureView>) -> Range<u32> {
        let start = self.views.len() as u32;
        self.views.extend(views.drain(..));
        let end = self.views.len() as u32;
        start..end
    }

    pub fn texture_view_array(&self) -> Vec<&wgpu::TextureView> {
        self.views.iter().collect()
    }
}
