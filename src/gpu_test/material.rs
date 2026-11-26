use super::support::{run, run_material_origin, run_scene};
use crate::scene::{model::parse_objects, packed::pack};

#[test]
fn material_relief_and_color_do_not_follow_camera_phase() -> Result<(), Box<dyn std::error::Error>>
{
    let mut inputs = Vec::new();
    for material in [0.0, 1.0] {
        for [x, y] in [[0.105, 0.035], [0.105, 0.0], [0.117, 0.073]] {
            for [px, py, pz, angle] in [[0.0; 4], [1.0, 1.0, 0.0, 0.73]] {
                inputs.push([x, y, 0.1, material, px, py, pz, angle]);
            }
        }
    }
    let expected = run_material_origin("verify_material_anchor", &inputs, [0.0; 3])?;
    assert!(expected.iter().all(|v| v.is_finite()));
    for origin in [[0.02, -0.01, 0.23], [-0.3, 0.4, 1.6], [3.0, 1.0, 2.0]] {
        let actual = run_material_origin("verify_material_anchor", &inputs, origin)?;
        for (channel, (&a, &b)) in actual.iter().zip(&expected).enumerate() {
            assert!(
                a.is_finite() && (a - b).abs() < 0.000_001,
                "camera {origin:?} moved material sample {} channel {}: {a} != {b}",
                channel / 16,
                channel % 16
            );
        }
    }
    Ok(())
}
#[test]
fn diffuse_environment_retains_normals_when_the_key_light_faces_away()
-> Result<(), Box<dyn std::error::Error>> {
    let input = [
        [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ];
    let q = run("verify_material_fill", &input, input.len() * 16)?;
    assert!(q.iter().all(|v| v.is_finite()));
    assert!(
        q[0] > q[16] + 0.3 && q[32] > q[48] + 0.3 && q[64] > q[80] + 0.1,
        "ambient diffuse erases material normals: {q:?}"
    );
    for sample in q.chunks_exact(16) {
        assert!(sample[..3].iter().all(|v| (0.05..0.59).contains(v)));
    }
    Ok(())
}

#[test]
fn stochastic_variance_preserves_relief_instead_of_clipping_flat_pore_floors()
-> Result<(), Box<dyn std::error::Error>> {
    let input = [
        [0.02, 0.04, 0.5, 0.5, 0.0, 0.0, 0.0, 0.0],
        [0.03, 0.05, 0.5, 0.5, 0.0, 0.0, 0.0, 0.0],
        [0.96, 0.98, 0.5, 0.5, 0.0, 0.0, 0.0, 0.0],
    ];
    let q = run("verify_material_variance", &input, input.len() * 16)?;
    assert!(q.iter().all(|v| v.is_finite()));
    assert!(
        q[3] < -0.15 && q[35] > 1.15,
        "relief extrema flattened: {}, {}",
        q[3],
        q[35]
    );
    assert!(
        0.01_f32
            .mul_add(-std::f32::consts::SQRT_2, q[19] - q[3])
            .abs()
            < 1e-6
    );
    for sample in q.chunks_exact(16) {
        assert!(sample[..3].iter().all(|v| v.to_bits() == 0.0_f32.to_bits()));
    }
    Ok(())
}

#[test]
fn brick_ray_hits_recess_mortar_below_faces_without_inventing_hits()
-> Result<(), Box<dyn std::error::Error>> {
    let inputs = [
        [0.105, 0.035, 0.1, 1.0, 0.0, 0.0, -1.0, 5.0], // brick face
        [0.105, 0.0, 0.1, 1.0, 0.0, 0.0, -1.0, 5.0],   // mortar
        [0.105, 0.0, 0.1, 0.0, 0.0, 0.0, -1.0, 5.0],   // steel height uses the same resolver
        [1.1, 0.0, 0.1, 1.0, 0.0, 0.0, -1.0, 5.0],     // finite miss
        [0.0, 0.1, 0.02, 1.0, 0.6, 0.0, -0.8, 5.0],    // oblique
        [0.105, 0.0, -0.1, 1.0, 0.0, 0.0, 1.0, 5.0],   // reverse
        [0.0, 0.0, 1.1, 1.0, 0.0, 0.0, -1.0, 1.0],     // curved
        [-0.8, 0.0, 0.01, 1.0, 1.0, 0.0, -0.015, 5.0], // grazing
    ];
    let rows = run("verify_material_hit", &inputs, inputs.len() * 16)?;
    let row = |i: usize| &rows[16 * i..16 * (i + 1)];
    assert!(row(0)[0] > 0.5 && row(0)[1] > row(0)[2]);
    assert!(row(1)[1] > row(0)[1] + 0.0025, "mortared joint stayed flat");
    assert!(row(2)[0] > 0.5 && (row(2)[1] - row(2)[2]).abs() < 0.0001);
    assert!(
        row(3)[0] < 0.5,
        "displacement invented a hit outside the panel"
    );
    assert!((row(1)[1] + row(5)[1] - 0.2).abs() < 2e-6);
    assert!(
        row(6)[0] > 0.5 && row(6)[1] > row(6)[2],
        "curved texh {rows:?}"
    );
    assert!(
        row(7)[0] < 0.5 || (row(7)[1] - row(7)[2]).abs() > 1e-4,
        "grazing rays silently fell back to the flat carrier"
    );
    for i in [0, 1, 4, 5, 6] {
        let input = inputs[i];
        let result = row(i);
        assert!(result.iter().all(|v| v.is_finite()));
        for axis in 0..3 {
            let p = input[4 + axis].mul_add(result[1], input[axis]);
            assert!((result[4 + axis] - p).abs() < 2e-6);
        }
    }
    assert!((row(4)[8] - 0.5_f32.mul_add(row(4)[4], 0.5)).abs() < 1e-6);
    assert!((row(4)[1] - row(4)[2]).abs() > 1e-4);
    Ok(())
}

#[test]
fn material_display_encodes_linear_light_for_the_unorm_srgb_canvas()
-> Result<(), Box<dyn std::error::Error>> {
    let input = [
        [0.18, 0.003_130_8, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [-1.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ];
    let q = run("verify_material_display", &input, 32)?;
    for (i, expected) in [
        (0, 0.461_356),
        (1, 0.040_45),
        (2, 1.0),
        (16, 0.0),
        (17, 0.0),
        (18, 1.0),
    ] {
        assert!(
            (q[i] - expected).abs() < 1e-5,
            "linear output bypassed display encoding: {}",
            q[i]
        );
    }
    Ok(())
}
#[test]
fn steel_has_directional_highlights_but_brick_stays_isotropic()
-> Result<(), Box<dyn std::error::Error>> {
    let inputs = [
        [0.0, 0.0, 1.0, 0.0, 0.6, 0.0, 0.8, 0.0],
        [0.0, 0.0, 1.0, 0.0, 0.0, 0.6, 0.8, 0.0],
        [0.0, 0.0, 1.0, 1.0, 0.6, 0.0, 0.8, 0.0],
        [0.0, 0.0, 1.0, 1.0, 0.0, 0.6, 0.8, 0.0],
        [0.0, 0.0, 0.0, 0.0, 0.6, 0.0, 0.8, 0.0],
        [0.0, 0.0, 1.0, 0.0, 0.6, 0.0, 0.8, 0.0],
    ];
    let q = run("verify_studio_brushing", &inputs, inputs.len() * 16)?;
    assert!(q.iter().all(|v| v.is_finite()));
    assert!(
        (q[0] - q[16]).abs() > 0.02,
        "brushing is shaded as directionless blur"
    );
    for c in 0..3 {
        assert!((q[32 + c] - q[48 + c]).abs() < 1e-6);
    }
    assert!(
        (q[68] - q[84]).abs() > 0.003,
        "studio environment erases brush direction"
    );
    Ok(())
}
#[test]
fn medium_steel_already_samples_fine_orders_before_they_are_pixel_resolved()
-> Result<(), Box<dyn std::error::Error>> {
    let input = [[0.13, 0.032, 0.0, 0.0, 0.0, 0.0, 1.0, 0.000_226]];
    let q = run("verify_material", &input, 16)?;
    assert!(
        q[10] > 4.0 && q[10] < 5.0,
        "unexpected physical LOD: {}",
        q[10]
    );
    assert!(q[11] > 0.89, "finer order arrives too late: {}", q[11]);
    Ok(())
}
#[test]
fn material_mirrors_reflect_source_height_without_periodic_stamps()
-> Result<(), Box<dyn std::error::Error>> {
    let inputs = [
        [0.231, 0.319, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [1.769, 0.319, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [0.13, 0.08, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [1.13, 0.08, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ];
    let q = run("verify_material_mapping", &inputs, inputs.len() * 16)?;
    for c in 8..12 {
        assert!(
            (q[c] - q[16 + c]).abs() < 0.00003,
            "repeat was not mirrored"
        );
    }
    assert!(
        (0..3).any(|c| (q[32 + c] - q[48 + c]).abs() > 0.005),
        "same wall stamp repeated"
    );
    Ok(())
}
#[test]
fn tiled_relief_shows_no_period_or_mirror_axis() -> Result<(), Box<dyn std::error::Error>> {
    // Mirror tiling repeats every 2 tiles and reflects about integer lines.
    let partners = [[2.0_f32, 0.0, 1.0], [0.0, 2.0, 1.0], [2.0, 0.0, -1.0]];
    let mut levels = Vec::new();
    let mut inputs = Vec::new();
    for (index, level) in crate::material::presets_with_leaf(false).iter().enumerate() {
        if level.atlas.is_some() {
            continue;
        }
        levels.push(index);
        let layer = f32::from(u16::try_from(index)?);
        let mut n = 0.0_f32;
        for _ in 0..1024 {
            n += 1.0;
            let [u, v] = [
                (n * 0.754_877_7).fract() * 4.0,
                (n * 0.569_840_3).fract() * 4.0,
            ];
            inputs.push([u, v, layer, 0.0, 0.0, 0.0, 0.0, 0.0]);
            for [du, dv, sign] in partners {
                inputs.push([sign.mul_add(u, du), dv + v, layer, 0.0, 0.0, 0.0, 0.0, 0.0]);
            }
        }
    }
    let q = run("verify_material_mapping", &inputs, inputs.len() * 16)?;
    for (level, samples) in levels.iter().zip(q.chunks_exact(1024 * 64)) {
        for partner in 1..4 {
            let [mut n, mut sx, mut sy, mut sxx, mut syy, mut sxy] = [0.0_f64; 6];
            for s in samples.chunks_exact(64) {
                let (x, y) = (f64::from(s[3]), f64::from(s[16 * partner + 3]));
                n += 1.0;
                sx += x;
                sy += y;
                sxx = x.mul_add(x, sxx);
                syy = y.mul_add(y, syy);
                sxy = x.mul_add(y, sxy);
            }
            let correlation = n.mul_add(sxy, -sx * sy)
                / (n.mul_add(sxx, -sx * sx) * n.mul_add(syy, -sy * sy)).sqrt();
            assert!(
                correlation.abs() < 0.15,
                "level {level} repeats with partner {partner}: correlation {correlation}"
            );
        }
    }
    Ok(())
}
#[test]
fn material_mapping_is_continuous_across_camera_and_atlas_boundaries()
-> Result<(), Box<dyn std::error::Error>> {
    let mut inputs = vec![
        [0.231, 0.319, 7.0, 0.0, 1.0, 0.0, 0.0, 0.0],
        [1.231, 0.319, 7.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ];
    for layer in [3.0, 4.0, 7.0] {
        for lod in [0.0, 0.75, 3.7, 6.0] {
            for edge in [0.0, 0.5, 1.0] {
                inputs.push([edge - 1e-7, 0.08, layer, lod, 0.0, 0.0, 0.0, 0.0]);
                inputs.push([edge + 1e-7, 0.08, layer, lod, 0.0, 0.0, 0.0, 0.0]);
            }
            inputs.push([0.13, 1.0 / 6.0 - 1e-7, layer, lod, 0.0, 0.0, 0.0, 0.0]);
            inputs.push([0.13, 1.0 / 6.0 + 1e-7, layer, lod, 0.0, 0.0, 0.0, 0.0]);
        }
    }
    let q = run("verify_material_mapping", &inputs, inputs.len() * 16)?;
    for (index, pair) in q.chunks_exact(32).enumerate() {
        assert!(pair.iter().all(|v| v.is_finite()));
        for c in 0..6 {
            let delta = (pair[c] - pair[16 + c]).abs();
            // Mapping is independent of artistic relief gain; allow hardware filter quantization.
            let continuous = if c < 4 { delta < 0.001 } else { delta < 0.1 };
            assert!(
                continuous,
                "mapping seam: {:?}, channel {c}, delta {delta}",
                inputs[index * 2]
            );
        }
    }
    Ok(())
}
#[test]
fn material_filtered_gradient_follows_the_texel_width_transformed_scalar_field()
-> Result<(), Box<dyn std::error::Error>> {
    let mut inputs = Vec::new();
    for u in [-0.39, 0.39, 1.39] {
        for lod in [0.0_f32, 1.25, 3.0] {
            let step = lod.exp2() / 512.0;
            inputs.push([u, 0.319, 7.0, lod, 0.0, 0.0, 0.0, 0.0]);
            inputs.push([u + step, 0.319, 7.0, lod, 0.0, 0.0, 0.0, 0.0]);
            inputs.push([u - step, 0.319, 7.0, lod, 0.0, 0.0, 0.0, 0.0]);
        }
    }
    let q = run("verify_material_mapping", &inputs, inputs.len() * 16)?;
    for (i, group) in q.chunks_exact(48).enumerate() {
        let step = inputs[i * 3][3].exp2() / 512.0;
        let gradient = (group[19] - group[35]) / (2.0 * step);
        assert!(
            (group[4] - gradient).abs() < 0.003,
            "flipped height derivative has wrong sign"
        );
    }
    Ok(())
}
#[test]
fn material_and_selection_survive_shared_definition_resolution()
-> Result<(), Box<dyn std::error::Error>> {
    let objects = parse_objects(&[
        1.0, 2.0, 0.0, 0.0, 0.0, 1.0, 1.5, 0.0, 2.0, 2.0, 3.0, 0.0, 0.0, 1.0, 1.5, 1.0,
    ])?;
    let packet = pack(&objects, &[2])?;
    let index = packet
        .records
        .iter()
        .position(|r| r.identity[0] == 2)
        .ok_or("missing instance")?;
    let input = [[
        f32::from(u16::try_from(index)?),
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ]];
    let q = run_scene("verify_material_instance", &input, &packet.records)?;
    assert_eq!(q[0].to_bits(), 1.0_f32.to_bits());
    assert_eq!(q[1].to_bits(), 1.0_f32.to_bits());
    assert_eq!(q[2].to_bits(), 1.5_f32.to_bits());
    Ok(())
}
#[test]
fn brick_micropores_retain_cavity_shadow_not_flat_painted_dots()
-> Result<(), Box<dyn std::error::Error>> {
    // Locate cavities after stochastic mapping, not at an untransformed asset pixel.
    let input: Vec<_> = (0_u8..32)
        .flat_map(|y| {
            (0_u8..32).map(move |x| {
                [
                    0.000_42 * (250.0 + (f32::from(x) + 0.5) / 32.0),
                    0.000_42 * (84.0 + (f32::from(y) + 0.5) / 32.0),
                    0.0,
                    1.0,
                    0.0,
                    0.0,
                    1.0,
                    1e-8,
                ]
            })
        })
        .collect();
    let q = run("verify_material", &input, input.len() * 16)?;
    let mut darkest = 1.0_f32;
    for sample in q.chunks_exact(16) {
        assert!(sample.iter().all(|v| v.is_finite()));
        darkest = darkest.min(sample[9]);
    }
    assert!(
        darkest > 0.0 && darkest < 0.15,
        "micropores remain bright painted dots: {darkest}"
    );
    Ok(())
}

#[test]
fn actual_material_tiles_are_filtered_finite_and_keep_finer_detail()
-> Result<(), Box<dyn std::error::Error>> {
    let mut inputs = Vec::new();
    for material in [0.0_f32, 1.0] {
        for footprint in [0.01_f32, 0.0001, 0.000_001, 0.000_000_01] {
            for step in [0.0_f32, 0.000_002, 0.000_019] {
                inputs.push([0.117, 0.073 + step, 0.0, material, 0.0, 0.0, 1.0, footprint]);
            }
        }
    }
    let actual = run("verify_material", &inputs, inputs.len() * 16)?;
    for (input, q) in inputs.iter().zip(actual.chunks_exact(16)) {
        assert!(q.iter().all(|v| v.is_finite()));
        assert!(q[..3].iter().all(|v| *v > 0.0 && *v <= 1.0));
        assert!(
            q[12..15].iter().all(|v| *v > 0.0 && *v <= 1.0),
            "material lighting is absent"
        );
        assert!((0.05..=0.98).contains(&q[3]));
        let norm = q[4].mul_add(q[4], q[5].mul_add(q[5], q[6] * q[6])).sqrt();
        assert!((norm - 1.0).abs() < 0.0001);
        assert!(q[6] > 0.25);
        assert_eq!(q[7].to_bits(), (1.0 - input[3]).to_bits());
        assert_eq!(q[8].to_bits(), 8.0_f32.to_bits());
    }
    for material in 0..2 {
        let start = (material * 12 + 9) * 16;
        assert!(
            actual[start..start + 16]
                .iter()
                .zip(&actual[start + 16..start + 32])
                .any(|(a, b)| (a - b).abs() > 0.00001),
            "microscopic texture is flat"
        );
    }
    Ok(())
}
