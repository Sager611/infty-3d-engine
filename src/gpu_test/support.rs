use std::{borrow::Cow, error::Error, sync::mpsc, time::Duration};

use bytemuck::cast_slice;
use wgpu::util::DeviceExt;

pub(super) const SHADER: &str = crate::shader::SOURCE;

pub(super) fn run(
    entry: &str,
    rays: &[[f32; 8]],
    output_floats: usize,
) -> Result<Vec<f32>, Box<dyn Error>> {
    pollster::block_on(run_async(entry, rays, output_floats, None))
}

pub(super) fn run_scene(
    entry: &str,
    rays: &[[f32; 8]],
    records: &[crate::scene::packed::Record],
) -> Result<Vec<f32>, Box<dyn Error>> {
    pollster::block_on(run_async(entry, rays, rays.len() * 16, Some(records)))
}

fn material_tiles(
    entry: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<Option<crate::material::gpu::Tiles>, Box<dyn Error>> {
    if !matches!(entry, "verify_material" | "verify_material_mapping") {
        return Ok(None);
    }
    let mut tiles = crate::material::gpu::Tiles::empty(device);
    assert!(!tiles.ready());
    assert!(tiles.ensure(device, queue)?);
    assert!(!tiles.ensure(device, queue)?);
    tiles.update(queue, [0.0; 3])?;
    Ok(Some(tiles))
}

async fn test_device() -> Result<(wgpu::Device, wgpu::Queue), Box<dyn Error>> {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: cfg!(target_os = "linux"),
            ..Default::default()
        })
        .await?;
    Ok(adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await?)
}

async fn run_async(
    entry: &str,
    rays: &[[f32; 8]],
    output_floats: usize,
    records: Option<&[crate::scene::packed::Record]>,
) -> Result<Vec<f32>, Box<dyn Error>> {
    let (device, queue) = test_device().await?;
    let ray_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: cast_slice(rays),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let result_size = u64::try_from(output_floats * std::mem::size_of::<f32>())?;
    let result_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: result_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: result_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER)),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &shader,
        entry_point: Some(entry),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    let scene_buffer = records.map(|records| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("test hierarchy"),
            contents: cast_slice(records),
            usage: wgpu::BufferUsages::STORAGE,
        })
    });
    let tiles = material_tiles(entry, &device, &queue)?;
    let material_view = tiles.as_ref().map(crate::material::gpu::Tiles::view);
    let mut entries = vec![
        wgpu::BindGroupEntry {
            binding: 3,
            resource: ray_buffer.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 4,
            resource: result_buffer.as_entire_binding(),
        },
    ];
    if let Some(buffer) = &scene_buffer {
        entries.push(wgpu::BindGroupEntry {
            binding: 5,
            resource: buffer.as_entire_binding(),
        });
    }
    if let (Some(tiles), Some(view)) = (&tiles, &material_view) {
        entries.extend(tiles.bindings(view));
    }
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &entries,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(u32::try_from(rays.len())?, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&result_buffer, 0, &staging, 0, result_size);
    queue.submit([encoder.finish()]);
    let (sender, receiver) = mpsc::channel();
    staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            drop(sender.send(result));
        });
    device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    })?;
    receiver.recv_timeout(Duration::from_secs(10))??;
    let actual = cast_slice::<u8, f32>(&staging.slice(..).get_mapped_range()?).to_vec();
    staging.unmap();
    Ok(actual)
}
