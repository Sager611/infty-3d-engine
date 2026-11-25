use super::*;
use crate::scene::{
    model::{pack_objects, parse_objects},
    packed::pack,
    prepared::Prepared,
};

#[test]
fn leaf_palette_appends_four_physical_orders_without_changing_the_baseline()
-> Result<(), &'static str> {
    let base = presets();
    let all = presets_with_leaf(true);
    assert_eq!(all.len(), 12);
    validate(&all, 12)?;
    for (before, after) in base.iter().zip(&all) {
        assert_eq!(before.material, after.material);
        assert_eq!(before.extent.to_bits(), after.extent.to_bits());
        assert!(std::ptr::eq(before.source, after.source));
    }
    for (index, level) in all[8..].iter().enumerate() {
        assert_eq!(level.material, Material::Leaf);
        let order = i32::try_from(index).map_err(|_| "order")?;
        assert!((level.extent - 0.14 / 10_f64.powi(order)).abs() < 1e-15);
        assert!(level.atlas.is_none());
        assert_eq!(texture(*level)?.len(), 512 * 512 * 4);
    }
    Ok(())
}
#[test]
fn four_physical_orders_per_finish_not_a_shader_level_limit() -> Result<(), &'static str> {
    assert_eq!(Material::from_tag(0), Some(Material::Steel));
    assert_eq!(Material::from_tag(1), Some(Material::Brick));
    assert_eq!(Material::from_tag(2), Some(Material::Leaf));
    assert_eq!(Material::from_tag(3), None);
    assert_eq!(Material::Brick.tag(), 1);
    let levels = presets();
    assert_eq!(levels.len(), 8);
    validate(&levels, 256)?;
    for material in [Material::Steel, Material::Brick] {
        let rows: Vec<_> = levels.iter().filter(|v| v.material == material).collect();
        assert_eq!(rows.len(), 4);
        for (i, row) in rows.iter().enumerate() {
            assert!(row.relief.is_finite() && row.color.is_finite() && row.roughness.is_finite());
            assert!(
                (row.extent - 0.42 / 10.0_f64.powi(i32::try_from(i).map_err(|_| "order")?)).abs()
                    < 1e-15
            );
        }
    }
    let mut extended = levels.clone();
    let mut row = levels[0];
    row.order = -1;
    row.extent = 4.2;
    row.relief = 0.0;
    extended.push(row);
    row.order = 4;
    row.extent = 0.000_042;
    extended.push(row);
    validate(&extended, 256)?;
    assert!(validate(&extended, 9).is_err());
    Ok(())
}
#[test]
fn brick_pore_relief_is_eight_times_stronger_without_deepening_mortar() {
    let relief: Vec<_> = presets()
        .into_iter()
        .filter(|level| level.material == Material::Brick)
        .map(|level| level.relief.to_bits())
        .collect();
    assert_eq!(
        relief,
        [0.006_f32, 0.0064, 0.00064, 0.000_048].map(f32::to_bits)
    );
}
#[test]
fn baked_assets_reject_corruption_and_wrong_pixel_counts() {
    for bytes in [&[][..], &[0][..], &[120, 156, 3, 0, 0, 0, 0, 1][..]] {
        assert!(super::texture::decode(bytes).is_err());
    }
    let oversized = miniz_oxide::deflate::compress_to_vec_zlib(&vec![0; 512 * 512 * 4 + 4], 1);
    assert!(super::texture::decode(&oversized).is_err());
}

#[test]
fn texture_levels_are_real_rgba_tiles_with_distinct_detail() -> Result<(), &'static str> {
    let levels = presets();
    assert_eq!(levels.len(), 8);
    let mut previous = Vec::new();
    for row in levels {
        let pixels = texture(row)?;
        assert_eq!(pixels.len(), 512 * 512 * 4);
        assert!(pixels.chunks_exact(4).any(|p| p != &pixels[..4]));
        assert_ne!(pixels, previous);
        assert!(pixels.chunks_exact(4).any(|p| p[3] > 32 && p[3] < 224));
        previous = pixels;
    }
    Ok(())
}
fn pore_components(pixels: &[u8]) -> (usize, usize) {
    let mut seen = vec![false; 512 * 512];
    let mut count = 0;
    let mut largest = 0;
    for start in 0..seen.len() {
        if seen[start] || pixels[4 * start + 3] >= 80 {
            continue;
        }
        let mut stack = vec![start];
        seen[start] = true;
        let mut size = 0;
        while let Some(index) = stack.pop() {
            size += 1;
            let x = index % 512;
            let y = index / 512;
            for next in [
                if x > 0 { Some(index - 1) } else { None },
                if x < 511 { Some(index + 1) } else { None },
                if y > 0 { Some(index - 512) } else { None },
                if y < 511 { Some(index + 512) } else { None },
            ]
            .into_iter()
            .flatten()
            {
                if !seen[next] && pixels[4 * next + 3] < 80 {
                    seen[next] = true;
                    stack.push(next);
                }
            }
        }
        count += size;
        largest = largest.max(size);
    }
    (count, largest)
}

#[test]
fn brick_microporosity_is_irregular_and_connected_not_round_dots() -> Result<(), &'static str> {
    let level = presets()
        .into_iter()
        .find(|l| l.material == Material::Brick && l.order == 3)
        .ok_or("missing brick micro level")?;
    let (area, largest) = pore_components(&texture(level)?);
    assert!(
        (512 * 512 * 8 / 100..512 * 512 * 35 / 100).contains(&area),
        "microscopy-scale pore area missing: {area}"
    );
    assert!(largest > 512, "only isolated dot pores remain: {largest}");
    Ok(())
}

#[test]
fn linear_box_mips_average_color_and_height() {
    let pixels = [0, 4, 8, 12, 4, 8, 12, 16, 8, 12, 16, 20, 12, 16, 20, 24];
    assert_eq!(mip(&pixels, 2), Ok(vec![6, 10, 14, 18]));
    assert!(mip(&pixels, 3).is_err());
    assert!(mip(&pixels[..12], 2).is_err());
}
#[test]
fn brick_cavity_floors_retain_microscopic_relief_instead_of_clipping_flat()
-> Result<(), &'static str> {
    for level in presets()
        .into_iter()
        .filter(|l| l.material == Material::Brick && l.order >= 2)
    {
        let pixels = texture(level)?;
        let zero = pixels.chunks_exact(4).filter(|p| p[3] == 0).count();
        assert!(
            zero * 1000 < pixels.len() / 4,
            "clipped cavity floors discard SEM relief: {zero} texels"
        );
    }
    Ok(())
}
#[test]
fn brick_pores_use_lighting_not_black_paint_for_cavity_contrast() -> Result<(), &'static str> {
    for level in presets()
        .into_iter()
        .filter(|l| l.material == Material::Brick && l.order >= 2)
    {
        let pixels = texture(level)?;
        let stats = pixels.chunks_exact(4).fold([0_u32; 4], |mut s, p| {
            if p[3] < 80 {
                s[0] += u32::from(p[0]);
                s[1] += 1;
            }
            if p[3] > 140 {
                s[2] += u32::from(p[0]);
                s[3] += 1;
            }
            s
        });
        assert!(stats[1] > 1000 && stats[3] > 1000);
        let ratio = (f64::from(stats[0]) / f64::from(stats[1]))
            / (f64::from(stats[2]) / f64::from(stats[3]));
        assert!(
            (0.85..1.15).contains(&ratio),
            "pore albedo encodes an ink mask: {ratio}"
        );
    }
    Ok(())
}
#[test]
fn steel_fine_tiles_share_the_shader_brush_axis() -> Result<(), &'static str> {
    for level in presets()
        .into_iter()
        .filter(|l| l.material == Material::Steel && l.order > 0)
    {
        let pixels = texture(level)?;
        let mut across = 0_u64;
        let mut along = 0_u64;
        for y in 0..511 {
            for x in 0..511 {
                let i = (y * 512 + x) * 4 + 3;
                across += u64::from(pixels[i].abs_diff(pixels[i + 4])).pow(2);
                along += u64::from(pixels[i].abs_diff(pixels[i + 512 * 4])).pow(2);
            }
        }
        assert!(
            across > 9 * along,
            "steel order {} runs across its declared brush axis",
            level.order
        );
    }
    Ok(())
}
#[test]
fn camera_phase_keeps_mirror_identity_without_losing_fractional_motion() {
    let l = presets()[0];
    let a = phase(l, [0.0, l.extent, -l.extent]);
    assert_eq!(a[3].to_bits(), 16_711_936.0_f32.to_bits());
    let b = phase(l, [l.extent * 256.0, l.extent * 257.0, l.extent * 255.0]);
    for i in 0..4 {
        assert!((a[i] - b[i]).abs() < 0.0001);
    }
    let near = phase(l, [l.extent - 1e-10, 0.0, 0.0]);
    assert_eq!(near[0].to_bits(), 0.0_f32.to_bits());
    assert_eq!(near[3].to_bits(), 1.0_f32.to_bits());
}
#[test]
fn camera_phases_keep_sub_f32_motion_and_repeat_at_both_signs() {
    let level = Level {
        atlas: None,
        source: super::texture::assets(Material::Steel)[0],
        material: Material::Steel,
        order: 3,
        extent: 0.000_42,
        relief: 0.0,
        color: 1.0,
        roughness: 0.0,
    };
    let a = phase(level, [1.0, -1.0, 0.0]);
    let b = phase(level, [1.0 + 1e-10, -1.0, 0.0]);
    assert_ne!(a[0].to_bits(), b[0].to_bits());
    assert!(a[..3].iter().all(|v| *v >= 0.0 && *v < 1.0));
    let c = phase(level, [1.0 + level.extent, -1.0 - level.extent, 0.0]);
    assert!((a[0] - c[0]).abs() < 1e-6 && (a[1] - c[1]).abs() < 1e-6);
}
#[test]
fn malformed_or_duplicate_layers_fail_before_gpu_work() {
    let row = Level {
        atlas: None,
        source: super::texture::assets(Material::Brick)[0],
        material: Material::Brick,
        order: 0,
        extent: 0.42,
        relief: 0.01,
        color: 1.0,
        roughness: 0.2,
    };
    assert!(validate(&[], 256).is_err());
    assert!(validate(&[row, row], 256).is_err());
    for cells in [[0, 1], [1, 0], [513, 1]] {
        assert!(
            validate(
                &[Level {
                    atlas: Some(Atlas {
                        cells,
                        border: [0.0; 4]
                    }),
                    ..row
                }],
                256
            )
            .is_err()
        );
    }
    assert!(
        validate(
            &[Level {
                atlas: Some(Atlas {
                    cells: [2, 6],
                    border: [f32::NAN; 4]
                }),
                ..row
            }],
            256
        )
        .is_err()
    );
    assert!(validate(&[Level { source: &[], ..row }], 256).is_err());
    for extent in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e300, 1e-300] {
        assert!(validate(&[Level { extent, ..row }], 256).is_err());
    }
    assert!(validate(&[Level { order: 1, ..row }], 256).is_err());
}
#[test]
fn material_packets_round_trip_without_splitting_shared_geometry()
-> Result<(), Box<dyn std::error::Error>> {
    let packet = [
        1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 2.0, 1.0, 3.0, 0.0, 0.0, 1.0, 0.0, 1.0,
    ];
    let objects = parse_objects(&packet)?;
    assert_eq!(pack_objects(&objects), packet);
    let scene = Prepared::new(&objects)?;
    assert_eq!(scene.geometries().len(), 1);
    assert_eq!(scene.instances()[1].material(), 1);
    let records = pack(&objects, &[2])?.records;
    let brick = records
        .iter()
        .find(|r| r.identity[0] == 2)
        .ok_or("missing brick")?;
    assert_eq!(brick.params[3].to_bits(), 1.0_f32.to_bits());
    assert_eq!(brick.identity[2], 1);
    Ok(())
}
