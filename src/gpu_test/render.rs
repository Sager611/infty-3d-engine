use super::support::run;
use crate::math::pureweight;
use std::error::Error;

#[test]
fn selection_requires_valid_upper_gap_without_filling_hits() -> Result<(), Box<dyn Error>> {
    // [upper, lower, error, kind, hit, selected, thickness, status]
    let fixtures = [
        ([0.005, 0.005, 0.0, 1.0, 0.0, 1.0, 0.01, 0.0], true),
        ([0.02, 0.0, 0.00001, 2.0, 0.0, 1.0, 0.01, 0.0], false),
        ([0.005, 0.0, 0.00001, 2.0, 0.0, 1.0, 0.01, 1.0], true),
        ([0.005, 0.005, 0.0, 3.0, 0.0, 1.0, 0.01, 0.0], false),
        ([0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.01, 0.0], false),
        ([0.005, 0.005, 0.0, 1.0, 0.0, 0.0, 0.01, 0.0], false),
        ([0.005, 0.0, 0.00001, 2.0, 0.0, 1.0, 0.01, 3.0], false),
    ];
    let rays: Vec<[f32; 8]> = fixtures.iter().map(|(ray, _)| *ray).collect();
    let output = run("verify_outline", &rays, rays.len() * 16)?;
    for (index, (_, expected)) in fixtures.into_iter().enumerate() {
        assert_eq!(
            output[index * 16].to_bits(),
            f32::from(u8::from(expected)).to_bits(),
            "case {index}"
        );
        assert!(
            output[index * 16 + 1..(index + 1) * 16]
                .iter()
                .all(|v| v.to_bits() == 0)
        );
    }
    Ok(())
}

#[test]
fn canonical_plane_uv_and_footprint_scale_once() -> Result<(), Box<dyn Error>> {
    let mut rays = Vec::new();
    for size in [0.5, 1.0, 2.0] {
        for mode in [0.0, 2.0] {
            rays.push([0.0, 0.0, 0.0, mode, 0.0, 0.0, 0.0, size]);
        }
    }
    let output = run("verify_surface", &rays, rays.len() * 16)?;
    let uv = [f64::from(0.6_f32), f64::from(0.2_f32)];
    let a: [f64; 3] = [0.78, 0.81, 0.82];
    let b: [f64; 3] = [0.93, 0.94, 0.93];
    let sky: [f64; 3] = [0.968_627_45, 0.964_705_9, 0.949_019_6];
    for (index, ray) in rays.iter().enumerate() {
        let actual = &output[index * 16..(index + 1) * 16];
        let expected_rho = 8.0 / (1.8 * 100.0 * f64::from(ray[7]));
        assert!(
            (f64::from(actual[0]) - expected_rho).abs() <= 2.0e-4,
            "size {}",
            ray[7]
        );
        let weight = if ray[3] > 0.0 {
            pureweight(uv, f64::from(actual[0]))
        } else {
            0.0
        };
        for channel in 0..3 {
            let expected = 0.02f64.mul_add(
                sky[channel],
                (b[channel] - a[channel]).mul_add(weight, a[channel]),
            );
            assert!(
                (f64::from(actual[channel + 1]) - expected).abs() <= 2.0e-4,
                "size {} mode {} channel {channel}",
                ray[7],
                ray[3]
            );
        }
        assert!(actual[4..].iter().all(|v| v.to_bits() == 0));
    }
    Ok(())
}
