use super::support::{run, run_field, used_bindings};
use crate::heightfield::Field;

type TestResult = Result<(), Box<dyn std::error::Error>>;

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
            .then_some(handle)
        })
        .collect();
    assert_eq!(functions.len(), 2);
    let mut seen = std::collections::BTreeSet::new();
    while let Some(handle) = functions.pop() {
        if !seen.insert(handle) {
            continue;
        }
        let function = &module.functions[handle];
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
                    naga::Statement::Call { function, .. } => functions.push(*function),
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
        "verify_material_hit",
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
