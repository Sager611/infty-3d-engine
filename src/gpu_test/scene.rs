use super::support::{run, run_scene};
use crate::{
    math::Ray,
    scene::{
        model::{Kind, Object},
        packed::pack,
        query::query_object,
    },
};

#[test]
fn posed_materials_keep_local_depth_uv_color_and_world_normals()
-> Result<(), Box<dyn std::error::Error>> {
    let half = std::f64::consts::FRAC_1_SQRT_2;
    let first = Object::new(7, Kind::FinitePlane, [0.5, 1.0, 0.25], 1.0, 0.0)?
        .with_material(crate::material::Material::Brick);
    let second = Object::new(8, Kind::FinitePlane, [0.5, 1.0, 0.25], 1.0, 0.0)?
        .with_material(crate::material::Material::Brick)
        .with_rotation([0.0, half, 0.0, half])?;
    let packet = pack(&[first, second], &[])?;
    let slot = |id| -> Result<f32, Box<dyn std::error::Error>> {
        Ok(f32::from(u16::try_from(
            packet
                .records
                .iter()
                .position(|r| r.identity[0] == id)
                .ok_or("missing instance")?,
        )?))
    };
    let inputs = [
        [0.625, 1.0625, 3.25, slot(7)?, 0.0, 0.0, -1.0, 0.0],
        [3.5, 1.0625, 0.125, slot(8)?, -1.0, 0.0, 0.0, 0.0],
        [2.875, 1.0625, 3.25, slot(7)?, -0.6, 0.0, -0.8, 0.0],
        [3.475, 1.0625, 4.05, slot(7)?, -0.6, 0.0, -0.8, 0.0],
    ];
    let q = run_scene("verify_instance_pose", &inputs, &packet.records)?;
    for row in q.chunks_exact(16) {
        assert!(
            row.iter().all(|v| v.is_finite()) && row[0] > 0.5 && row[2] > 0.5,
            "{row:?}"
        );
    }
    for index in [1, 3, 8, 9, 12, 13, 14] {
        assert!(
            (q[index] - q[16 + index]).abs() < 0.002,
            "pose frame mismatch {index}: {q:?}"
        );
    }
    assert!(
        (q[4] + q[22]).abs() < 0.002
            && (q[5] - q[21]).abs() < 0.002
            && (q[6] - q[20]).abs() < 0.002,
        "world normal: {q:?}"
    );
    for index in [8, 9, 12, 13, 14] {
        assert!(
            (q[32 + index] - q[48 + index]).abs() < 0.002,
            "camera moved material frame {index}: {q:?}"
        );
    }
    Ok(())
}

#[test]
fn leaf_cutout_is_not_a_solid_selection_carrier() -> Result<(), Box<dyn std::error::Error>> {
    let object = Object::new(7, Kind::FinitePlane, [0.0; 3], 1.0, 0.0)?
        .with_material(crate::material::Material::Leaf);
    let packet = pack(&[object], &[7])?;
    let q = run_scene(
        "verify_instance_pose",
        &[[0.99, 0.99, 1.0, 0.0, 0.0, 0.0, -1.0, 0.0]],
        &packet.records,
    )?;
    assert!(q[0] < 0.5 && q[2] < 0.5, "cutout became covered: {q:?}");
    assert!(
        q[7] < 0.5,
        "transparent carrier filled by selection outline: {q:?}"
    );
    Ok(())
}

#[test]
fn transformed_scene_packets_match_cpu() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures = [
        (Kind::Sphere, [0.0, 1.0, -3.0, 1.0, 0.0, 0.0, 1.0, 2.0]),
        (Kind::Sphere, [3.0, 1.0, -3.0, 1.0, 0.0, 0.0, 1.0, 0.75]),
        (Kind::Plane, [0.0, 3.0, 0.0, 3.0, 0.0, -1.0, 0.0, 2.0]),
        (Kind::Plane, [0.0, 3.0, 0.0, 3.0, 1.0, 0.0, 0.0, 2.0]),
        (Kind::Goursat, [0.75, 1.75, -3.0, 2.0, 0.0, 0.0, 1.0, 0.75]),
    ];
    let inputs: Vec<[f32; 8]> = fixtures.iter().map(|(_, ray)| *ray).collect();
    let actual = run("verify_scene", &inputs, inputs.len() * 32)?;
    for (index, (kind, input)) in fixtures.into_iter().enumerate() {
        let object = Object::new(
            7,
            kind,
            [0.0, 1.0, 0.0],
            f64::from(input[7]),
            if kind == Kind::Goursat { 1.5 } else { 0.0 },
        )?;
        let ray = Ray {
            o: std::array::from_fn(|i| f64::from(input[i])),
            d: std::array::from_fn(|i| f64::from(input[i + 4])),
        };
        let expected = query_object(&object, ray)?.packed(ray);
        let output = &actual[index * 32..(index + 1) * 32];
        for (field, expected) in expected.into_iter().enumerate() {
            let value = f64::from(output[field]);
            let tolerance = 2.0e-4 * expected.abs().max(1.0);
            assert!(
                value.is_finite() && (value - expected).abs() <= tolerance,
                "{kind:?} field {field}: {value} != {expected}"
            );
        }
        assert!(output[24..].iter().all(|v| v.to_bits() == 0));
    }
    Ok(())
}

#[test]
fn relief_runs_once_per_pixel_after_a_lookup_free_traversal() {
    let render = include_str!("../shaders/render.wgsl");
    let shadow = include_str!("../shaders/materials/shadow.wgsl");
    let after = render
        .find("// One relief pass per pixel")
        .unwrap_or(usize::MAX);
    assert_eq!(render.matches("material_texh_depth(").count(), 1);
    assert!(
        render
            .find("material_texh_depth(")
            .is_some_and(|at| at > after)
    );
    assert!(render.contains("scene_canonical(local_o,local_d,object,false)"));
    assert!(shadow.contains("scene_canonical(local,local_d,other,false)"));
    for source in [render, shadow] {
        assert!(
            !source.contains("leaf_covered("),
            "traversal coverage lookup"
        );
    }
}
