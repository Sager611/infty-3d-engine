#![deny(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo)]

use bytemuck::{Pod, Zeroable};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use wgpu::util::DeviceExt;

use crate::math;

const MAX_WIDTH: u16 = 1_440;
const MAX_HEIGHT: u16 = 1_000;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    view: [f32; 4],
    screen: [f32; 4],
}

/// WebGPU renderer for the analytic scene.
#[wasm_bindgen]
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    compute_pipeline: wgpu::ComputePipeline,
    present_pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    output: wgpu::Texture,
    compute_bind_group: wgpu::BindGroup,
    present_bind_group: wgpu::BindGroup,
    width: u16,
    height: u16,
    view: [f64; 4],
}

#[wasm_bindgen]
impl Renderer {
    /// Creates a renderer attached to an owned HTML canvas.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error if WebGPU cannot create the surface, adapter,
    /// device, configuration, or shader pipelines.
    /// Browser WebGPU futures are local to the browser event loop.
    #[allow(clippy::future_not_send, reason = "browser WebGPU futures are local")]
    pub async fn create(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        let width = u16::try_from(canvas.width().clamp(1, u32::from(MAX_WIDTH)))
            .map_err(|_| js_error("canvas width is out of range"))?;
        let height = u16::try_from(canvas.height().clamp(1, u32::from(MAX_HEIGHT)))
            .map_err(|_| js_error("canvas height is out of range"))?;
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|error| js_error(format!("surface creation failed: {error}")))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|error| js_error(format!("adapter request failed: {error}")))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|error| js_error(format!("device request failed: {error}")))?;
        let config = surface
            .get_default_config(&adapter, u32::from(width), u32::from(height))
            .ok_or_else(|| js_error("surface has no supported configuration"))?;
        surface.configure(&device, &config);

        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene.wgsl").into()),
        });
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("draw"),
            layout: None,
            module: &shader,
            entry_point: Some("draw"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let targets = [Some(wgpu::ColorTargetState {
            format: config.format,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let present_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("present"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &targets,
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        if let Some(error) = validation.pop().await {
            return Err(js_error(error.to_string()));
        }
        let params = Params {
            view: [0.0, 0.0, 5.0, 1.0],
            screen: [f32::from(width), f32::from(height), 0.0, 0.0],
        };
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("params"),
            contents: bytemuck::cast_slice(&[params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let output = create_output(&device, width, height);
        let (compute_bind_group, present_bind_group) = create_bind_groups(
            &device,
            &compute_pipeline,
            &present_pipeline,
            &output,
            &uniform,
        );

        Ok(Self {
            device,
            queue,
            surface,
            config,
            compute_pipeline,
            present_pipeline,
            uniform,
            output,
            compute_bind_group,
            present_bind_group,
            width,
            height,
            view: [0.0, 0.0, 5.0, 1.0],
        })
    }

    /// Resizes the canvas and recreates the compute output texture.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error if either dimension is zero.
    pub fn resize(&mut self, width: u16, height: u16) -> Result<(), JsValue> {
        if width == 0 || height == 0 {
            return Err(js_error("renderer dimensions must be positive"));
        }
        if width > MAX_WIDTH || height > MAX_HEIGHT {
            return Err(js_error("renderer dimensions exceed limits"));
        }
        if width == self.width && height == self.height {
            return Ok(());
        }
        self.width = width;
        self.height = height;
        self.config.width = u32::from(width);
        self.config.height = u32::from(height);
        self.surface.configure(&self.device, &self.config);
        self.output = create_output(&self.device, width, height);
        (self.compute_bind_group, self.present_bind_group) = create_bind_groups(
            &self.device,
            &self.compute_pipeline,
            &self.present_pipeline,
            &self.output,
            &self.uniform,
        );
        Ok(())
    }

    /// Renders one frame with the supplied orbit camera and scene settings.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for non-finite camera values, non-positive
    /// distance or radius, or a lost, unavailable, or invalid surface frame.
    pub fn render(
        &mut self,
        yaw: f32,
        pitch: f32,
        distance: f32,
        radius: f32,
        mode: u32,
    ) -> Result<(), JsValue> {
        let mode = match mode {
            0 => 0.0,
            1 => 1.0,
            2 => 2.0,
            _ => return Err(js_error("render mode must be 0, 1, or 2")),
        };
        if !yaw.is_finite()
            || !pitch.is_finite()
            || !distance.is_finite()
            || !radius.is_finite()
            || distance <= 0.0
            || radius <= 0.0
        {
            return Err(js_error(
                "camera values must be finite and positive where required",
            ));
        }
        self.view = [
            f64::from(yaw),
            f64::from(pitch),
            f64::from(distance),
            f64::from(radius),
        ];
        let params = Params {
            view: [yaw, pitch, distance, radius],
            screen: [f32::from(self.width), f32::from(self.height), mode, 0.0],
        };
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::cast_slice(&[params]));

        let frame = acquire_frame(&self.surface, &self.device, &self.config)?;
        let frame_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("draw"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.compute_pipeline);
            pass.set_bind_group(0, &self.compute_bind_group, &[]);
            pass.dispatch_workgroups(
                u32::from(self.width).div_ceil(8),
                u32::from(self.height).div_ceil(8),
                1,
            );
        }
        {
            let attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &frame_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present"),
                color_attachments: &attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.present_pipeline);
            pass.set_bind_group(0, &self.present_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
        Ok(())
    }

    /// Returns packed sphere and plane samples for a normalized canvas point.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error if either coordinate is non-finite.
    pub fn probe(&self, x: f64, y: f64) -> Result<Vec<f64>, JsValue> {
        if !x.is_finite() || !y.is_finite() {
            return Err(js_error("probe coordinates must be finite"));
        }
        let ray = math::camera(
            self.view,
            [f64::from(self.width), f64::from(self.height)],
            [x, y],
        );
        let sphere_sample = math::sphere(ray, [0.0, self.view[3], 0.0], self.view[3]);
        let plane_sample = math::plane(ray, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let sphere_rho = math::footprint(
            sphere_sample,
            ray,
            f64::from(self.height),
            self.view[3],
            true,
        );
        let plane_rho = math::footprint(
            plane_sample,
            ray,
            f64::from(self.height),
            self.view[3],
            false,
        );
        let sphere = sphere_sample.packed(ray, sphere_rho);
        let plane = plane_sample.packed(ray, plane_rho);
        let mut samples = Vec::with_capacity(32);
        samples.extend_from_slice(&sphere);
        samples.extend_from_slice(&plane);
        Ok(samples)
    }
}

fn create_output(device: &wgpu::Device, width: u16, height: u16) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("output"),
        size: wgpu::Extent3d {
            width: u32::from(width),
            height: u32::from(height),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

fn create_bind_groups(
    device: &wgpu::Device,
    compute_pipeline: &wgpu::ComputePipeline,
    present_pipeline: &wgpu::RenderPipeline,
    output: &wgpu::Texture,
    uniform: &wgpu::Buffer,
) -> (wgpu::BindGroup, wgpu::BindGroup) {
    let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());
    let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("draw bindings"),
        layout: &compute_pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&output_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: uniform.as_entire_binding(),
            },
        ],
    });
    let present_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("present bindings"),
        layout: &present_pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 2,
            resource: wgpu::BindingResource::TextureView(&output_view),
        }],
    });
    (compute_bind_group, present_bind_group)
}

fn acquire_frame(
    surface: &wgpu::Surface<'static>,
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
) -> Result<wgpu::SurfaceTexture, JsValue> {
    let current = surface.get_current_texture();
    match current {
        wgpu::CurrentSurfaceTexture::Success(frame)
        | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => Ok(frame),
        wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
            surface.configure(device, config);
            match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => Ok(frame),
                wgpu::CurrentSurfaceTexture::Timeout => Err(js_error("surface frame timed out")),
                wgpu::CurrentSurfaceTexture::Occluded => Err(js_error("surface is occluded")),
                wgpu::CurrentSurfaceTexture::Outdated => Err(js_error("surface is outdated")),
                wgpu::CurrentSurfaceTexture::Lost => Err(js_error("surface is lost")),
                wgpu::CurrentSurfaceTexture::Validation => {
                    Err(js_error("surface frame validation failed"))
                }
            }
        }
        wgpu::CurrentSurfaceTexture::Timeout => Err(js_error("surface frame timed out")),
        wgpu::CurrentSurfaceTexture::Occluded => Err(js_error("surface is occluded")),
        wgpu::CurrentSurfaceTexture::Validation => Err(js_error("surface frame validation failed")),
    }
}

fn js_error(message: impl AsRef<str>) -> JsValue {
    JsValue::from_str(message.as_ref())
}
