use super::support::run_field;
use crate::heightfield::{Field, LEAF};
use crate::{
    geometry::{Geometry, Kind},
    material::gpu::resources::{Demand, Resources},
    math::Ray,
};

#[test]
fn leaf_gpu_material_uses_the_same_scalar_and_vertex_aligned_tiles()
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
    let actual = run_field("verify_leaf_material", &inputs, inputs.len() * 16, LEAF)?;
    let coarse = Field::new(
        crate::heightfield::DATA,
        512,
        [1.0, 0.0, 0.0, 0.0],
        [0.0; 2],
    )?;
    let finest = Field::new(
        crate::heightfield::DATA,
        512,
        [0.0, 0.0, 0.0, 1.0],
        [0.0; 2],
    )?;
    for (input, row) in inputs.iter().zip(actual.chunks_exact(16)) {
        let xy = [f64::from(input[0]), f64::from(input[1])];
        let value = LEAF.sample(xy)?;
        let n = value.normal();
        let mut n = if input[3] < 0.5 {
            n
        } else if input[3] < 1.5 {
            [n[2], n[0], n[1]]
        } else {
            [n[1], n[2], n[0]]
        };
        if (0..3).map(|a| n[a] * f64::from(input[4 + a])).sum::<f64>() > 0.0 {
            n = n.map(|v| -v);
        }
        assert!(row.iter().all(|v| v.is_finite()));
        assert!(row[7] < 0.5, "leaf is metallic");
        for a in 0..3 {
            assert!(
                (f64::from(row[a]) - n[a]).abs() < 2e-5,
                "wrong summed normal: {row:?} {n:?}"
            );
        }
        assert!(f64::from(row[8]) <= value.height && f64::from(row[9]) >= value.height);
        for (index, field) in [(12, coarse), (13, finest)] {
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
    let actual = run_field("verify_leaf_material", &inputs, inputs.len() * 16, LEAF)?;
    for row in actual.chunks_exact(16) {
        assert!(row.iter().all(|v| v.is_finite()));
        assert_eq!(
            bytemuck::cast_slice::<f32, u32>(&row[8..14]),
            bytemuck::cast_slice::<f32, u32>(&actual[8..14])
        );
    }
    let base = &actual[2 * 16..3 * 16];
    let doubled = &actual[8 * 16..9 * 16];
    assert_eq!(
        bytemuck::cast_slice::<f32, u32>(base),
        bytemuck::cast_slice::<f32, u32>(doubled)
    );
    assert!(
        actual[7 * 16 + 3] > actual[3],
        "unresolved fine slopes add no roughness"
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
    assert_eq!(heights.buffer.size(), 5 * 512 * 512 + 32);
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
        let field = Field::new(&data, 2, [255.0 / 256.0, 0.0, 0.0, 0.0], [0.0; 2])?;
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
                field.texh(ray, base, kind == Kind::FinitePlane)?
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
fn leaf_gpu_reads_one_summed_scalar_and_normals() -> Result<(), Box<dyn std::error::Error>> {
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
    let inputs = points.map(|[x, y]| [x, y, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    let actual = run_field("verify_heightfield", &inputs, inputs.len() * 32, LEAF)?;
    for (index, row) in actual.chunks_exact(32).enumerate() {
        let value = LEAF.sample(points[index].map(f64::from))?;
        assert!(
            row[11] > 0.5 && f64::from(row[1]) <= value.height && f64::from(row[2]) >= value.height
        );
        assert!(f64::from(row[6]) <= value.coverage && f64::from(row[7]) >= value.coverage);
        for axis in 0..2 {
            assert!((f64::from(row[4 + axis]) - value.gradient[axis]).abs() < 2e-5);
        }
    }
    Ok(())
}
