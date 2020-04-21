pub mod defaults;

use include_dir::{include_dir, Dir};
const GEN_DIR: Dir = include_dir!("gen");
// We use `memoffset::offset_of` to get the offsets of all of these fields... do we need C representation?
#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct Vertex {
    pub position: [f32; 4],
    pub color: [f32; 4],
    pub size: f32,
}
pub struct Renderer {
    pub surface: wgpu::Surface,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub sc_desc: wgpu::SwapChainDescriptor,
    pub swap_chain: wgpu::SwapChain,
    pub camera_uniform_buffer: wgpu::Buffer,
    pub camera: OrbitCamera,
    pub uniforms_bind_group_layout: wgpu::BindGroupLayout,
    pub render_pipeline: wgpu::RenderPipeline,
    pub depth_texture: wgpu::Texture,
    pub depth_texture_view: wgpu::TextureView,
}

#[derive(Debug, Copy, Clone)]
pub struct OrbitCamera {
    aspect: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,
    target: nalgebra::Point3<f32>,
    range: f32,
    azimuth: f32,
    elevation: f32,
}

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct CameraUniform {
    camera_pos: [f32; 4],
    view_proj: [[f32; 4]; 4],
}

pub trait Camera {
    fn generate_uniform(&self) -> CameraUniform;
}

impl OrbitCamera {
    pub fn default(aspect: f32) -> Self {
        OrbitCamera {
            aspect: aspect,
            fovy: 45.0 * 180.0 * 3.1415,
            znear: 0.1,
            zfar: 100.0,

            // Look at the origin...
            target: nalgebra::Point3::new(0.0, 0.0, 0.0),
            // ... from 10 units away...
            range: 10.0,
            // ... from a 45/45 degree perspective
            azimuth: 45.0_f32.to_radians(),
            elevation: 45.0_f32.to_radians(),
        }
    }

    pub fn set_aspect(&mut self, aspect: f32) {
        self.aspect = aspect;
    }

    pub fn move_longitudinally(&mut self, delta: f32) {
        self.range = self.range * (0.75_f32).powf(delta);
    }

    pub fn move_on_orbit(&mut self, delta: nalgebra::Vector2<f32>) {
        self.azimuth -= delta[0] * 0.01;
        self.elevation += delta[1] * 0.01;

        // Clamp elevation
        // TODO: Use this: https://github.com/rust-lang/rust/issues/44095
        self.elevation = match self.elevation {
            d if d >= 90_f32.to_radians() => 90_f32.to_radians(),
            d if d <= -270_f32.to_radians() => -270_f32.to_radians(),
            _ => self.elevation,
        };

        // Wrap azimuth
        self.azimuth = self.azimuth % 360_f32.to_radians();
    }

    pub fn move_focus(&mut self, delta: nalgebra::Vector2<f32>) {
        #[rustfmt::skip]
        let transform = nalgebra::Matrix3x2::new(
            -self.azimuth.sin(),  self.azimuth.cos(),
            self.azimuth.cos(), self.azimuth.sin(),
            0.0,                 0.0
        );
        let world_space_delta = transform * delta * 0.1;
        self.target -= world_space_delta;
    }
}

fn cartesian_from_polar<T: nalgebra::base::Scalar + num_traits::real::Real>(
    range: T,
    azimuth: T,
    elevation: T,
) -> nalgebra::Vector3<T> {
    let r_cos_el = range * elevation.cos();
    let x = r_cos_el * azimuth.cos();
    let y = r_cos_el * azimuth.sin();
    let z = range * elevation.sin();
    return nalgebra::Vector3::<T>::new(x, y, z);
}

impl Camera for OrbitCamera {
    fn generate_uniform(&self) -> CameraUniform {
        let delta = 0.01;
        let eye = self.target + cartesian_from_polar(self.range, self.azimuth, self.elevation);
        let up = self.target
            + cartesian_from_polar(self.range, self.azimuth, self.elevation + delta)
            - eye;
        let view = nalgebra::Isometry3::look_at_rh(&eye, &self.target, &up);
        let projection = nalgebra::Perspective3::new(self.aspect, self.fovy, self.znear, self.zfar);

        // https://matthewwellings.com/blog/the-new-vulkan-coordinate-system/
        // TODO: Check if this means that every platform will need a different transform matrix?
        #[rustfmt::skip]
        let opengl_to_wgpu_matrix = nalgebra::Matrix4::<f32>::new(
            -1.0,  0.0, 0.0, 0.0,
            0.0,  -1.0, 0.0, 0.0,
            0.0,   0.0, 0.5, 0.0,
            0.0,   0.0, 0.5, 1.0,
        );

        CameraUniform {
            camera_pos: *eye.to_homogeneous().as_ref(),
            view_proj: *(opengl_to_wgpu_matrix * projection.as_matrix() * view.to_homogeneous())
                .as_ref(),
        }
    }
}

impl Renderer {
    pub fn new(surface: wgpu::Surface, size: winit::dpi::PhysicalSize<u32>) -> Self {
        let adapter = futures::executor::block_on(wgpu::Adapter::request(
            &wgpu::RequestAdapterOptions {
                // TODO: Make this configurable
                power_preference: wgpu::PowerPreference::Default,
                compatible_surface: Some(&surface),
            },
            wgpu::BackendBit::PRIMARY,
        ))
        .unwrap();

        let (device, queue) =
            futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                extensions: wgpu::Extensions {
                    anisotropic_filtering: true,
                },
                limits: wgpu::Limits::default(),
            }));

        let sc_desc = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Mailbox,
        };

        let swap_chain = device.create_swap_chain(&surface, &sc_desc);

        let camera = OrbitCamera::default(size.width as f32 / size.height as f32);

        let camera_uniform_buffer = device.create_buffer_with_data(
            u8_slice_from_slice(std::slice::from_ref(&camera.generate_uniform())),
            wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        );

        let uniforms_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                bindings: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX,
                    ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                }],
                label: None,
            });

        let vs_bytes = GEN_DIR
            .get_file("shaders/shader.vert.spv")
            .unwrap()
            .contents();
        let vs_module = device
            .create_shader_module(&wgpu::read_spirv(std::io::Cursor::new(&vs_bytes[..])).unwrap());

        let fs_bytes = GEN_DIR
            .get_file("shaders/shader.frag.spv")
            .unwrap()
            .contents();
        let fs_module = device
            .create_shader_module(&wgpu::read_spirv(std::io::Cursor::new(&fs_bytes[..])).unwrap());
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[&uniforms_bind_group_layout],
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: &pipeline_layout,
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: &vs_module,
                entry_point: "main",
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                module: &fs_module,
                entry_point: "main",
            }),
            rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::None,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            }),
            primitive_topology: wgpu::PrimitiveTopology::PointList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: Some(wgpu::DepthStencilStateDescriptor {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil_front: wgpu::StencilStateFaceDescriptor::IGNORE,
                stencil_back: wgpu::StencilStateFaceDescriptor::IGNORE,
                stencil_read_mask: 0,
                stencil_write_mask: 0,
            }),
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint32,
                vertex_buffers: &[wgpu::VertexBufferDescriptor {
                    stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttributeDescriptor {
                            format: wgpu::VertexFormat::Float4,
                            offset: memoffset::offset_of!(Vertex, position) as wgpu::BufferAddress,
                            shader_location: 0,
                        },
                        wgpu::VertexAttributeDescriptor {
                            format: wgpu::VertexFormat::Float4,
                            offset: memoffset::offset_of!(Vertex, color) as wgpu::BufferAddress,
                            shader_location: 1,
                        },
                        wgpu::VertexAttributeDescriptor {
                            format: wgpu::VertexFormat::Float4,
                            offset: memoffset::offset_of!(Vertex, size) as wgpu::BufferAddress,
                            shader_location: 2,
                        },
                    ],
                }],
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            size: wgpu::Extent3d {
                width: sc_desc.width,
                height: sc_desc.height,
                depth: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            label: None,
            array_layer_count: 1,
        });

        let depth_texture_view = depth_texture.create_default_view();

        Self {
            surface: surface,
            adapter: adapter,
            device: device,
            queue: queue,
            sc_desc: sc_desc,
            swap_chain: swap_chain,
            camera: camera,
            camera_uniform_buffer: camera_uniform_buffer,
            uniforms_bind_group_layout: uniforms_bind_group_layout,
            render_pipeline: render_pipeline,
            depth_texture: depth_texture,
            depth_texture_view: depth_texture_view,
        }
    }

    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.sc_desc.width = size.width;
        self.sc_desc.height = size.height;
        self.camera
            .set_aspect(size.width as f32 / size.height as f32);
        self.swap_chain = self.device.create_swap_chain(&self.surface, &self.sc_desc);
        self.depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            size: wgpu::Extent3d {
                width: self.sc_desc.width,
                height: self.sc_desc.height,
                depth: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            label: None,
            array_layer_count: 1,
        });

        self.depth_texture_view = self.depth_texture.create_default_view();
    }

    pub fn render(
        &self,
        command_encoder: &mut wgpu::CommandEncoder,
        texture_view: &wgpu::TextureView,
        vertices: &Vec<Vertex>,
        indices: &Vec<u32>,
        first_pass: bool
    ) {
        // It might be expensive to copy these buffers every call?
        let vertex_buffer = self.device.create_buffer_with_data(
            u8_slice_from_slice(vertices.as_slice()),
            wgpu::BufferUsage::VERTEX,
        );

        let index_buffer = self.device.create_buffer_with_data(
            u8_slice_from_slice(indices.as_slice()),
            wgpu::BufferUsage::INDEX,
        );
        let camera_uniform_buffer = self.device.create_buffer_with_data(
            u8_slice_from_slice(std::slice::from_ref(&self.camera.generate_uniform())),
            wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_SRC,
        );
        let uniforms_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.uniforms_bind_group_layout,
            bindings: &[wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &camera_uniform_buffer,
                    range: 0..std::mem::size_of::<CameraUniform>() as wgpu::BufferAddress,
                },
            }],
            label: None,
        });
        {
            let mut render_pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &texture_view,
                    resolve_target: None,
                    load_op: if first_pass {wgpu::LoadOp::Clear} else {wgpu::LoadOp::Load},
                    store_op: wgpu::StoreOp::Store,
                    clear_color: wgpu::Color::TRANSPARENT,
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
                    attachment: &self.depth_texture_view,
                    depth_load_op: if first_pass {wgpu::LoadOp::Clear} else {wgpu::LoadOp::Load},
                    depth_store_op: wgpu::StoreOp::Store,
                    clear_depth: 1.0,
                    stencil_load_op: if first_pass {wgpu::LoadOp::Clear} else {wgpu::LoadOp::Load},
                    stencil_store_op: wgpu::StoreOp::Store,
                    clear_stencil: 0,
                }),
            });
            render_pass.set_pipeline(&self.render_pipeline);

            render_pass.set_bind_group(0, &uniforms_bind_group, &[]);
            render_pass.set_index_buffer(&index_buffer, 0, 0);
            render_pass.set_vertex_buffer(0, &vertex_buffer, 0, 0);
            render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }
    }
}

fn u8_slice_from_slice<T>(data: &[T]) -> &[u8] {
    let slice = unsafe {
        std::slice::from_raw_parts(
            data.as_ptr() as *const u8,
            data.len() * std::mem::size_of::<T>(),
        )
    };
    return slice;
}
