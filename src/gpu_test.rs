use std::{borrow::Cow, error::Error, sync::mpsc, time::Duration};

use bytemuck::cast_slice;
use wgpu::util::DeviceExt;

use crate::math::{Ray, footprint, plane, pureweight, sphere};

const SHADER: &str = include_str!("scene.wgsl");
const TOLERANCE: f64 = 2.0e-4;

#[test]
fn scene_shader_parses_and_validates() -> Result<(), Box<dyn Error>> {
    let module = naga::front::wgsl::parse_str(SHADER)?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)?;
    Ok(())
}

const fn rays() -> [[f32; 8]; 14] {
    [
        [0.0, 1.0, -3.0, 0.0, 0.0, 0.0, 1.0, 1.0],
        [2.0, 1.0, -3.0, 0.0, 0.0, 0.0, 1.0, 2.0],
        [0.0, 1.0, 3.0, 0.0, 0.0, 0.0, 1.0, 4.0],
        [0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 8.0],
        [1.0, 1.0, -3.0, 0.0, 0.0, 0.0, 1.0, 16.0],
        [0.0, 3.0, 0.0, 0.0, 0.0, -1.0, 0.0, 32.0],
        [2.0, 3.0, 4.0, 0.0, 0.0, -1.0, 0.0, 64.0],
        [2.0, 3.0, 4.0, 0.0, 1.0, 0.0, 0.0, 128.0],
        [2.0, 0.0, 4.0, 0.0, 1.0, 0.0, 0.0, 256.0],
        [2.0, -3.0, 4.0, 0.0, 0.0, -1.0, 0.0, 512.0],
        [1e-7, 3.0, 0.0, 0.0, 0.0, -1.0, 0.0, 768.0],
        [0.137, 1.0, 0.243, 0.0, 0.0, -1.0, 0.0, 1_024.0],
        [-0.2345, 0.001, 0.6789, 0.0, 0.0, -1.0, 0.0, 4_096.0],
        [
            -0.249_999_97,
            0.001,
            0.312_345,
            0.0,
            0.0,
            -1.0,
            0.0,
            4_096.0,
        ],
    ]
}

fn cpu_results(ray: [f32; 8]) -> [[f64; 16]; 2] {
    let height = f64::from(ray[7]);
    let ray = Ray {
        o: [f64::from(ray[0]), f64::from(ray[1]), f64::from(ray[2])],
        d: [f64::from(ray[4]), f64::from(ray[5]), f64::from(ray[6])],
    };
    let sphere_sample = sphere(ray, [0.0, 1.0, 0.0], 1.0);
    let plane_sample = plane(ray, [0.0; 3], [0.0, 1.0, 0.0]);
    [
        sphere_sample.packed(ray, footprint(sphere_sample, ray, height, 1.0, true)),
        plane_sample.packed(ray, footprint(plane_sample, ray, height, 1.0, false)),
    ]
}

fn assert_results(actual: &[f32], expected: &[[f64; 16]]) {
    for (index, (actual, expected)) in actual.chunks_exact(16).zip(expected.iter()).enumerate() {
        for field in 0..16 {
            assert!(
                actual[field].is_finite(),
                "ray {index} field {field} is not finite"
            );
            let actual_field = f64::from(actual[field]);
            let expected_field = if field == 11 {
                pureweight(
                    [f64::from(actual[3]), f64::from(actual[4])],
                    f64::from(actual[15]),
                )
            } else {
                expected[field]
            };
            let difference = if field == 3 {
                let direct = (actual_field - expected_field).abs();
                direct.min((1.0 - direct).abs())
            } else {
                (actual_field - expected_field).abs()
            };
            let limit = if field == 15 {
                TOLERANCE * expected_field.abs().max(1.0e-9)
            } else {
                TOLERANCE * expected_field.abs().max(1.0)
            };
            assert!(
                difference <= limit,
                "ray {index} field {field}: {actual_field} != {expected_field} (limit {limit})"
            );
        }
    }
}

async fn run_gpu() -> Result<(), Box<dyn Error>> {
    let rays = rays();
    let expected: Vec<[f64; 16]> = rays.iter().flat_map(|ray| cpu_results(*ray)).collect();
    for packed in &expected {
        // Keep the CPU packing check on the same UV inputs as pureweight.
        assert_eq!(
            packed[11].to_bits(),
            pureweight([packed[3], packed[4]], packed[15]).to_bits()
        );
    }
    let instance = wgpu::Instance::default();
    #[cfg(target_os = "linux")]
    let force_fallback_adapter = true;
    #[cfg(not(target_os = "linux"))]
    let force_fallback_adapter = false;
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter,
            ..Default::default()
        })
        .await?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await?;
    let ray_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: cast_slice(&rays),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let result_size = u64::try_from(expected.len() * 16 * std::mem::size_of::<f32>())?;
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
        entry_point: Some("verify"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 3,
                resource: ray_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: result_buffer.as_entire_binding(),
            },
        ],
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
    assert_results(&actual, &expected);
    Ok(())
}

#[test]
fn scene_verify_matches_cpu_math() -> Result<(), Box<dyn Error>> {
    pollster::block_on(run_gpu())
}
