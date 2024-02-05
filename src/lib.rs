use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::EventLoop,
    keyboard::{Key, NamedKey},
    window::{Window, WindowBuilder},
};

struct State<'window> {
    pub window: &'window Window,
    surface: wgpu::Surface<'window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    mandelbrot_uniform: MandelbrotUniform,
    mandelbrot_buffer: wgpu::Buffer,
    mandelbrot_bind_group: wgpu::BindGroup,
    cursor_pos: winit::dpi::PhysicalPosition<f64>,
    dragging: bool,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MandelbrotUniform {
    min_x: f64,
    min_y: f64,
    height: f64,
    // width / height, i.e. width = height * aspect_ratio
    aspect_ratio: f64,
    max_iterations: u32,
    _padding: u32,
}

impl<'window> State<'window> {
    // Creating some of the wgpu types requires async code
    async fn new(window: &'window Window) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let surface = instance.create_surface(window).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptionsBase {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::SHADER_F64,
                    #[cfg(not(target_arch = "wasm32"))]
                    required_limits: wgpu::Limits::default(),
                    #[cfg(target_arch = "wasm32")]
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                    label: None,
                },
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        // let mandelbrot_uniform = MandelbrotUniform {
        //     min_x: -0.749488,
        //     min_y: 0.031567533,
        //     height: 0.000141897,
        //     aspect_ratio: size.width as f64 / size.height as f64,
        //     max_iterations: 4096,
        //     _padding: 0,
        // };
        let mandelbrot_uniform = MandelbrotUniform {
            min_x: -2.0,
            min_y: -1.0,
            height: 2.0,
            aspect_ratio: size.width as f64 / size.height as f64,
            max_iterations: 128,
            _padding: 0,
        };

        let mandelbrot_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mandelbrot Buffer"),
            contents: bytemuck::cast_slice(&[mandelbrot_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: mandelbrot_buffer.as_entire_binding(),
            }],
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
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

        Self {
            cursor_pos: winit::dpi::PhysicalPosition::new(0.0, 0.0),
            surface,
            device,
            queue,
            config,
            size,
            window,
            render_pipeline,
            mandelbrot_uniform,
            mandelbrot_buffer,
            mandelbrot_bind_group: bind_group,
            dragging: false,
        }
    }

    fn update_uniform(&mut self) {
        self.queue.write_buffer(
            &self.mandelbrot_buffer,
            0,
            bytemuck::cast_slice(&[self.mandelbrot_uniform]),
        );
        self.window.request_redraw();
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.mandelbrot_uniform.aspect_ratio = new_size.width as f64 / new_size.height as f64;
            self.update_uniform();
        }
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CursorLeft { .. } => {
                self.dragging = false;
                false
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.dragging = *state == ElementState::Pressed;
                false
            }
            WindowEvent::CursorMoved { position, .. } => {
                if self.dragging {
                    let dx = position.x - self.cursor_pos.x;
                    let dy = position.y - self.cursor_pos.y;
                    let MandelbrotUniform {
                        min_x,
                        min_y,
                        height,
                        aspect_ratio,
                        ..
                    } = self.mandelbrot_uniform;
                    self.mandelbrot_uniform.min_x = min_x - dx / self.size.width as f64 * height * aspect_ratio;
                    self.mandelbrot_uniform.min_y = min_y + dy / self.size.height as f64 * height;
                    self.update_uniform();
                }
                self.cursor_pos = *position;
                false
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y as f64,
                    MouseScrollDelta::PixelDelta(pos) => pos.y,
                };
                let scale = 1.0 - delta / 10.0;
                let u = self.cursor_pos.x / self.size.width as f64;
                let v = 1.0 - self.cursor_pos.y / self.size.height as f64;
                let MandelbrotUniform {
                    min_x,
                    min_y,
                    height,
                    aspect_ratio,
                    ..
                } = self.mandelbrot_uniform;
                let new_height = height * scale;
                let height_diff = new_height - height;
                self.mandelbrot_uniform.min_x = min_x - u * height_diff * aspect_ratio;
                self.mandelbrot_uniform.min_y = min_y - v * height_diff;
                self.mandelbrot_uniform.height *= scale;
                self.update_uniform();
                true
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        logical_key: Key::Named(k @ (NamedKey::ArrowUp | NamedKey::ArrowDown)),
                        ..
                    },
                ..
            } => {
                if k == &NamedKey::ArrowUp {
                    self.mandelbrot_uniform.max_iterations += 128;
                } else {
                    self.mandelbrot_uniform.max_iterations = self
                        .mandelbrot_uniform
                        .max_iterations
                        .saturating_sub(128)
                        .max(128);
                };
                dbg!(self.mandelbrot_uniform.max_iterations);
                self.update_uniform();
                true
            }
            _ => false,
        }
    }

    fn update(&mut self) {}

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                timestamp_writes: None,
                occlusion_query_set: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.mandelbrot_bind_group, &[]);
            render_pass.draw(0..4, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

pub async fn run() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_inner_size(winit::dpi::PhysicalSize::new(800, 600))
        .with_title("Mandelbrot Explorer")
        .build(&event_loop)
        .unwrap();

    let mut state = State::new(&window).await;

    event_loop
        .run(move |event, tgt| match event {
            Event::WindowEvent {
                window_id,
                ref event,
                ..
            } if window_id == state.window.id() && !state.input(event) => match event {
                WindowEvent::CloseRequested
                | WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            state: ElementState::Pressed,
                            logical_key: Key::Named(NamedKey::Escape),
                            ..
                        },
                    ..
                } => tgt.exit(),
                WindowEvent::Resized(physical_size) => {
                    state.resize(*physical_size);
                }
                WindowEvent::RedrawRequested => {
                    state.update();
                    match state.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                        Err(wgpu::SurfaceError::OutOfMemory) => tgt.exit(),
                        Err(e) => eprintln!("{:?}", e),
                    }
                }
                _ => {}
            },
            _ => (),
        })
        .unwrap();
}
