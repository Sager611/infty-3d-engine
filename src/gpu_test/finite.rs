use super::support::run;

#[test]
fn finite_gpu_keeps_camera_and_instance_low_parts_at_metre_offsets()
-> Result<(), Box<dyn std::error::Error>> {
    let position = [1.000_000_031_25, 1.000_000_012_5, -0.725_000_015];
    for axis in [0_u8, 1, 2] {
        let object = Object::new(7, Kind::FinitePlane, position, 0.07, f64::from(axis))?;
        let normal = match axis {
            0 => 2,
            1 => 0,
            _ => 1,
        };
        let mut origin = position;
        origin[normal] += 1e-8;
        let mut direction = [0.0; 3];
        direction[normal] = -1.0;
        let (high, low) = crate::math::types::split32(origin).ok_or("split")?;
        let mut d = [0.0_f32; 3];
        d[normal] = -1.0;
        let inputs = [
            [high[0], high[1], high[2], 0.0, d[0], d[1], d[2], 0.0],
            [low[0], low[1], low[2], 0.0, 0.0, 0.0, 0.0, 0.0],
        ];
        let packet = crate::scene::packed::pack(&[object], &[])?;
        let actual = super::support::run_scene("verify_finite_precise", &inputs, &packet.records)?;
        let expected = query_object(
            &object,
            Ray {
                o: origin,
                d: direction,
            },
        )?;
        assert!(actual[0] > 0.5);
        assert!(
            (f64::from(actual[1]) - expected.sample.t).abs() < 2e-13,
            "lost nano-scale gap: {actual:?}"
        );
        assert!(
            actual[8] < 0.5,
            "axis bits accidentally selected an unselected plane"
        );
        assert!(actual.iter().all(|v| v.is_finite()));
    }
    Ok(())
}
#[test]
fn finite_scene_leaf_axes_cutouts_and_miss_bounds_match_cpu()
-> Result<(), Box<dyn std::error::Error>> {
    let inputs = [
        [0.012, -0.217, 1.0, 0.0, 0.0, 0.0, -1.0, 2.0],
        [1.0, 0.012, -0.217, 1.0, -1.0, 0.0, 0.0, 2.0],
        [-0.217, 1.0, 0.012, 2.0, 0.0, -1.0, 0.0, 2.0],
        [0.012, -0.217, -1.0, 0.0, 0.0, 0.0, 1.0, 2.0],
        [-0.96, -0.96, 1.0, 0.0, 0.0, 0.0, -1.0, 2.0],
        [0.012, -0.217, 1.0, 0.0, 0.0, 0.0, -1.0, 0.0],
        [1.01, 0.0, 1.0, 0.0, 0.0, 0.0, -1.0, 0.0],
        [-0.8, 0.0, 0.012, 0.0, 1.0, 0.0, 0.0, 2.0],
        [-25.0 / 256.0, 114.0 / 256.0, 1.0, 0.0, 0.0, 0.0, -1.0, 2.0],
        [-66.0 / 256.0, 114.0 / 256.0, 1.0, 0.0, 0.0, 0.0, -1.0, 2.0],
        [-0.8, 0.5, 0.038, 0.0, 1.0, 0.0, -0.05, 2.0],
    ];
    let actual = run("verify_finite_scene", &inputs, inputs.len() * 32)?;
    assert!(actual[8 * 32 + 9] < -0.2 && actual[9 * 32 + 9] > 0.2);
    assert!(
        actual[10 * 32 + 2] > 0.5 && actual[10 * 32 + 6] > 0.1,
        "oblique ray must skip the uncovered near root and hit the far blade"
    );
    for (index, row) in actual.chunks_exact(32).enumerate() {
        let input = inputs[index];
        let material = if input[7] > 1.0 {
            crate::material::Material::Leaf
        } else {
            crate::material::Material::Steel
        };
        let object = Object::new(7, Kind::FinitePlane, [0.0; 3], 1.0, f64::from(input[3]))?
            .with_material(material);
        let ray = Ray {
            o: [0, 1, 2].map(|i| f64::from(input[i])),
            d: [4, 5, 6].map(|i| f64::from(input[i])),
        };
        let expected = query_object(&object, ray)?.packed(ray);
        if index < 4 {
            let height = crate::heightfield::LEAF.sample([0.012, -0.217])?.height;
            let normal_axis = [2, 0, 1, 2][index];
            assert!(row[2] > 0.5);
            assert!(
                (f64::from(row[6 + normal_axis]) - height).abs() < 3e-6,
                "Leaf hit stayed on the flat carrier: {row:?}"
            );
        }
        if index == 7 {
            assert!(row[2] > 0.5, "edge-on ray missed the cup");
            let xy = [6, 7].map(|i| f64::from(row[i]));
            let height = crate::heightfield::LEAF.sample(xy)?.height;
            assert!((height - f64::from(row[8])).abs() < 0.0005);
        }
        for (field, (gpu, cpu)) in row.iter().take(24).zip(expected).enumerate() {
            assert!(
                gpu.is_finite() && (f64::from(*gpu) - cpu).abs() < 2e-5,
                "finite scene{index} field{field}: {gpu} != {cpu}"
            );
        }
        assert!(row[25] < 0.5 && row[26] > 0.5);
    }
    Ok(())
}
#[test]
fn finite_gpu_lights_both_sides() -> Result<(), Box<dyn std::error::Error>> {
    let actual = run("verify_finite_pending", &[[0.0; 8]], 16)?;
    assert!(
        actual[7] < -0.99,
        "back face kept its forward lighting normal"
    );
    Ok(())
}
#[test]
fn finite_gpu_pending_bounds_cannot_be_hidden_by_an_existing_candidate()
-> Result<(), Box<dyn std::error::Error>> {
    let inputs = [
        [0.5, 1.0, 1.0, 1.0, 0.01, 0.0, 0.0, 0.0],
        [1.1, 1.0, 1.0, 1.0, 0.01, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0],
        [2.0_f32.powi(100), 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ];
    let actual = run("verify_finite_pending", &inputs, inputs.len() * 16)?;
    for (row, expected) in actual
        .chunks_exact(16)
        .zip([true, false, true, false, true, true])
    {
        assert_eq!(row[0] > 0.5, expected);
    }
    Ok(())
}

use crate::{
    math::Ray,
    scene::{
        model::{Kind, Object},
        query::query_object,
    },
};

#[test]
fn finite_plane_real_gpu_matches_cpu_edges_front_back_and_misses()
-> Result<(), Box<dyn std::error::Error>> {
    let inputs = [
        [0.25, 1.25, 2.0, 5.0, 0.0, 0.0, -1.0, 1.0],
        [0.25, 1.25, -2.0, 5.0, 0.0, 0.0, 1.0, 1.0],
        [1.125, 1.0, 2.0, 5.0, 0.0, 0.0, -1.0, 1.0],
        [0.0, 1.0, 2.0, 5.0, 0.0, 0.0, 1.0, 1.0],
        [-2.0, 1.0, 0.0, 5.0, 1.0, 0.0, 0.0, 1.0],
        [1.0, 2.0, 2.0, 5.0, 0.0, 0.0, -1.0, 1.0],
    ];
    let actual = run("verify_scene", &inputs, 6 * 32)?;
    assert_eq!(
        actual[2].to_bits(),
        1.0_f32.to_bits(),
        "finite planes are absent from GPU dispatch"
    );
    let kind = Kind::from_tag(5).ok_or("missing finite plane")?;
    let object = Object::new(7, kind, [0.0, 1.0, 0.0], 1.0, 0.0)?;
    for (index, input) in inputs.into_iter().enumerate() {
        let ray = Ray {
            o: [0, 1, 2].map(|i| f64::from(input[i])),
            d: [4, 5, 6].map(|i| f64::from(input[i])),
        };
        let expected = query_object(&object, ray)?.packed(ray);
        for field in 0..24 {
            let value = f64::from(actual[index * 32 + field]);
            assert!(
                value.is_finite() && (value - expected[field]).abs() < 2e-5,
                "fixture{index} field{field}: {value} != {}",
                expected[field]
            );
        }
    }
    Ok(())
}

#[test]
fn finite_gpu_threaded_bounds_never_truncate_large_finite_ray_distances()
-> Result<(), Box<dyn std::error::Error>> {
    let inputs = [
        [1e31, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0],
        [2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, -2.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        [1e31, 2.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0],
    ];
    let actual = run("verify_finite_branch", &inputs, inputs.len() * 16)?;
    for (row, expected) in actual.chunks_exact(16).zip([true, false, true, false]) {
        assert_eq!(
            row[0] > 0.5,
            expected,
            "threaded bounds lost a possible finite contact: {row:?}"
        );
    }
    Ok(())
}
#[test]
fn finite_gpu_intervals_enclose_f64_arithmetic() -> Result<(), Box<dyn std::error::Error>> {
    let inputs = [
        [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [f32::from_bits(1), 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.125, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [1e-20, 1e-20, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [-1e-20, 1e-20, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [1e10, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [1e30, 1e30, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ];
    let actual = run("verify_finite_interval", &inputs, inputs.len() * 16)?;
    for (row, input) in actual.chunks_exact(16).zip(inputs) {
        let a = f64::from(input[0]);
        let b = f64::from(input[1]);
        for (index, expected) in [a + b, a * b, a / b].into_iter().enumerate() {
            let valid = !(index == 1 && a > 1e25);
            assert_eq!(row[index * 3 + 2] > 0.5, valid);
            if valid {
                assert!(
                    f64::from(row[index * 3]) <= expected
                        && f64::from(row[index * 3 + 1]) >= expected,
                    "GPU interval excludes {expected}: {:?}",
                    &row[index * 3..index * 3 + 3]
                );
            }
        }
    }
    Ok(())
}
