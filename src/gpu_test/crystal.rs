use super::support::run;

use crate::math::{Ray, crystal::reference};

const TOLERANCE: f64 = 2.0e-4;
const SURFACE_TOLERANCE: f64 = 8.0e-4;
const TINY: f64 = 0.000_000_953_674_316_406_25;

#[test]
fn crystal_renderer_requests_gaps_and_lights_open_faces() -> Result<(), Box<dyn std::error::Error>>
{
    let mut inputs = Vec::new();
    let mut expected = Vec::new();
    for kind in 0_u8..=5 {
        for display in [false, true] {
            for selected in [false, true] {
                for sign in [-1.0_f32, 1.0] {
                    inputs.push([
                        f32::from(kind),
                        f32::from(u8::from(display)),
                        f32::from(u8::from(selected)),
                        0.0,
                        0.0,
                        0.0,
                        2.0 * sign,
                        0.0,
                    ]);
                    let gap = matches!(kind, 2 | 4) && (display || selected);
                    let normal = if kind == 4 { -1.0_f32 } else { sign };
                    expected.push((f32::from(u8::from(gap)).to_bits(), normal.to_bits()));
                }
            }
        }
    }
    let actual = run("verify_crystal_render", &inputs, inputs.len() * 16)?;
    let gap_errors = expected
        .iter()
        .zip(actual.chunks_exact(16))
        .filter(|(e, q)| e.0 != q[0].to_bits())
        .count();
    let normal_errors = expected
        .iter()
        .zip(actual.chunks_exact(16))
        .filter(|(e, q)| e.1 != q[3].to_bits())
        .count();
    assert_eq!((gap_errors, normal_errors), (0, 0));
    Ok(())
}

#[test]
fn crystal_gpu_grazing_contacts_do_not_claim_resolved_hits()
-> Result<(), Box<dyn std::error::Error>> {
    let inputs = [
        [
            2.508_276_8e-8,
            -8.983_036e-7,
            -9.328_003e-7,
            0.0,
            0.448_044_03,
            -0.688_961_8,
            0.569_726_4,
            1.0,
        ],
        [
            -2.660_425_3e-6,
            1.469_087_9e-6,
            2.748_039_3e-8,
            0.0,
            -0.768_695_53,
            0.312_295_82,
            -0.558_192_2,
            1.0,
        ],
    ];
    let actual = run("verify_crystal", &inputs, inputs.len() * 16)?;
    for (input, q) in inputs.iter().zip(actual.chunks_exact(16)) {
        assert!(q.iter().all(|v| v.is_finite()));
        assert_eq!(q[13].to_bits(), 1.0_f32.to_bits());
        let witness = [f64::from(q[5]), f64::from(q[6]), f64::from(q[7])];
        let error = f64::from(q[12]);
        assert!(reference::distance(witness) <= error);
        if q[0] > 0.0 {
            let t = f64::from(q[1]);
            let p =
                std::array::from_fn(|i| f64::from(input[i + 4]).mul_add(t, f64::from(input[i])));
            assert!(
                reference::distance(p)
                    <= (128.0 * f64::EPSILON).mul_add(reference::magnitude(p), error)
            );
        }
    }
    Ok(())
}

#[test]
fn crystal_scene_packets_scale_once_and_keep_boolean_status()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::scene::{
        model::{Kind, Object},
        query::query_object,
    };
    for scale in [0.25_f32, 2.0] {
        let inputs = [
            [0.625 * scale, 1.0, 2.0 * scale, 4.0, 0.0, 0.0, -1.0, scale],
            [-3.0 * scale, 1.0, -3.0 * scale, 4.0, 0.0, 0.0, -1.0, scale],
        ];
        let actual = run("verify_scene", &inputs, inputs.len() * 32)?;
        let object = Object::new(
            7,
            Kind::CrystallineCluster,
            [0.0, 1.0, 0.0],
            f64::from(scale),
            0.0,
        )?;
        for (index, input) in inputs.iter().enumerate() {
            let ray = Ray {
                o: [
                    f64::from(input[0]),
                    f64::from(input[1]),
                    f64::from(input[2]),
                ],
                d: [0.0, 0.0, -1.0],
            };
            let expected = query_object(&object, ray, true)?.packed(ray);
            let q = &actual[index * 32..(index + 1) * 32];
            assert!(q.iter().all(|v| v.is_finite()));
            let fields: Vec<usize> = if index == 0 {
                (0..20).chain([21, 23]).collect()
            } else {
                vec![0, 1, 2, 3, 5, 21, 23]
            };
            for field in fields {
                let exact = expected[field];
                assert!(
                    (f64::from(q[field]) - exact).abs()
                        <= TOLERANCE * exact.abs().max(f64::from(scale)),
                    "scale {scale}, field {field}: {} versus {exact}",
                    q[field]
                );
            }
            assert!(q[4] >= 0.0 && q[5] >= q[4] && q[20] >= 0.0);
            assert!(q[22].to_bits() == 0 || q[22].to_bits() == 1.0_f32.to_bits());
            assert!(q[24..].iter().all(|v| v.to_bits() == 0));
        }
    }
    Ok(())
}

#[test]
fn crystal_gpu_coincident_envelope_keeps_the_first_ridge() -> Result<(), Box<dyn std::error::Error>>
{
    let a = -1.0_f32 / 5.0_f32.sqrt();
    let input = [[1.5, 0.0, 3.0, 0.0, a, 0.0, 2.0 * a, 1.0]];
    let actual = run("verify_crystal", &input, 16)?;
    let expected = 0.75 / f64::from(-a);
    assert!(actual[0] > 0.0);
    assert!(
        (f64::from(actual[1]) - expected).abs() <= TOLERANCE * expected,
        "coincident envelope skipped the first ridge: {} versus {expected}",
        actual[1]
    );
    Ok(())
}

fn point_inputs() -> Vec<[f32; 8]> {
    let mut points = vec![[0.0; 8]];
    for exponent in -20..=2 {
        let s = 2.0_f32.powi(exponent);
        for p in [
            [0.2, -0.3, 0.7],
            [2.0, -2.0, 1.0],
            [0.75, 0.0, 1.5],
            [0.0, 0.0, 1.0],
            [-0.5, -1.0, -2.0],
            [1.0; 3],
        ] {
            points.push([p[0] * s, p[1] * s, p[2] * s, 0.0, 0.0, 0.0, 0.0, 0.0]);
        }
    }
    points
}
fn ray_inputs() -> Vec<[f32; 8]> {
    let mut rays = vec![[0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]];
    for exponent in -20..=2 {
        let s = 2.0_f32.powi(exponent);
        rays.extend([
            [0.0, 0.0, -4.0 * s, 0.0, 0.0, 0.0, 1.0, 1.0],
            [0.75 * s, 0.0, 3.0 * s, 0.0, 0.0, 0.0, -1.0, 1.0],
            [2.0 * s, 0.5 * s, 2.0 * s, 0.0, -1.0, 0.0, 0.0, 1.0],
            [-2.0 * s, -s, -s, 0.0, 1.0, 0.0, 0.0, 1.0],
            [1.5 * s, -2.0 * s, 3.0 * s, 0.0, 0.0, 1.0, 0.0, 1.0],
            [0.0, 0.0, -s, 0.0, 0.0, 0.0, -1.0, 1.0],
        ]);
    }
    rays
}

#[test]
fn crystal_gpu_global_nearest_and_witnesses_match_exhaustive_model()
-> Result<(), Box<dyn std::error::Error>> {
    let inputs = point_inputs();
    let actual = run("verify_crystal_point", &inputs, inputs.len() * 16)?;
    let mut max_error = 0.0_f64;
    for (input, q) in inputs.iter().zip(actual.chunks_exact(16)) {
        assert!(q.iter().all(|v| v.is_finite()));
        let p = [
            f64::from(input[0]),
            f64::from(input[1]),
            f64::from(input[2]),
        ];
        let point = [f64::from(q[4]), f64::from(q[5]), f64::from(q[6])];
        let exact = reference::distance(p);
        let scale = reference::magnitude(p).max(TINY);
        let upper = f64::from(q[0]);
        let error = f64::from(q[1]);
        assert!(
            1e-12_f64.mul_add(scale, upper) >= exact,
            "GPU bound excludes nearest distance at {p:?}"
        );
        assert!(
            (upper - exact).abs() <= TOLERANCE * scale,
            "GPU nearest distance at {p:?}"
        );
        assert!(
            reference::distance(point) <= 1e-12_f64.mul_add(scale, error),
            "GPU surface witness at {point:?}"
        );
        let normal = [f64::from(q[8]), f64::from(q[9]), f64::from(q[10])];
        assert!((reference::magnitude(normal) - 1.0).abs() <= TOLERANCE);
        max_error = max_error.max((upper - exact).abs() / scale);
    }
    eprintln!(
        "crystal GPU points={} max_scaled_upper_error={max_error}",
        inputs.len()
    );
    Ok(())
}

#[test]
fn crystal_gpu_first_hits_and_miss_witnesses_match_exhaustive_model()
-> Result<(), Box<dyn std::error::Error>> {
    let inputs = ray_inputs();
    let actual = run("verify_crystal", &inputs, inputs.len() * 16)?;
    let mut uncertain = 0;
    let mut mismatches = 0;
    let mut max_time_error = 0.0_f64;
    for (input, q) in inputs.iter().zip(actual.chunks_exact(16)) {
        assert!(q.iter().all(|v| v.is_finite()));
        let ray = Ray {
            o: [
                f64::from(input[0]),
                f64::from(input[1]),
                f64::from(input[2]),
            ],
            d: [
                f64::from(input[4]),
                f64::from(input[5]),
                f64::from(input[6]),
            ],
        };
        let scale = reference::magnitude(ray.o).max(TINY);
        let hit = q[0] > 0.0;
        let error = f64::from(q[12]);
        assert!(q[13].to_bits() == 0 || q[13].to_bits() == 1.0_f32.to_bits());
        uncertain += usize::from(q[13] > 0.0);
        match (reference::first_hit(ray), hit) {
            (Some(t), true) => {
                let delta = (f64::from(q[1]) - t).abs();
                max_time_error = max_time_error.max(delta / scale);
                assert!(
                    delta <= TOLERANCE.mul_add(scale, error),
                    "GPU first hit at {:?}",
                    ray.o
                );
            }
            (None, false) => {}
            _ => {
                mismatches += 1;
                assert!(
                    q[13] > 0.0,
                    "resolved GPU ray classification at {:?}",
                    ray.o
                );
            }
        }
        let point = [f64::from(q[5]), f64::from(q[6]), f64::from(q[7])];
        assert!(
            reference::distance(point) <= SURFACE_TOLERANCE * reference::magnitude(point).max(TINY)
        );
        let normal = [f64::from(q[8]), f64::from(q[9]), f64::from(q[10])];
        assert!((reference::magnitude(normal) - 1.0).abs() <= TOLERANCE);
        assert!(q[11] >= 0.0 && q[2] >= q[11]);
        let t = f64::from(q[1]);
        let delta = [
            ray.d[0].mul_add(t, ray.o[0]) - point[0],
            ray.d[1].mul_add(t, ray.o[1]) - point[1],
            ray.d[2].mul_add(t, ray.o[2]) - point[2],
        ];
        let distance = reference::magnitude(delta);
        if hit {
            assert!(distance <= TOLERANCE.mul_add(scale, error));
        } else {
            assert!(distance <= TOLERANCE.mul_add(scale, f64::from(q[2]) + error));
        }
    }
    eprintln!(
        "crystal GPU rays={} uncertain={uncertain} classification_disagreements={mismatches} max_scaled_time_error={max_time_error}",
        inputs.len()
    );
    Ok(())
}

#[test]
fn crystal_gpu_gap_skip_and_invalid_inputs_are_explicit() -> Result<(), Box<dyn std::error::Error>>
{
    let inv = 1.0_f32 / 3.0_f32.sqrt();
    let inputs = [
        [-3.0, -3.0, -3.0, 0.0, -inv, -inv, -inv, 0.0],
        [1e20, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0],
        [0.0; 8],
    ];
    let actual = run("verify_crystal", &inputs, inputs.len() * 16)?;
    assert!(actual.iter().all(|x| x.is_finite()));
    assert_eq!(actual[13].to_bits(), 2.0_f32.to_bits());
    assert_eq!(actual[2].to_bits(), 0);
    assert_eq!(actual[12].to_bits(), 0);
    assert_eq!(actual[29].to_bits(), 3.0_f32.to_bits());
    assert_eq!(actual[45].to_bits(), 3.0_f32.to_bits());
    Ok(())
}

#[test]
fn crystal_point_bounds_survive_squared_underflow() -> Result<(), Box<dyn std::error::Error>> {
    let inputs = [
        [0.0, 0.0, -1e-20, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, -1e-25, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, -1e-30, 0.0, 0.0, 0.0, 0.0, 0.0],
    ];
    let actual = run("verify_crystal_point", &inputs, inputs.len() * 16)?;
    for (index, input) in inputs.iter().enumerate() {
        let exact = f64::from(input[2]).abs() / 5.0_f64.sqrt();
        let upper = f64::from(actual[index * 16]);
        assert!(
            upper >= exact,
            "GPU upper {upper} excludes tiny distance {exact}"
        );
        assert!(actual[index * 16 + 1].is_finite());
    }
    let rays = inputs.map(|p| [p[0], p[1], p[2], 0.0, 0.0, 0.0, -1.0, 0.0]);
    let shadow = run("verify_crystal", &rays, rays.len() * 16)?;
    for (input, q) in rays.iter().zip(shadow.chunks_exact(16)) {
        assert_eq!(q[13].to_bits(), 1.0_f32.to_bits());
        assert!(
            f64::from(q[2]) >= f64::from(input[2]).abs(),
            "uncertain shadow discarded its apex-witness distance"
        );
    }
    Ok(())
}
