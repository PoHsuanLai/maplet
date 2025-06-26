use crate::Result;
use fxhash::FxHashMap;
use wgpu::{
    Adapter, BindGroupLayout, Buffer, CommandEncoder, Device, Instance, Queue,
    RenderPass, RenderPipeline as WgpuRenderPipeline, SurfaceConfiguration, Texture, TextureView,
};

/// Different types of render passes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderPassType {
    Tile,
    Vector,
    Marker,
    Text,
    UI,
}

/// Configuration for a render pipeline
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub shader_name: String,
    pub vertex_buffer_layout: Vec<wgpu::VertexAttribute>,
    pub blend_state: Option<wgpu::BlendState>,
    pub depth_test: bool,
    pub cull_mode: Option<wgpu::Face>,
}

/// Manages all rendering pipelines and GPU resources
pub struct RenderPipeline {
    pub adapter: Adapter,
    pub device: Device,
    pub queue: Queue,
    /// Optional surface used when rendering to a window. If a surface is not
    /// configured (e.g. when rendering off-screen or during headless tests),
    /// the render pipeline can still be used for resource management.
    pub surface: Option<wgpu::Surface<'static>>,
    pub surface_config: Option<SurfaceConfiguration>,

    // Rendering pipelines for different types
    pipelines: FxHashMap<RenderPassType, WgpuRenderPipeline>,
    bind_group_layouts: FxHashMap<RenderPassType, BindGroupLayout>,

    // Shared resources
    camera_buffer: Buffer,

    // Texture resources
    texture_cache: FxHashMap<String, Texture>,

    pub enabled: bool,
}

impl RenderPipeline {
    /// Create a new render pipeline
    pub async fn new() -> Result<Self> {
        // Create wgpu instance
        let instance = Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
            flags: wgpu::InstanceFlags::default(),
            gles_minor_version: wgpu::Gles3MinorVersion::Automatic,
        });

        // Request adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or("Failed to find an appropriate adapter")?;

        // Request device and queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| format!("Failed to create device: {}", e))?;

        // Create camera buffer
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Buffer"),
            size: 64, // 4x4 matrix = 16 f32s = 64 bytes
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create camera bind group layout
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // Create camera bind group
        let _camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let mut pipeline = Self {
            adapter,
            device,
            queue,
            surface: None,
            surface_config: None,
            pipelines: FxHashMap::default(),
            bind_group_layouts: FxHashMap::default(),
            camera_buffer,
            texture_cache: FxHashMap::default(),
            enabled: true,
        };

        // Initialize default pipelines
        pipeline.init_pipelines()?;

        Ok(pipeline)
    }

    /// Initialize all rendering pipelines
    fn init_pipelines(&mut self) -> Result<()> {
        // Create tile rendering pipeline
        self.create_tile_pipeline()?;

        // Create vector rendering pipeline
        self.create_vector_pipeline()?;

        // Create marker rendering pipeline
        self.create_marker_pipeline()?;

        // Create text rendering pipeline
        self.create_text_pipeline()?;

        Ok(())
    }

    /// Create the tile rendering pipeline
    fn create_tile_pipeline(&mut self) -> Result<()> {
        let tile_shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Tile Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/tile.wgsl").into()),
            });

        let tile_bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Tile Bind Group Layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Tile Pipeline Layout"),
                bind_group_layouts: &[&tile_bind_group_layout],
                push_constant_ranges: &[],
            });

        let tile_pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Tile Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &tile_shader,
                    entry_point: "vs_main",
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 20, // position (8) + texcoord (8) + padding (4)
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 0,
                                shader_location: 0,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 8,
                                shader_location: 1,
                            },
                        ],
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &tile_shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8UnormSrgb,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
            });

        self.pipelines.insert(RenderPassType::Tile, tile_pipeline);
        self.bind_group_layouts
            .insert(RenderPassType::Tile, tile_bind_group_layout);

        Ok(())
    }

    /// Create the vector rendering pipeline
    fn create_vector_pipeline(&mut self) -> Result<()> {
        let vector_shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Vector Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/vector.wgsl").into()),
            });

        let vector_bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Vector Bind Group Layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Vector Pipeline Layout"),
                bind_group_layouts: &[&vector_bind_group_layout],
                push_constant_ranges: &[],
            });

        let vector_pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Vector Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vector_shader,
                    entry_point: "vs_main",
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 16, // position (8) + color (4) + padding (4)
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 0,
                                shader_location: 0,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Unorm8x4,
                                offset: 8,
                                shader_location: 1,
                            },
                        ],
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &vector_shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8UnormSrgb,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
            });

        self.pipelines
            .insert(RenderPassType::Vector, vector_pipeline);
        self.bind_group_layouts
            .insert(RenderPassType::Vector, vector_bind_group_layout);

        Ok(())
    }

    /// Create the marker rendering pipeline
    fn create_marker_pipeline(&mut self) -> Result<()> {
        // For the moment we simply render markers with the vector pipeline, so
        // no dedicated marker pipeline is required.
        Ok(())
    }

    /// Create the text rendering pipeline
    fn create_text_pipeline(&mut self) -> Result<()> {
        // Text rendering will also reuse the vector pipeline for now.
        Ok(())
    }

    /// Configure the surface for rendering
    pub fn configure_surface(
        &mut self,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<()> {
        // In some execution contexts (e.g. CI or unit tests) we may not have an
        // actual display surface. To avoid panics we defensively pick the first
        // available surface format if any, otherwise default to Bgra8Unorm.
        let formats = surface.get_capabilities(&self.adapter).formats;
        let format = *formats.first().unwrap_or(&wgpu::TextureFormat::Bgra8Unorm);

        let config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&self.device, &config);

        self.surface = Some(surface);
        self.surface_config = Some(config);

        Ok(())
    }

    /// Begin a render frame
    pub fn begin_frame(&mut self) -> Result<(CommandEncoder, TextureView)> {
        let surface = self.surface.as_ref().ok_or("Surface not configured")?;

        let output = surface
            .get_current_texture()
            .map_err(|e| format!("Failed to acquire next swap chain texture: {}", e))?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        Ok((encoder, view))
    }

    /// Create a render pass for a specific type
    pub fn create_render_pass<'a>(
        &'a self,
        encoder: &'a mut CommandEncoder,
        view: &'a TextureView,
        pass_type: RenderPassType,
    ) -> Option<RenderPass<'a>> {
        if !self.pipelines.contains_key(&pass_type) {
            return None;
        }

        Some(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(&format!("{:?} Render Pass", pass_type)),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if pass_type == RenderPassType::Tile {
                        wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        })
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        }))
    }

    /// Get a rendering pipeline by type
    pub fn get_pipeline(&self, pass_type: RenderPassType) -> Option<&WgpuRenderPipeline> {
        self.pipelines.get(&pass_type)
    }

    /// Get a bind group layout by type
    pub fn get_bind_group_layout(&self, pass_type: RenderPassType) -> Option<&BindGroupLayout> {
        self.bind_group_layouts.get(&pass_type)
    }

    /// Update camera matrices
    pub fn update_camera(&mut self, view_projection_matrix: &[[f32; 4]; 4]) {
        let matrix_data: &[u8] = bytemuck::cast_slice(view_projection_matrix);
        self.queue.write_buffer(&self.camera_buffer, 0, matrix_data);
    }

    /// Create a vertex buffer
    pub fn create_vertex_buffer(&self, data: &[u8], label: Option<&str>) -> Buffer {
        use wgpu::util::DeviceExt;
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label,
                contents: data,
                usage: wgpu::BufferUsages::VERTEX,
            })
    }

    /// Create an index buffer
    pub fn create_index_buffer(&self, data: &[u8], label: Option<&str>) -> Buffer {
        use wgpu::util::DeviceExt;
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label,
                contents: data,
                usage: wgpu::BufferUsages::INDEX,
            })
    }

    /// Load a texture from bytes
    pub fn load_texture(&mut self, name: String, data: &[u8]) -> Result<()> {
        let img = image::load_from_memory(data)
            .map_err(|e| format!("Failed to load image: {}", e))?
            .to_rgba8();

        let dimensions = img.dimensions();

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&name),
            size: wgpu::Extent3d {
                width: dimensions.0,
                height: dimensions.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
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

        self.texture_cache.insert(name, texture);
        Ok(())
    }

    /// Get a cached texture
    pub fn get_texture(&self, name: &str) -> Option<&Texture> {
        self.texture_cache.get(name)
    }

    /// Finish and submit a frame
    pub fn submit_frame(&mut self, encoder: CommandEncoder) {
        self.queue.submit(std::iter::once(encoder.finish()));

        if let Some(surface) = &self.surface {
            // Present the frame
            if let Ok(output) = surface.get_current_texture() {
                output.present();
            }
        }
    }
}

// Removed Default implementation as it uses unsafe zeroed values
// Use RenderPipeline::new() instead
