#![deny(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo)]

use bytemuck::{Pod, Zeroable};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use wgpu::util::DeviceExt;

use crate::scene::{MAX_OBJECTS, Object, parse_camera, parse_objects};

const MAX_WIDTH: u16 = 1_440;
const MAX_HEIGHT: u16 = 1_000;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    origin: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    forward: [f32; 4],
    screen: [u32; 4],
    selection: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuObject {
    meta: [u32; 4],
    position_size: [f32; 4],
    params: [f32; 4],
}

const _: () = assert!(std::mem::size_of::<Params>() == 96);
const _: () = assert!(std::mem::size_of::<GpuObject>() == 48);

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
    objects: wgpu::Buffer,
    output: wgpu::Texture,
    compute_bind_group: wgpu::BindGroup,
    present_bind_group: wgpu::BindGroup,
    width: u16,
    height: u16,
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
        let (width, height) = canvas_dimensions(&canvas)?;
        let (surface, device, queue, config) = create_surface(canvas, width, height).await?;
        let (compute_pipeline, present_pipeline) = create_pipelines(&device, config.format).await?;
        let uniform = create_uniform(&device, width, height);
        let objects = create_objects_buffer(&device)?;
        let output = create_output(&device, width, height);
        let (compute_bind_group, present_bind_group) = create_bind_groups(
            &device,
            &compute_pipeline,
            &present_pipeline,
            &output,
            &uniform,
            &objects,
        );

        Ok(Self {
            device,
            queue,
            surface,
            config,
            compute_pipeline,
            present_pipeline,
            uniform,
            objects,
            output,
            compute_bind_group,
            present_bind_group,
            width,
            height,
        })
    }
    /// Resizes the canvas and recreates the compute output texture.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error if either dimension is zero or exceeds its limit.
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
            &self.objects,
        );
        Ok(())
    }

    /// Renders one frame from a six-value camera packet and an eight-value
    /// packet per scene object.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error if the camera, scene, mode, or selected IDs
    /// are invalid, or if the surface frame is lost, unavailable, or invalid.
    pub fn render(
        &mut self,
        camera: &[f64],
        scene: &[f64],
        mode: u32,
        selected: &[u32],
    ) -> Result<(), JsValue> {
        let (params, gpu_objects) =
            frame_params(camera, scene, mode, selected, self.width, self.height)?;
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&params));
        self.queue
            .write_buffer(&self.objects, 0, bytemuck::cast_slice(&gpu_objects));

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
}

fn frame_params(
    camera_packet: &[f64],
    scene_packet: &[f64],
    mode: u32,
    selected: &[u32],
    width: u16,
    height: u16,
) -> Result<(Params, [GpuObject; MAX_OBJECTS]), JsValue> {
    if mode > 2 {
        return Err(js_error("render mode must be 0, 1, or 2"));
    }
    let camera = parse_camera(camera_packet).map_err(|error| js_error(error.to_string()))?;
    let objects = parse_objects(scene_packet).map_err(|error| js_error(error.to_string()))?;
    let mut slot_mask = 0_u32;
    for &id in selected {
        let slot = objects
            .iter()
            .position(|object| object.id() == id)
            .ok_or_else(|| js_error("selected object id is unknown"))?;
        slot_mask |= 1_u32 << slot;
    }

    let basis = camera.basis();
    let mut gpu_objects = [EMPTY_GPU_OBJECT; MAX_OBJECTS];
    for (slot, object) in objects.iter().enumerate() {
        gpu_objects[slot] = gpu_object(object);
    }
    let params = Params {
        origin: [
            f32_value(basis.origin[0]),
            f32_value(basis.origin[1]),
            f32_value(basis.origin[2]),
            0.0,
        ],
        right: [
            f32_value(basis.right[0]),
            f32_value(basis.right[1]),
            f32_value(basis.right[2]),
            0.0,
        ],
        up: [
            f32_value(basis.up[0]),
            f32_value(basis.up[1]),
            f32_value(basis.up[2]),
            0.0,
        ],
        forward: [
            f32_value(basis.forward[0]),
            f32_value(basis.forward[1]),
            f32_value(basis.forward[2]),
            1.8,
        ],
        screen: [
            u32::from(width),
            u32::from(height),
            mode,
            u32::try_from(objects.len()).map_err(|_| js_error("too many scene objects"))?,
        ],
        selection: [slot_mask, 0, 0, 0],
    };
    Ok((params, gpu_objects))
}

fn canvas_dimensions(canvas: &HtmlCanvasElement) -> Result<(u16, u16), JsValue> {
    let width = u16::try_from(canvas.width().clamp(1, u32::from(MAX_WIDTH)))
        .map_err(|_| js_error("canvas width is out of range"))?;
    let height = u16::try_from(canvas.height().clamp(1, u32::from(MAX_HEIGHT)))
        .map_err(|_| js_error("canvas height is out of range"))?;
    Ok((width, height))
}

#[allow(clippy::future_not_send, reason = "browser WebGPU futures are local")]
async fn create_surface(
    canvas: HtmlCanvasElement,
    width: u16,
    height: u16,
) -> Result<
    (
        wgpu::Surface<'static>,
        wgpu::Device,
        wgpu::Queue,
        wgpu::SurfaceConfiguration,
    ),
    JsValue,
> {
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
    Ok((surface, device, queue, config))
}

#[allow(clippy::future_not_send, reason = "browser WebGPU futures are local")]
async fn create_pipelines(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> Result<(wgpu::ComputePipeline, wgpu::RenderPipeline), JsValue> {
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("scene shader"),
        source: wgpu::ShaderSource::Wgsl(crate::shader::SOURCE.into()),
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
        format,
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
    Ok((compute_pipeline, present_pipeline))
}

fn create_uniform(device: &wgpu::Device, width: u16, height: u16) -> wgpu::Buffer {
    let params = Params {
        origin: [0.0, 0.0, 1.0, 0.0],
        right: [1.0, 0.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0, 0.0],
        forward: [0.0, 0.0, -1.0, 1.8],
        screen: [u32::from(width), u32::from(height), 0, 0],
        selection: [0; 4],
    };
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("params"),
        contents: bytemuck::cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

const EMPTY_GPU_OBJECT: GpuObject = GpuObject {
    meta: [0; 4],
    position_size: [0.0; 4],
    params: [0.0; 4],
};

const fn gpu_object(object: &Object) -> GpuObject {
    let position = object.position();
    GpuObject {
        meta: [object.id(), object.kind().tag(), 0, 0],
        position_size: [
            f32_value(position[0]),
            f32_value(position[1]),
            f32_value(position[2]),
            f32_value(object.size()),
        ],
        params: [f32_value(object.parameter()), 0.0, 0.0, 0.0],
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "scene and camera validation bounds values well within f32"
)]
const fn f32_value(value: f64) -> f32 {
    value as f32
}

fn create_objects_buffer(device: &wgpu::Device) -> Result<wgpu::Buffer, JsValue> {
    let size = u64::try_from(std::mem::size_of::<GpuObject>() * MAX_OBJECTS)
        .map_err(|_| js_error("object buffer size is out of range"))?;
    Ok(device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("objects"),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    }))
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
    objects: &wgpu::Buffer,
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
            wgpu::BindGroupEntry {
                binding: 5,
                resource: objects.as_entire_binding(),
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
