use std::error::Error;

use crate::math::{Ray, footprint, plane, pureweight, sphere};

mod finite;
mod finite_shadow;
mod goursat;
mod heightfield;
mod material;
mod render;
mod scene;
mod support;
mod texh;

const TOLERANCE: f64 = 2.0e-4;

#[test]
fn scene_shader_parses_and_validates() -> Result<(), Box<dyn Error>> {
    let module = naga::front::wgsl::parse_str(&crate::shader::source())?;
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

#[test]
fn scene_verify_matches_cpu_math() -> Result<(), Box<dyn Error>> {
    let rays = rays();
    let expected: Vec<[f64; 16]> = rays.iter().flat_map(|ray| cpu_results(*ray)).collect();
    for packed in &expected {
        // Keep the CPU packing check on the same UV inputs as pureweight.
        assert_eq!(
            packed[11].to_bits(),
            pureweight([packed[3], packed[4]], packed[15]).to_bits()
        );
    }
    let actual = support::run("verify", &rays, rays.len() * 32)?;
    assert_results(&actual, &expected);
    Ok(())
}

mod scale;
