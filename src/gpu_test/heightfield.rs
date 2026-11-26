use super::support::{run, run_field, used_bindings};
use crate::heightfield::{Field, LEAF};
use crate::{
    geometry::{Geometry, Kind},
    material::gpu::resources::{Demand, Resources},
    math::Ray,
};

#[test]
fn generated_shader_validates_for_any_material_and_field_order_count()
-> Result<(), Box<dyn std::error::Error>> {
    let presets = crate::material::presets_with_leaf(true);
    for orders in [1, 2, 6, crate::heightfield::MAX_ORDERS] {
        // Layered materials get `orders` levels too; the field material keeps its data.
        let levels: Vec<_> = presets
            .iter()
            .filter(|l| l.order == 0 || l.material.spec().field)
            .flat_map(|&l| {
                let count = if l.material.spec().field { 1 } else { orders };
                (0..count).map(move |o| crate::material::Level {
                    order: l.order + i32::try_from(o).unwrap_or(0),
                    extent: l.extent / 10.0_f64.powi(i32::try_from(o).unwrap_or(0)),
                    relief: l.relief / 10.0_f32.powi(i32::try_from(o).unwrap_or(0)),
                    ..l
                })
            })
            .collect();
        crate::material::validate(&levels, 256)?;
        let source = crate::shader::source_for(&levels, orders);
        let module = naga::front::wgsl::parse_str(&source)?;
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)?;
    }
    Ok(())
}
const PLANES: [[f64; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];
#[test]
fn leaf_gpu_material_uses_carrier_normals_and_vertex_aligned_color()
-> Result<(), Box<dyn std::error::Error>> {
    let points = [
        [0.0_f32, 0.0],
        [0.012, -0.217],
        [-0.125, 0.125],
        [1.0, 0.0],
        [-1.0, 0.0],
        [0.999_999, 0.003],
    ];
    let mut inputs = Vec::with_capacity(36);
    for p in points {
        for axis in [0_u8, 1, 2] {
            for side in [-1.0, 1.0] {
                let d = match axis {
                    0 => [0.0, 0.0, side],
                    1 => [side, 0.0, 0.0],
                    _ => [0.0, side, 0.0],
                };
                inputs.push([p[0], p[1], 1e-10, f32::from(axis), d[0], d[1], d[2], 0.07]);
            }
        }
    }
    assert!(!used_bindings("verify_leaf_material")?.contains(&9));
    let actual = run("verify_leaf_material", &inputs, inputs.len() * 16)?;
    let plane = |order: usize| Field::new(crate::heightfield::DATA, 512, &PLANES[order], [0.0; 2]);
    let fields = [plane(0)?, plane(3)?, plane(2)?, plane(1)?];
    for (input, row) in inputs.iter().zip(actual.chunks_exact(16)) {
        let xy = [f64::from(input[0]), f64::from(input[1])];
        let n = [4, 5, 6].map(|axis| -f64::from(input[axis]));
        assert!(row.iter().all(|v| v.is_finite()));
        assert!(row[7] < 0.5, "leaf is metallic");
        for a in 0..3 {
            assert!(
                (f64::from(row[a]) - n[a]).abs() < 2e-5,
                "direct hit evaluated height shading: {row:?} {n:?}"
            );
        }
        for (slot, field) in fields.into_iter().enumerate() {
            let index = 12 + slot;
            let alpha = field.sample(xy)?.height + 128.0 / 255.0;
            assert!(
                (f64::from(row[index]) - alpha).abs() < 0.005,
                "tile/field phase mismatch: {row:?}"
            );
        }
    }
    Ok(())
}
#[test]
fn leaf_gpu_lod_filters_only_appearance_and_scales_with_the_instance()
-> Result<(), Box<dyn std::error::Error>> {
    let mut inputs = Vec::with_capacity(10);
    for footprint in [1e-10, 2.7e-7, 2.7e-6, 2.7e-5, 0.000_27, 0.0027, 0.027, 0.27] {
        inputs.push([0.012, -0.217, footprint, 0.0, 0.0, 0.0, -1.0, 0.07]);
    }
    inputs.push([0.012, -0.217, 0.000_005_4, 0.0, 0.0, 0.0, -1.0, 0.14]);
    let actual = run("verify_leaf_material", &inputs, inputs.len() * 16)?;
    for row in actual.chunks_exact(16) {
        assert!(row.iter().all(|v| v.is_finite()));
        assert_eq!(
            bytemuck::cast_slice::<f32, u32>(&row[..4]),
            bytemuck::cast_slice::<f32, u32>(&actual[..4])
        );
    }
    let base = &actual[2 * 16..3 * 16];
    let doubled = &actual[8 * 16..9 * 16];
    assert_eq!(
        bytemuck::cast_slice::<f32, u32>(base),
        bytemuck::cast_slice::<f32, u32>(doubled)
    );
    assert!(
        (actual[7 * 16 + 4] - actual[4]).abs() > 0.001,
        "LOD did not filter the leaf color"
    );
    Ok(())
}
#[test]
fn leaf_gpu_resources_are_lazy_and_growth_never_reverts() -> Result<(), Box<dyn std::error::Error>>
{
    let (device, queue) = pollster::block_on(super::support::test_device())?;
    let uploaded = crate::heightfield::gpu::Heights::new(&device, LEAF)?;
    assert!(uploaded.ready());

    let mut resources = Resources::empty(&device);
    assert!(!resources.ensure(
        &device,
        &queue,
        Demand {
            leaf: false,
            materials: false
        }
    )?);
    assert!(resources.ensure(
        &device,
        &queue,
        Demand {
            leaf: true,
            materials: false
        }
    )?);
    assert!(!resources.tiles.ready() && resources.heights.ready());
    assert!(resources.ensure(
        &device,
        &queue,
        Demand {
            leaf: true,
            materials: true
        }
    )?);
    assert!(!resources.ensure(
        &device,
        &queue,
        Demand {
            leaf: true,
            materials: true
        }
    )?);
    let mut heights = crate::heightfield::gpu::Heights::empty(&device);
    assert!(!heights.ready());
    assert_eq!(heights.buffer.size(), 48);
    assert!(!heights.ensure(&device, false)?);
    assert!(heights.ensure(&device, true)?);
    assert!(heights.ready());
    assert_eq!(
        heights.buffer.size(),
        u64::try_from(crate::heightfield::DATA.len())? + 32
    );
    assert!(!heights.ensure(&device, true)?);
    assert!(!heights.ensure(&device, false)?);
    let mut tiles = crate::material::gpu::Tiles::empty(&device);
    assert!(tiles.ensure(&device, &queue, false)?);
    assert_eq!(tiles.buffer.size(), 640);
    assert!(!tiles.ensure(&device, &queue, false)?);
    assert!(tiles.ensure(&device, &queue, true)?);
    assert_eq!(tiles.buffer.size(), 960);
    assert_eq!(tiles.texture.size().depth_or_array_layers, 12);
    assert!(!tiles.ensure(&device, &queue, true)?);
    assert!(!tiles.ensure(&device, &queue, false)?);
    assert_eq!(tiles.update(&queue, [1.0; 3], false)?, 640);
    assert_eq!(tiles.update(&queue, [1.0; 3], true)?, 960);
    Ok(())
}
fn fixture(pixels: [[u8; 4]; 5]) -> Vec<u8> {
    let mut data = vec![0; 40];
    for (order, quad) in pixels.into_iter().enumerate() {
        data[order * 8..order * 8 + 4].copy_from_slice(&quad);
        let mut low = 255;
        let mut high = 0;
        for value in quad {
            low = low.min(value);
            high = high.max(value);
        }
        data[order * 8 + 4] = low;
        data[order * 8 + 5] = high;
    }
    data
}
#[test]
fn texh_gpu_matches_cpu_for_either_side_and_multiple_carriers()
-> Result<(), Box<dyn std::error::Error>> {
    for (texel, side) in [(192, 1.0_f32), (64, -1.0)] {
        let data = fixture([[texel; 4], [128; 4], [128; 4], [128; 4], [0, 255, 0, 255]]);
        let field = Field::new(&data, 2, &[255.0 / 256.0, 0.0, 0.0, 0.0], [0.0; 2])?;
        let inputs = [
            [0.0, 0.0, side * 1.125, 1.0, 0.0, 0.0, side, 2.0],
            [0.0, 0.0, side * 1.25, 1.0, 0.0, 0.0, side, 2.0],
            [0.0, 0.0, side * 1.375, 1.0, 0.0, 0.0, side, 2.0],
            [0.0, 0.0, -side * 0.125, 5.0, 0.0, 0.0, -side, 2.0],
            [0.0, 0.0, side * 1.125, 1.0, 0.0, 0.0, side, 0.0],
            [0.0, 0.0, side * 0.125, 5.0, 0.0, 0.0, side, 2.0],
            [-0.9, 0.0, side * 0.125, 5.0, 0.0, 0.0, side, 2.0],
            [0.0, 0.0, side, 5.0, 0.0, 0.0, -side, 2.0],
        ];
        let actual = run_field("verify_texh", &inputs, inputs.len() * 16, field)?;
        for ((row, input), hit) in actual.chunks_exact(16).zip(inputs).zip([
            texel > 128,
            texel > 128,
            false,
            false,
            false,
            true,
            false,
            true,
        ]) {
            let kind = if input[3] > 2.0 {
                Kind::FinitePlane
            } else {
                Kind::Sphere
            };
            let ray = Ray {
                o: [0, 1, 2].map(|i| f64::from(input[i])),
                d: [4, 5, 6].map(|i| f64::from(input[i])),
            };
            let base = Geometry::new(kind, 0.0)?.query(ray)?;
            let query = if input[7] > 1.0 {
                field.texh(ray, base, (kind == Kind::FinitePlane).then_some(0.0))?
            } else {
                base
            };
            assert_eq!(query.sample.hit, hit, "CPU texh {input:?}");
            assert_eq!(row[0] > 0.5, hit, "GPU texh {input:?}: {row:?}");
            assert!((f64::from(row[1]) - query.sample.t).abs() < 1e-5);
        }
    }
    Ok(())
}

#[test]
fn texh_blends_four_texels_on_a_gap() -> Result<(), Box<dyn std::error::Error>> {
    let data = fixture([[192, 128, 224, 160], [128; 4], [128; 4], [128; 4], [255; 4]]);
    let field = Field::new(&data, 2, &[255.0 / 256.0, 0.0, 0.0, 0.0], [0.0; 2])?;
    // UV (.25,.25) blends four corners to 176, for a height of 0.1875.
    let inputs = [0.17_f32, 0.20].map(|z| [-0.5, -0.5, z, 5.0, 0.0, 0.0, 1.0, 2.0]);
    let actual = run_field("verify_texh", &inputs, inputs.len() * 16, field)?;
    let carrier = Geometry::new(Kind::FinitePlane, 0.0)?;
    for ((row, input), hit) in actual.chunks_exact(16).zip(inputs).zip([true, false]) {
        let ray = Ray {
            o: [-0.5, -0.5, f64::from(input[2])],
            d: [0.0, 0.0, 1.0],
        };
        assert_eq!(
            field.texh(ray, carrier.query(ray)?, Some(0.0))?.sample.hit,
            hit
        );
        assert_eq!(row[0] > 0.5, hit);
    }
    Ok(())
}

#[test]
fn leaf_gpu_bilinear_matches_cpu_bands_and_mask() -> Result<(), Box<dyn std::error::Error>> {
    let points: [[f32; 2]; 16] = [
        [0.0, 0.0],
        [0.012, -0.217],
        [-0.96, -0.96],
        [-1.0, -1.0],
        [1.0, 1.0],
        [0.25, 0.75],
        [-0.125, 0.125],
        [0.003, -0.017],
        [-0.34567, 0.12345],
        [0.99999, -0.9999],
        [-0.75, -0.25],
        [0.1, -0.1],
        [0.123, -0.321],
        [0.5, -0.375],
        [-0.007, 0.019],
        [0.42, 0.18],
    ];
    let inputs = points.map(|[x, y]| {
        [
            x.mul_add(0.5, 0.5),
            y.mul_add(0.5, 0.5),
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
        ]
    });
    let actual = run_field("verify_heightfield", &inputs, inputs.len() * 16, LEAF)?;
    for (row, input) in actual.chunks_exact(16).zip(inputs) {
        let value = LEAF.sample([0, 1].map(|axis| f64::from(input[axis]).mul_add(2.0, -1.0)))?;
        assert!((f64::from(row[0]) - value.height).abs() < 1e-5);
        assert!((f64::from(row[1]) - value.coverage).abs() < 5e-5);
    }
    Ok(())
}
