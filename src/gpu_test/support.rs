use std::{borrow::Cow, error::Error, sync::mpsc, time::Duration};

use bytemuck::cast_slice;
use wgpu::util::DeviceExt;

pub(super) fn run(
    entry: &str,
    rays: &[[f32; 8]],
    output_floats: usize,
) -> Result<Vec<f32>, Box<dyn Error>> {
    pollster::block_on(run_async(
        entry,
        rays,
        output_floats,
        None,
        crate::heightfield::LEAF,
        [0.0; 3],
    ))
}

pub(super) fn run_scene(
    entry: &str,
    rays: &[[f32; 8]],
    records: &[crate::scene::packed::Record],
) -> Result<Vec<f32>, Box<dyn Error>> {
    pollster::block_on(run_async(
        entry,
        rays,
        rays.len() * 16,
        Some(records),
        crate::heightfield::LEAF,
        [0.0; 3],
    ))
}

pub(super) fn run_scene_field(
    entry: &str,
    rays: &[[f32; 8]],
    records: &[crate::scene::packed::Record],
    field: crate::heightfield::Field<'_>,
) -> Result<Vec<f32>, Box<dyn Error>> {
    pollster::block_on(run_async(
        entry,
        rays,
        rays.len() * 16,
        Some(records),
        field,
        [0.0; 3],
    ))
}
pub(super) fn run_field(
    entry: &str,
    rays: &[[f32; 8]],
    output_floats: usize,
    field: crate::heightfield::Field<'_>,
) -> Result<Vec<f32>, Box<dyn Error>> {
    pollster::block_on(run_async(entry, rays, output_floats, None, field, [0.0; 3]))
}
pub(super) fn run_material_origin(
    entry: &str,
    rays: &[[f32; 8]],
    origin: [f64; 3],
) -> Result<Vec<f32>, Box<dyn Error>> {
    pollster::block_on(run_async(
        entry,
        rays,
        rays.len() * 16,
        None,
        crate::heightfield::LEAF,
        origin,
    ))
}
pub(super) fn used_bindings(
    entry: &str,
) -> Result<std::collections::BTreeSet<u32>, Box<dyn Error>> {
    let module = naga::front::wgsl::parse_str(&crate::shader::source())?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)?;
    if module.entry_points.len() > 64 || module.global_variables.len() > 64 {
        return Err("verification reflection budget".into());
    }
    let index = module
        .entry_points
        .iter()
        .take(64)
        .position(|point| point.name == entry)
        .ok_or("missing verification entry")?;
    Ok(module
        .global_variables
        .iter()
        .take(64)
        .filter_map(|(handle, global)| {
            let binding = global.binding.as_ref()?;
            (binding.group == 0 && !info.get_entry_point(index)[handle].is_empty())
                .then_some(binding.binding)
        })
        .collect())
}
fn material_tiles(
    entry: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    origin: [f64; 3],
) -> Result<Option<crate::material::gpu::Tiles>, Box<dyn Error>> {
    if !matches!(
        entry,
        "verify_material"
            | "verify_material_anchor"
            | "verify_material_mapping"
            | "verify_leaf_material"
            | "verify_material_hit"
            | "verify_material_hit_full"
            | "verify_leaf_culling"
            | "verify_material_color"
            | "verify_slope_precision"
            | "verify_texh_borrow"
            | "verify_texh_flat"
            | "verify_instance_pose"
    ) {
        return Ok(None);
    }
    let mut tiles = crate::material::gpu::Tiles::empty(device);
    assert!(!tiles.ready());
    let leaf = matches!(
        entry,
        "verify_leaf_material" | "verify_material_color" | "verify_instance_pose"
    );
    assert!(tiles.ensure(device, queue, leaf)?);
    assert!(!tiles.ensure(device, queue, leaf)?);
    tiles.update(queue, origin, leaf)?;
    Ok(Some(tiles))
}

pub(super) async fn test_device() -> Result<(wgpu::Device, wgpu::Queue), Box<dyn Error>> {
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

fn input_buffer(device: &wgpu::Device, label: &str, contents: &[u8]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents,
        usage: wgpu::BufferUsages::STORAGE,
    })
}
async fn run_async(
    entry: &str,
    rays: &[[f32; 8]],
    output_floats: usize,
    records: Option<&[crate::scene::packed::Record]>,
    field: crate::heightfield::Field<'_>,
    origin: [f64; 3],
) -> Result<Vec<f32>, Box<dyn Error>> {
    let (device, queue) = test_device().await?;
    let ray_buffer = input_buffer(&device, "test rays", cast_slice(rays));
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
        source: wgpu::ShaderSource::Wgsl(Cow::Owned(crate::shader::source())),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &shader,
        entry_point: Some(entry),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    let scene_buffer =
        records.map(|records| input_buffer(&device, "test hierarchy", cast_slice(records)));
    let used = used_bindings(entry)?;
    let heights = if used.contains(&9) {
        Some(crate::heightfield::gpu::Heights::new(&device, field)?.buffer)
    } else {
        None
    };
    let tiles = material_tiles(entry, &device, &queue, origin)?;
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
    if let Some(buffer) = &heights {
        entries.push(wgpu::BindGroupEntry {
            binding: 9,
            resource: buffer.as_entire_binding(),
        });
    }
    if let Some(buffer) = &scene_buffer {
        entries.push(wgpu::BindGroupEntry {
            binding: 5,
            resource: buffer.as_entire_binding(),
        });
    }
    if let (Some(tiles), Some(view)) = (&tiles, &material_view) {
        entries.extend(
            tiles
                .bindings(view)
                .into_iter()
                .filter(|entry| used.contains(&entry.binding)),
        );
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
