use super::support::{run, run_field, used_bindings};
use crate::heightfield::Field;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn carrier_flag_has_no_metal_partial_word_padding() -> TestResult {
    let module = naga::front::wgsl::parse_str(&crate::shader::source())?;
    let sample = module
        .types
        .iter()
        .find(|(_, ty)| ty.name.as_deref() == Some("SceneSample"))
        .ok_or("missing scene sample")?
        .1;
    let naga::TypeInner::Struct { members, span } = &sample.inner else {
        return Err("scene sample is not a struct".into());
    };
    let flag = members
        .iter()
        .find(|member| member.name.as_deref() == Some("base_hit"))
        .ok_or("missing carrier flag")?;
    assert_eq!(*span, flag.offset + 4);
    assert!(matches!(
        module.types[flag.ty].inner,
        naga::TypeInner::Scalar(naga::Scalar {
            kind: naga::ScalarKind::Uint,
            width: 4
        })
    ));
    Ok(())
}

#[test]
fn material_bumps_keep_the_carrier_side_across_view_tangencies() -> TestResult {
    let mut rays = Vec::new();
    let mut expected = Vec::new();
    for kind in 1_u8..=5 {
        for side in [-1.0, 1.0] {
            for axis in 0..2 {
                for z in [-0.0001, 0.0001] {
                    let mut ray = [0.0; 8];
                    ray[0] = f32::from(kind);
                    ray[1 + axis] = 0.6 * side;
                    ray[3] = -0.8 * side;
                    ray[4 + axis] = 1.0;
                    ray[6] = z;
                    rays.push(ray);
                    expected.push((axis, if kind >= 4 { side } else { 1.0 }));
                }
            }
        }
    }
    let q = run("verify_crystal_render", &rays, rays.len() * 16)?;
    for (i, (row, (axis, side))) in q.chunks_exact(16).zip(expected).enumerate() {
        assert!(row[axis + 1] * side > 0.9999, "flipped bump {i}: {row:?}");
        assert!(row[3].abs() < 0.0002, "lost detail {i}: {row:?}");
    }
    Ok(())
}

#[test]
fn close_brick_relief_draws_shown_spots_near_their_own_depth() -> TestResult {
    let (sin, cos) = 0.87_f32.sin_cos();
    let mut rays = Vec::new();
    for (x, y) in [-0.3_f32, -0.1, 0.1, 0.3]
        .into_iter()
        .flat_map(|x| [-0.3_f32, -0.1, 0.1, 0.3].map(|y| (x, y)))
    {
        for (a, b) in [-0.15_f32, -0.05, 0.05, 0.15]
            .into_iter()
            .flat_map(|a| [-0.15_f32, -0.05, 0.05, 0.15].map(|b| (a, b)))
        {
            rays.push([x, y, 0.005, 1.0, sin + a, b, -cos, 5.0]);
        }
    }
    let q = run("verify_texh_borrow", &rays, rays.len() * 16)?;
    let mut error: Vec<f32> = q
        .chunks_exact(16)
        .filter(|row| row[0] > 0.5)
        .map(|row| (row[1] - row[2]).abs())
        .collect();
    assert!(
        error.len() == rays.len(),
        "missed brick rays: {}",
        error.len()
    );
    error.sort_by(f32::total_cmp);
    let (median, tail) = (error[error.len() / 2], error[error.len() * 9 / 10]);
    assert!(
        median < 7.5e-5,
        "shown spots borrow depth from elsewhere: median {median} m, p90 {tail} m"
    );
    Ok(())
}

#[test]
fn relief_normal_stencils_bound_byte_error_without_blurring_color() -> TestResult {
    let fine = (0.0064 / 0.042) * std::f32::consts::SQRT_2 / 255.0;
    let coarse = (0.125 * 0.5) / 255.0;
    let rays = [
        [1.0 / 512.0, fine, 0.17, 0.31, 5.0, 0.0, 0.0, 0.0],
        [1.0 / 512.0, -fine, 0.13, 0.53, 5.0, 0.0, 0.0, 0.0],
        [1.0 / 512.0, coarse, 0.71, 0.77, 4.0, 0.0, 0.0, 0.0],
        [1.0 / 512.0, 0.0, 0.32, 0.27, 4.0, 0.0, 0.0, 0.0],
        [1.0 / 32.0, fine, 0.14, 0.88, 5.0, 4.0, 0.0, 0.0],
        [1.0 / 512.0, fine, 0.13, 0.23, 2.0, 0.0, 0.0, 0.0],
        [0.5, fine, 0.17, 0.31, 5.0, 9.0, 0.0, 0.0],
    ];
    let q = run("verify_slope_precision", &rays, rays.len() * 16)?;
    let mut changed = false;
    for (row, ray) in q.chunks_exact(16).zip(rays) {
        let support = row[0];
        assert!(support >= ray[0]);
        assert!(support <= ray[0].max(16.0 * ray[1].abs()) * (1.0 + 1e-6));
        assert!(
            ray[1].abs() / (2.0 * support) <= 1.0 / 32.0 + 1e-6,
            "quantized height still jumps the shading slope: {ray:?}, {row:?}"
        );
        assert_eq!(
            row[4..8].iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            row[8..12].iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            "normal precision changed color or height"
        );
        changed |= (row[12] - row[13]).abs() > 1e-3;
    }
    assert!(changed, "shading still uses the raw height differential");
    Ok(())
}

#[test]
fn shading_footprint_covers_direct_relief_depth_and_warp() -> TestResult {
    let rays = [
        [0.001, -0.006, 0.6, 0.0, 0.001, 0.0, 0.0, 0.0],
        [0.001, -0.006, 0.6, 2.0, 0.001, 0.0, 0.0, 0.0],
        [0.001, 0.006, -0.6, -2.0, 0.001, 3.0, 0.0, 0.0],
        [0.01, 0.0, 1.0, 20.0, 0.001, 0.0, 0.0, 0.0],
        [0.001, -0.006, 0.02, 2.0, 0.001, 3.0, 0.0, 0.0],
        [0.001, -0.000_006, 0.002, 2.0, 0.001, 3.0, 0.0, 0.0],
        [0.001, 0.0001, 0.6, 0.0, 0.001, 0.0, 0.0, 0.0],
        [0.001, -0.006, 0.6, 2.0, 0.0, 0.0, 0.0, 0.0],
    ];
    let q = run("verify_texh_filter", &rays, rays.len() * 16)?;
    for (i, (row, ray)) in q.chunks_exact(16).zip(rays).enumerate() {
        let [distance, height, cosine, slope_x, pixel, slope_y, ..] = ray.map(f64::from);
        let sine = cosine.mul_add(-cosine, 1.0).max(0.0).sqrt();
        let direction = [sine, 0.0, -cosine];
        // Independent finite differences of the direct map of a sloped height field.
        let map = |vector: [f64; 3]| {
            let length = vector.iter().map(|v| v * v).sum::<f64>().sqrt();
            let vector = vector.map(|v| v / length);
            let carrier = -distance * cosine / vector[2];
            let height = (slope_y * carrier).mul_add(
                vector[1],
                slope_x.mul_add(distance.mul_add(-sine, carrier * vector[0]), height),
            );
            let depth = height.mul_add(vector[2] / (vector[2] * vector[2]).max(0.0025), carrier);
            [depth * vector[0], depth * vector[1]]
        };
        let difference = |axis: [f64; 3]| {
            let plus = map(std::array::from_fn(|j| axis[j].mul_add(1e-6, direction[j])));
            let minus = map(std::array::from_fn(|j| {
                axis[j].mul_add(-1e-6, direction[j])
            }));
            [(plus[0] - minus[0]) / 2e-6, (plus[1] - minus[1]) / 2e-6]
        };
        let dx = difference([cosine.abs(), 0.0, cosine.signum() * sine]);
        let dy = difference([0.0, 1.0, 0.0]);
        let xx = dx[1].mul_add(dx[1], dx[0] * dx[0]);
        let xy = dx[1].mul_add(dy[1], dx[0] * dy[0]);
        let yy = dy[1].mul_add(dy[1], dy[0] * dy[0]);
        let stretch = (0.5 * (xx + yy + (4.0 * xy).mul_add(xy, (xx - yy).powi(2)).sqrt())).sqrt();
        let depth = height.mul_add(-cosine / (cosine * cosine).max(0.0025), distance);
        let expected = (pixel * stretch)
            .max(pixel * distance.max(depth) / cosine.abs().max(0.01))
            .max(1e-30);
        assert!(
            (f64::from(row[0]) / expected - 1.0).abs() < 1e-5,
            "wrong direct-warp filter {i}: {row:?}, expected {expected}"
        );
    }
    Ok(())
}

fn data(height: [u8; 4], coverage: [u8; 4]) -> [u8; 40] {
    let mut result = [0; 40];
    for (i, pixels) in [height, [128; 4], [128; 4], [128; 4], coverage]
        .into_iter()
        .enumerate()
    {
        let at = i * 8;
        result[at..at + 4].copy_from_slice(&pixels);
        result[at + 4] = *pixels.iter().min().unwrap_or(&0);
        result[at + 5] = *pixels.iter().max().unwrap_or(&255);
    }
    result
}

#[test]
#[expect(
    clippy::cast_possible_truncation,
    reason = "World reference rays narrow once at the GPU boundary"
)]
fn leaf_envelope_culls_only_irrelevant_rays_and_depths() -> TestResult {
    let cases = [
        ([1.5, 0.0, 1.0], [0.0, 0.0, -1.0], 4.0, false),
        ([0.2, 0.3, 1.0], [0.0, 0.0, -1.0], 4.0, true),
        ([0.2, 0.3, -1.0], [0.0, 0.0, -1.0], 4.0, false),
        ([1.02, 0.0, 0.02], [0.0, 0.0, -1.0], 4.0, true),
        ([0.2, 0.3, 0.0001], [1.0, 0.0, -0.0001], 4.0, true),
        ([0.2, 0.3, 0.02], [1.0, 0.0, 0.0], 4.0, true),
        ([0.2, 0.3, 1.0], [0.0, 0.0, -1.0], 0.5, false),
        ([1.08, 0.0, 20.0], [0.0, 0.0, -1.0], 40.0, false),
    ];
    let mut rays = Vec::new();
    for mode in 0_u8..18 {
        let pose = mode % 9;
        let angle: f64 = if pose >= 3 { 0.73 } else { 0.0 };
        let (sin, cos) = angle.sin_cos();
        let size = if pose >= 6 { 0.035 } else { 0.07 };
        let transform = |p: [f64; 3]| {
            let [x, y, z] = match mode % 3 {
                1 => [p[2], p[0], p[1]],
                2 => [p[1], p[2], p[0]],
                _ => p,
            };
            [sin.mul_add(z, cos * x), y, sin.mul_add(-x, cos * z)]
        };
        for (o, d, limit, _) in cases {
            let o = transform(o).map(|v| v * size);
            let d = transform(d);
            rays.push([
                (1.0 + o[0]) as f32,
                (1.0 + o[1]) as f32,
                o[2] as f32,
                f32::from(mode),
                d[0] as f32,
                d[1] as f32,
                d[2] as f32,
                (limit * size) as f32,
            ]);
        }
    }
    let q = run("verify_leaf_culling", &rays, rays.len() * 16)?;
    for (i, row) in q.chunks_exact(16).enumerate() {
        let selected = i / cases.len() >= 9;
        let case = i % cases.len();
        let outline = selected && case == 7;
        let edge_miss = !selected && matches!(case, 3 | 4);
        assert_eq!(
            row[0] > 0.5,
            (cases[case].3 && !edge_miss) || outline,
            "case {i}: {row:?}"
        );
        if outline {
            assert!(row[4] > 0.5, "missing original outline: {i}: {row:?}");
        }
        assert!(
            row[0] > 0.5 || row[1] < 0.5 || row[2] > rays[i][7],
            "culled a nearer hit: {i}: {row:?}"
        );
    }
    let bytes = data([255; 4], [255; 4]);
    let field = Field::new(&bytes, 2, [0.125, 0.0, 0.0, 0.0], [0.0; 2])?;
    let q = run_field("verify_leaf_culling", &rays[3..5], 32, field)?;
    assert!(
        q[0] > 0.5 && q[16] > 0.5,
        "opaque boundary was culled: {q:?}"
    );
    Ok(())
}

#[test]
fn leaf_height_orders_wait_for_visible_bounded_amplitude() -> TestResult {
    let bound = (1.0_f32 / 32768.0) * (128.0 / 255.0);
    let spans = [0.01, bound * 8.0, bound * 3.0, bound * 2.0, 0.0];
    let inputs: Vec<_> = spans
        .into_iter()
        .map(|span| [0.317, 0.419, span, 0.0, 0.0, 0.0, 0.0, 0.0])
        .collect();
    let q = run("verify_height_lod", &inputs, inputs.len() * 16)?;
    for row in q.chunks_exact(16) {
        assert!(row.iter().all(|v| v.is_finite()));
        assert_eq!(row[0].to_bits(), 1_f32.to_bits());
    }
    assert_eq!(
        &q[1..4],
        &[0.0; 3],
        "subpixel height orders loaded too early"
    );
    assert_eq!(q[19].to_bits(), 0);
    assert!(q[35] > 0.0 && q[35] < 1.0, "detail must fade smoothly");
    assert_eq!(q[51].to_bits(), 1_f32.to_bits());
    assert_eq!(&q[65..68], &[1.0; 3], "full close-up strength lost");
    assert!((q[68] - q[69]).abs() < 1e-7);
    assert!(
        (q[4] - q[5]).abs() > 1e-7,
        "far height still evaluates the complete field"
    );
    let bytes = data([0; 4], [255; 4]);
    let amplitudes = [0.125, 1.0 / 1024.0, 1.0 / 8192.0, 1.0 / 32768.0];
    let field = Field::new(&bytes, 2, amplitudes, [0.0; 2])?;
    let q = run_field(
        "verify_height_lod",
        &[[0.6, 0.65, 0.0, 0.08, 0.01, 0.0, 0.0, 0.0]],
        16,
        field,
    )?;
    let remaining = amplitudes[1..].iter().sum::<f64>() * (128.0 / 255.0);
    let expected = 0.01 * (0.08 + 0.125 * 128.0 / 255.0 - remaining);
    assert!(
        (f64::from(q[8]) - expected).abs() < 1e-7,
        "macro envelope admitted fine heights too early: {q:?}"
    );
    Ok(())
}

#[test]
fn displacement_and_shading_are_direct_loop_free_and_nonrecursive() -> TestResult {
    let module = naga::front::wgsl::parse_str(&crate::shader::source())?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)?; // WGSL validation rejects recursion.
    let mut functions: Vec<_> = module
        .functions
        .iter()
        .filter_map(|(handle, function)| {
            matches!(
                function.name.as_deref(),
                Some("material_texh" | "texh_surface")
            )
            .then_some((handle, false))
        })
        .collect();
    assert_eq!(functions.len(), 2);
    let mut seen = std::collections::BTreeSet::new();
    while let Some((handle, candidate)) = functions.pop() {
        let function = &module.functions[handle];
        let candidate = candidate || function.name.as_deref() == Some("material_texh_depth");
        if !seen.insert((handle, candidate)) {
            continue;
        }
        assert!(
            !candidate
                || !matches!(
                    function.name.as_deref(),
                    Some(
                        "hf_slope"
                            | "hf_gradient_lod"
                            | "material_layers"
                            | "layered_material"
                            | "leaf_material"
                    )
                ),
            "candidate shades before nearest-hit selection: {:?}",
            function.name
        );
        let mut blocks = vec![&function.body];
        while let Some(block) = blocks.pop() {
            for statement in block {
                match statement {
                    naga::Statement::Loop { .. } => {
                        return Err(format!("loop in {:?}", function.name).into());
                    }
                    naga::Statement::Block(body) => blocks.push(body),
                    naga::Statement::If { accept, reject, .. } => blocks.extend([accept, reject]),
                    naga::Statement::Switch { cases, .. } => {
                        blocks.extend(cases.iter().map(|case| &case.body));
                    }
                    naga::Statement::Call { function, .. } => {
                        functions.push((*function, candidate));
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

#[test]
fn render_keeps_the_exact_live_host_binding_contract() -> TestResult {
    assert_eq!(
        used_bindings("draw")?,
        std::collections::BTreeSet::from([0, 1, 5, 6, 7, 8, 9, 10, 11])
    );
    Ok(())
}

#[test]
fn direct_texh_uses_the_carrier_height_without_searching_for_a_root() -> TestResult {
    let bytes = data([128, 255, 128, 255], [255; 4]);
    let field = Field::new(&bytes, 2, [255.0 / 1024.0, 0.0, 0.0, 0.0], [0.0; 2])?;
    let q = run_field(
        "verify_material_hit",
        &[[-0.6, 0.0, 0.3, 2.0, 0.6, 0.0, -0.8, 5.0]],
        16,
        field,
    )?;
    let height = field.uv([0.3125, 0.5])?.height;
    let expected = 0.375 - height / 0.8;
    assert!(
        q[0] > 0.5 && (f64::from(q[1]) - expected).abs() < 2e-6,
        "{q:?}"
    );
    assert!((q[8] - 0.5_f32.mul_add(q[4], 0.5)).abs() < 1e-6);
    assert!(q[12] < -0.05 && q[14] > 0.98);
    Ok(())
}

#[test]
fn texh_shares_shifted_uv_rgb_and_normals_without_geometry_switches() -> TestResult {
    let source = include_str!("../shaders/materials/texh.wgsl");
    assert!(
        ["plane_", "sphere_", "goursat_", "crystal_", "identity.y"]
            .iter()
            .all(|name| !source.contains(name))
    );
    let bytes = data([128, 255, 128, 255], [255; 4]);
    let field = Field::new(&bytes, 2, [255.0 / 1024.0, 0.0, 0.0, 0.0], [0.0; 2])?;
    let rays = [[-0.6, 0.0, 0.3, 2.0, 0.6, 0.0, -0.8, 5.0]];
    let leaf = run_field("verify_material_color", &rays, 16, field)?;
    let brick = run(
        "verify_material_color",
        &[[0.0, 0.1, 0.02, 1.0, 0.6, 0.0, -0.8, 5.0]],
        16,
    )?;
    for color in [&leaf, &brick] {
        assert!(color[3] > 0.5, "{color:?}");
        for i in 0..3 {
            assert!((color[i] - color[4 + i]).abs() < 2e-5, "{color:?}");
        }
        assert!((color[12] - color[14]).abs() > 1e-5, "{color:?}");
    }
    Ok(())
}

#[test]
fn displaced_leaf_normals_blend_across_bilinear_texel_boundaries() -> TestResult {
    let bytes = data([128, 255, 128, 255], [255; 4]);
    let field = Field::new(&bytes, 2, [255.0 / 1024.0, 0.0, 0.0, 0.0], [0.0; 2])?;
    let rays = [
        [-1.0 / 1024.0, 0.0, 0.3, 2.0, 0.0, 0.0, -1.0, 5.0],
        [1.0 / 1024.0, 0.0, 0.3, 2.0, 0.0, 0.0, -1.0, 5.0],
    ];
    let q = run_field("verify_material_hit", &rays, 32, field)?;
    assert!(q[0] > 0.5 && q[16] > 0.5, "{q:?}");
    assert!(
        q[12] < -0.02 && q[28] < -0.02,
        "lost interpolated slope: {q:?}"
    );
    let jump = (q[12] - q[28]).hypot(q[13] - q[29]).hypot(q[14] - q[30]);
    assert!(jump < 0.001, "faceted normal jump: {jump}; {q:?}");
    Ok(())
}
#[test]
fn direct_texh_preserves_zero_and_constant_height_on_distinct_carriers() -> TestResult {
    let rays = [
        [0.0, 0.0, 2.0, 2.0, 0.0, 0.0, -1.0, 1.0],
        [0.3, 1.0, 0.2, 2.0, 0.0, -1.0, 0.0, 3.0],
        [0.2, 0.3, 1.0, 2.0, 0.0, 0.0, -1.0, 5.0],
        [1.0, 1.0, 3.0, 2.0, 0.0, 0.0, -1.0, 2.0],
        [1.0, 1.0, 3.0, 2.0, 0.0, 0.0, -1.0, 2.25],
        [1.0, 1.0, 3.0, 2.0, 0.0, 0.0, -1.0, 2.0625],
    ];
    let bytes = data([160; 4], [255; 4]);
    for amplitude in [0.0, 255.0 / 32768.0] {
        let field = Field::new(&bytes, 2, [amplitude, 0.0, 0.0, 0.0], [0.0; 2])?;
        for q in run_field("verify_material_hit", &rays, rays.len() * 16, field)?.chunks_exact(16) {
            assert!(
                q.iter().all(|v| v.is_finite()) && q[0] > 0.5 && q[11] > 0.5,
                "{q:?}"
            );
            assert!(
                (f64::from(q[2] - q[1]) - 32.0 * amplitude / 255.0).abs() < 5e-5,
                "{q:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn leaf_direct_depth_keeps_every_centered_height_band() -> TestResult {
    let mut bytes = data([133; 4], [255; 4]);
    for (order, value) in [133, 137, 147, 157].into_iter().enumerate() {
        bytes[order * 8..order * 8 + 6].fill(value);
    }
    let field = Field::new(
        &bytes,
        2,
        [0.125, 1.0 / 1024.0, 1.0 / 8192.0, 1.0 / 32768.0],
        [0.0; 2],
    )?;
    let expected = field.uv([0.6, 0.65])?.height;
    let q = run_field(
        "verify_material_hit_full",
        &[[0.2, 0.3, 1.0, 2.0, 0.0, 0.0, -1.0, 5.0]],
        16,
        field,
    )?;
    assert!(
        q[0] > 0.5 && (f64::from(q[6]) - expected).abs() < 1e-6,
        "{q:?}"
    );
    Ok(())
}

#[test]
fn leaf_source_depth_uses_the_carrier_sample_and_preserves_cutout() -> TestResult {
    let rays = [
        [0.012, -0.217, 1.0, 2.0, 0.0, 0.0, -1.0, 5.0],
        [-25.0 / 256.0, 114.0 / 256.0, 1.0, 2.0, 0.0, 0.0, -1.0, 5.0],
        [0.012, -0.217, -1.0, 2.0, 0.0, 0.0, 1.0, 5.0],
        [-0.1, 0.0, 0.5, 2.0, 0.3, 0.1, -1.0, 5.0],
    ];
    let rows = run("verify_material_hit", &rays, rays.len() * 16)?;
    for (ray, q) in rays.iter().zip(rows.chunks_exact(16)) {
        let length = f64::from(ray[4])
            .hypot(f64::from(ray[5]))
            .hypot(f64::from(ray[6]));
        let uv = std::array::from_fn(|axis| {
            (f64::from(ray[axis + 4]) / length)
                .mul_add(f64::from(q[2]), f64::from(ray[axis]))
                .mul_add(0.5, 0.5)
        });
        let source = crate::heightfield::LEAF.uv(uv)?;
        let covered = crate::heightfield::LEAF.uv([f64::from(q[8]), f64::from(q[9])])?;
        let incidence = f64::from(ray[6]) / length;
        let expected = f64::from(q[2]) + source.height / incidence;
        assert!(
            q[0] > 0.5 && q[11] > 0.5 && covered.coverage >= 127.5,
            "{q:?}"
        );
        assert!((f64::from(q[1]) - expected).abs() < 5e-5, "{q:?}");
    }
    Ok(())
}

#[test]
fn direct_texh_leaves_misses_alone_and_rejects_an_uncovered_or_backward_candidate() -> TestResult {
    let rays = [[1.1, 0.0, 1.0, 2.0, 0.0, 0.0, -1.0, 5.0]];
    let q = run("verify_material_hit", &rays, 16)?;
    assert!(q[11] < 0.5 && q[0].to_bits() == q[15].to_bits(), "{q:?}");
    assert_eq!(q[1].to_bits(), q[2].to_bits());
    for (coverage, origin) in [([255, 0, 0, 0], 0.3), ([255; 4], 0.01)] {
        let bytes = data([255; 4], coverage);
        let field = Field::new(&bytes, 2, [0.125, 0.0, 0.0, 0.0], [-1.0; 2])?;
        let q = run_field(
            "verify_material_hit",
            &[[0.0, 0.0, origin, 2.0, 0.0, 0.0, -1.0, 5.0]],
            16,
            field,
        )?;
        assert!(
            q[11] > 0.5 && q[0] < 0.5 && q[3].to_bits() == 0.0_f32.to_bits(),
            "{q:?}"
        );
    }
    Ok(())
}

#[test]
fn direct_texh_fades_grazing_offsets_instead_of_searching() -> TestResult {
    let bytes = data([160; 4], [255; 4]);
    let field = Field::new(&bytes, 2, [255.0 / 1024.0, 0.0, 0.0, 0.0], [0.0; 2])?;
    let q = run_field(
        "verify_material_hit",
        &[[0.2, 0.1, 0.25, 2.0, 1.0, -0.0001, 0.0, 3.0]],
        16,
        field,
    )?;
    let expected = q[10] * (-0.0001 / 0.0025);
    assert!(q[0] > 0.5 && q.iter().all(|v| v.is_finite()), "{q:?}");
    assert!((q[1] - q[2] - expected).abs() < 0.0001, "{q:?}");
    Ok(())
}
