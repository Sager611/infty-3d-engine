use super::support::run_scene_field;
use crate::{
    heightfield::Field,
    material::Material,
    scene::{
        model::{Kind, Object},
        packed::pack,
    },
};
#[test]
fn brick_carrier_shadow_skips_only_its_provably_outgoing_ray()
-> Result<(), Box<dyn std::error::Error>> {
    let source =
        Object::new(7, Kind::FinitePlane, [0.0; 3], 1.0, 0.0)?.with_material(Material::Brick);
    let blocker = Object::new(8, Kind::FinitePlane, [0.5, 0.0, 0.125], 0.25, 1.0)?;
    let half = std::f64::consts::FRAC_1_SQRT_2;
    let rotation = [0.0, half, 0.0, half];
    let posed =
        Object::new(8, Kind::FinitePlane, [0.5, 0.0, 0.0], 0.25, 0.0)?.with_rotation(rotation)?;
    for (scene, light, blocked) in [
        (vec![source], [1.0, 1e-30, 1e-30], false),
        (vec![source], [0.0, 0.0, -1.0], true),
        (vec![source, blocker], [1.0, 0.0, 0.25], true),
        (
            vec![source.with_rotation(rotation)?, posed],
            [1.0, 0.0, 0.0],
            true,
        ),
    ] {
        let packet = pack(&scene, &[])?;
        let slot = packet
            .records
            .iter()
            .position(|r| r.identity[0] == 7)
            .ok_or("source")?;
        let input = [[
            0.0,
            0.0,
            1.0,
            f32::from(u16::try_from(slot)?),
            light[0],
            light[1],
            light[2],
            f32::from(u16::try_from(packet.count)?),
        ]];
        let row = run_scene_field(
            "verify_finite_shadow",
            &input,
            &packet.records,
            crate::heightfield::LEAF,
        )?;
        assert!(row[0] > 0.5);
        assert!(row[2] < 0.5, "outgoing ray reported unresolved: {row:?}");
        assert_eq!(row[3] > 0.5, blocked, "shadow changed: {row:?}");
    }
    Ok(())
}

fn data(height: [u8; 4], mask: [u8; 4]) -> [u8; 40] {
    let mut bytes = [0; 40];
    for (field, quad) in [height, [128; 4], [128; 4], [128; 4], mask]
        .into_iter()
        .enumerate()
    {
        let offset = 8 * field;
        bytes[offset..offset + 4].copy_from_slice(&quad);
        bytes[offset + 4] = quad.into_iter().fold(255, u8::min);
        bytes[offset + 5] = quad.into_iter().fold(0, u8::max);
    }
    bytes
}
#[test]
fn leaf_shadow_cutouts_and_close_occluders_do_not_inherit_a_world_scale_bias()
-> Result<(), Box<dyn std::error::Error>> {
    for (mask, x, blocked) in [([255; 4], 0.0, true), ([0, 255, 0, 255], -0.9, false)] {
        let bytes = data([128; 4], mask);
        let field = Field::new(&bytes, 2, &[0.0; 4], [0.0; 2])?;
        let source = Object::new(7, Kind::FinitePlane, [0.0, 1.0, 0.0], 0.07, 0.0)?;
        let target = Object::new(8, Kind::FinitePlane, [0.0, 1.0, 0.000_02], 0.07, 0.0)?
            .with_material(Material::Leaf);
        let packet = pack(&[source, target], &[])?;
        let slot = packet
            .records
            .iter()
            .position(|r| r.identity[0] == 7)
            .ok_or("source")?;
        let input = [[
            x,
            0.0,
            0.000_01,
            f32::from(u16::try_from(slot)?),
            x * 0.07,
            1.0,
            1.0,
            f32::from(u16::try_from(packet.count)?),
        ]];
        let result = run_scene_field("verify_finite_shadow", &input, &packet.records, field)?;
        assert!(result[0] > 0.5, "source carrier disappeared");
        assert_eq!(
            result[1] < 0.5,
            blocked,
            "thin leaf shadow was skipped: {result:?}"
        );
        assert!(result[2] < 0.5);
    }
    Ok(())
}
