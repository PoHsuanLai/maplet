use crate::Result;
use std::collections::HashMap;
use wgpu::{BindGroup, BindGroupLayout, Buffer, Device, Queue, Sampler, Texture, TextureView};

/// Manages GPU resources and provides caching
pub struct Resources {
    /// Device reference for creating resources
    device: Option<Device>,

    /// Queue reference for uploading data
    queue: Option<Queue>,

    /// Texture cache
    textures: HashMap<String, Texture>,

    /// Texture view cache
    texture_views: HashMap<String, TextureView>,

    /// Buffer cache
    buffers: HashMap<String, Buffer>,

    /// Bind group cache
    bind_groups: HashMap<String, BindGroup>,

    /// Sampler cache
    samplers: HashMap<String, Sampler>,

    /// Bind group layout cache
    bind_group_layouts: HashMap<String, BindGroupLayout>,
}

impl Resources {
    /// Create a new resource manager
    pub fn new() -> Self {
        Self {
            device: None,
            queue: None,
            textures: HashMap::new(),
            texture_views: HashMap::new(),
            buffers: HashMap::new(),
            bind_groups: HashMap::new(),
            samplers: HashMap::new(),
            bind_group_layouts: HashMap::new(),
        }
    }

    /// Initialize with device and queue
    pub fn init(&mut self, device: Device, queue: Queue) {
        self.device = Some(device);
        self.queue = Some(queue);
    }

    /// Get device reference
    pub fn device(&self) -> Option<&Device> {
        self.device.as_ref()
    }

    /// Get queue reference
    pub fn queue(&self) -> Option<&Queue> {
        self.queue.as_ref()
    }

    /// Create and cache a texture
    pub fn create_texture(
        &mut self,
        name: String,
        descriptor: &wgpu::TextureDescriptor,
    ) -> Result<&Texture> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;

        let texture = device.create_texture(descriptor);
        self.textures.insert(name.clone(), texture);

        Ok(self.textures.get(&name).unwrap())
    }

    /// Load texture from image data
    pub fn load_texture_from_bytes(
        &mut self,
        name: String,
        data: &[u8],
        format: wgpu::TextureFormat,
    ) -> Result<&Texture> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        let queue = self.queue.as_ref().ok_or("Queue not initialized")?;

        // Load image using the image crate
        let img = image::load_from_memory(data)
            .map_err(|e| format!("Failed to load image: {}", e))?
            .to_rgba8();

        let dimensions = img.dimensions();

        // Create texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&name),
            size: wgpu::Extent3d {
                width: dimensions.0,
                height: dimensions.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload image data to texture
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &img,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            wgpu::Extent3d {
                width: dimensions.0,
                height: dimensions.1,
                depth_or_array_layers: 1,
            },
        );

        self.textures.insert(name.clone(), texture);
        Ok(self.textures.get(&name).unwrap())
    }

    /// Get a cached texture
    pub fn get_texture(&self, name: &str) -> Option<&Texture> {
        self.textures.get(name)
    }

    /// Create and cache a texture view
    pub fn create_texture_view(
        &mut self,
        name: String,
        texture: &Texture,
        descriptor: Option<&wgpu::TextureViewDescriptor>,
    ) -> &TextureView {
        let view =
            texture.create_view(descriptor.unwrap_or(&wgpu::TextureViewDescriptor::default()));
        self.texture_views.insert(name.clone(), view);
        self.texture_views.get(&name).unwrap()
    }

    /// Get a cached texture view
    pub fn get_texture_view(&self, name: &str) -> Option<&TextureView> {
        self.texture_views.get(name)
    }

    /// Create and cache a buffer
    pub fn create_buffer(
        &mut self,
        name: String,
        descriptor: &wgpu::BufferDescriptor,
    ) -> Result<&Buffer> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;

        let buffer = device.create_buffer(descriptor);
        self.buffers.insert(name.clone(), buffer);

        Ok(self.buffers.get(&name).unwrap())
    }

    /// Create buffer with initial data
    pub fn create_buffer_with_data(
        &mut self,
        name: String,
        data: &[u8],
        usage: wgpu::BufferUsages,
    ) -> Result<&Buffer> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;

        use wgpu::util::DeviceExt;
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&name),
            contents: data,
            usage,
        });

        self.buffers.insert(name.clone(), buffer);
        Ok(self.buffers.get(&name).unwrap())
    }

    /// Get a cached buffer
    pub fn get_buffer(&self, name: &str) -> Option<&Buffer> {
        self.buffers.get(name)
    }

    /// Update buffer data
    pub fn update_buffer(&self, name: &str, offset: u64, data: &[u8]) -> Result<()> {
        let queue = self.queue.as_ref().ok_or("Queue not initialized")?;
        let buffer = self.buffers.get(name).ok_or("Buffer not found")?;

        queue.write_buffer(buffer, offset, data);
        Ok(())
    }

    /// Create and cache a sampler
    pub fn create_sampler(
        &mut self,
        name: String,
        descriptor: &wgpu::SamplerDescriptor,
    ) -> Result<&Sampler> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;

        let sampler = device.create_sampler(descriptor);
        self.samplers.insert(name.clone(), sampler);

        Ok(self.samplers.get(&name).unwrap())
    }

    /// Get a cached sampler
    pub fn get_sampler(&self, name: &str) -> Option<&Sampler> {
        self.samplers.get(name)
    }

    /// Create and cache a bind group layout
    pub fn create_bind_group_layout(
        &mut self,
        name: String,
        descriptor: &wgpu::BindGroupLayoutDescriptor,
    ) -> Result<&BindGroupLayout> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;

        let layout = device.create_bind_group_layout(descriptor);
        self.bind_group_layouts.insert(name.clone(), layout);

        Ok(self.bind_group_layouts.get(&name).unwrap())
    }

    /// Get a cached bind group layout
    pub fn get_bind_group_layout(&self, name: &str) -> Option<&BindGroupLayout> {
        self.bind_group_layouts.get(name)
    }

    /// Create and cache a bind group
    pub fn create_bind_group(
        &mut self,
        name: String,
        descriptor: &wgpu::BindGroupDescriptor,
    ) -> Result<&BindGroup> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;

        let bind_group = device.create_bind_group(descriptor);
        self.bind_groups.insert(name.clone(), bind_group);

        Ok(self.bind_groups.get(&name).unwrap())
    }

    /// Get a cached bind group
    pub fn get_bind_group(&self, name: &str) -> Option<&BindGroup> {
        self.bind_groups.get(name)
    }

    /// Create common samplers
    pub fn create_default_samplers(&mut self) -> Result<()> {
        // Linear filtering sampler
        self.create_sampler(
            "linear".to_string(),
            &wgpu::SamplerDescriptor {
                label: Some("Linear Sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                lod_min_clamp: 0.0,
                lod_max_clamp: f32::MAX,
                compare: None,
                anisotropy_clamp: 1,
                border_color: None,
            },
        )?;

        // Nearest filtering sampler
        self.create_sampler(
            "nearest".to_string(),
            &wgpu::SamplerDescriptor {
                label: Some("Nearest Sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                lod_min_clamp: 0.0,
                lod_max_clamp: f32::MAX,
                compare: None,
                anisotropy_clamp: 1,
                border_color: None,
            },
        )?;

        Ok(())
    }

    /// Clear all cached resources
    pub fn clear_cache(&mut self) {
        self.textures.clear();
        self.texture_views.clear();
        self.buffers.clear();
        self.bind_groups.clear();
        self.samplers.clear();
        self.bind_group_layouts.clear();
    }

    /// Get memory usage statistics
    pub fn get_stats(&self) -> ResourceStats {
        ResourceStats {
            texture_count: self.textures.len(),
            texture_view_count: self.texture_views.len(),
            buffer_count: self.buffers.len(),
            bind_group_count: self.bind_groups.len(),
            sampler_count: self.samplers.len(),
            bind_group_layout_count: self.bind_group_layouts.len(),
        }
    }
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}

/// Resource usage statistics
#[derive(Debug, Clone)]
pub struct ResourceStats {
    pub texture_count: usize,
    pub texture_view_count: usize,
    pub buffer_count: usize,
    pub bind_group_count: usize,
    pub sampler_count: usize,
    pub bind_group_layout_count: usize,
}

impl ResourceStats {
    /// Total number of cached resources
    pub fn total_resources(&self) -> usize {
        self.texture_count
            + self.texture_view_count
            + self.buffer_count
            + self.bind_group_count
            + self.sampler_count
            + self.bind_group_layout_count
    }
}
