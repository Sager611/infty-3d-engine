use super::support::run_field;
use crate::heightfield::{Field, LEAF, Node};
use crate::{
    material::{
        Material,
        gpu::resources::{Demand, Resources},
    },
    math::Ray,
    scene::{
        model::{Kind, Object},
        query::query_object,
    },
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
            geometry: false,
            leaf: false,
            materials: false
        }
    )?);
    assert!(resources.ensure(
        &device,
        &queue,
        Demand {
            geometry: true,
            leaf: true,
            materials: false
        }
    )?);
    assert!(!resources.tiles.ready() && resources.heights.ready());
    assert!(resources.ensure(
        &device,
        &queue,
        Demand {
            geometry: true,
            leaf: true,
            materials: true
        }
    )?);
    assert!(!resources.ensure(
        &device,
        &queue,
        Demand {
            geometry: true,
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
        u64::try_from(crate::heightfield::DATA.len() + 32)?
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
#[test]
fn leaf_gpu_front_back_cutout_oblique_and_work_exhaustion_are_explicit()
-> Result<(), Box<dyn std::error::Error>> {
    let inputs = [
        [0.012, -0.217, 1.0, 0.0, 0.0, 0.0, -1.0, 64.0],
        [0.012, -0.217, -1.0, 0.0, 0.0, 0.0, 1.0, 64.0],
        [-0.96, -0.96, 1.0, 0.0, 0.0, 0.0, -1.0, 64.0],
        [0.0, 0.0, 0.1, 0.0, 0.3, 0.4, -0.75_f32.sqrt(), 64.0],
        [0.0, 0.0, 0.1, 0.0, 0.3, 0.4, -0.75_f32.sqrt(), 0.0],
        [0.0, 0.0, 0.1, 0.0, 0.3, 0.4, -0.75_f32.sqrt(), 1.0],
        [0.0, 0.0, 0.1, 0.0, 0.3, 0.4, -0.75_f32.sqrt(), 4096.0],
    ];
    let actual = run_field("verify_height_ray", &inputs, inputs.len() * 32, LEAF)?;
    let object =
        Object::new(7, Kind::FinitePlane, [0.0; 3], 1.0, 0.0)?.with_material(Material::Leaf);
    for (index, row) in actual.chunks_exact(32).enumerate() {
        assert!(row[11] <= 64.0 && row[11] <= inputs[index][7]);
        if index == 4 || index == 5 {
            assert!(row[0] < 0.5 && row[14] < 0.5 && row[15] > 0.5);
            continue;
        }
        assert!(
            row[14] > 0.5 && row[15] < 0.5,
            "GPU leaf unresolved: ray{index} {row:?}"
        );
        let input = inputs[index];
        let ray = Ray {
            o: [0, 1, 2].map(|i| f64::from(input[i])),
            d: [4, 5, 6].map(|i| f64::from(input[i])),
        };
        let expected = query_object(&object, ray, true)?;
        assert_eq!(row[0] > 0.5, expected.sample.hit);
        if expected.sample.hit {
            let length = ray.d[0].hypot(ray.d[1]).hypot(ray.d[2]);
            let time = expected.sample.t / length;
            assert!(f64::from(row[2]) <= time + 1e-12 && f64::from(row[3]) >= time - 1e-12);
            let xy = [f64::from(row[4]), f64::from(row[5])];
            assert!(
                (LEAF.sample(xy)?.height - f64::from(row[6])).abs() <= f64::from(row[7]) + 1e-12
            );
        }
    }
    assert_eq!(
        bytemuck::cast_slice::<f32, u32>(&actual[3 * 32..4 * 32]),
        bytemuck::cast_slice::<f32, u32>(&actual[6 * 32..7 * 32])
    );
    Ok(())
}
#[test]
fn leaf_gpu_certificates_agree_with_f64_without_hiding_unresolved_work()
-> Result<(), Box<dyn std::error::Error>> {
    let mut inputs = Vec::with_capacity(1024);
    for y in 0_u16..32 {
        for x in 0_u16..32 {
            let case = (x + y) % 8;
            let mut direction = match case % 4 {
                0 => [0.0, 0.0, -1.0],
                1 => [0.3, 0.4, -0.75_f32.sqrt()],
                2 => [0.98, 0.0, -0.98_f32.mul_add(-0.98, 1.0).sqrt()],
                _ => [1.0, 0.0, 0.0],
            };
            if case >= 4 {
                direction[2] *= -1.0;
            }
            let target = [
                (f32::from(x) / 31.0).mul_add(2.2, -1.1),
                (f32::from(y) / 31.0).mul_add(2.2, -1.1),
            ];
            let time = if direction[2].abs() > 0.0 {
                0.1 / direction[2].abs()
            } else {
                0.2
            };
            inputs.push([
                target[0] - time * direction[0],
                target[1] - time * direction[1],
                -time * direction[2],
                0.0,
                direction[0],
                direction[1],
                direction[2],
                64.0,
            ]);
        }
    }
    let actual = run_field("verify_height_ray", &inputs, inputs.len() * 32, LEAF)?;
    let object =
        Object::new(7, Kind::FinitePlane, [0.0; 3], 1.0, 0.0)?.with_material(Material::Leaf);
    let mut resolved = 0;
    let mut cpu_incomplete = 0;
    let mut hits = 0;
    for (index, row) in actual.chunks_exact(32).enumerate() {
        assert!(row.iter().take(32).all(|value| value.is_finite()));
        assert!(row[11] <= 64.0);
        if row[14] < 0.5 {
            assert!(row[15] > 0.5 && (row[0] < 0.5 || row[16] <= row[3]));
            continue;
        }
        resolved += 1;
        assert!(row[15] < 0.5);
        let input = inputs[index];
        let ray = Ray {
            o: [0, 1, 2].map(|i| f64::from(input[i])),
            d: [4, 5, 6].map(|i| f64::from(input[i])),
        };
        let expected = query_object(&object, ray, true)?;
        if expected.uncertain {
            cpu_incomplete += 1;
            continue;
        }
        assert_eq!(
            row[0] > 0.5,
            expected.sample.hit,
            "resolved leaf ray{index}: {input:?} {row:?}"
        );
        if row[0] > 0.5 {
            hits += 1;
            let norm = ray.d[0].hypot(ray.d[1]).hypot(ray.d[2]);
            let time = expected.sample.t / norm;
            assert!(
                f64::from(row[2]) <= time + expected.error
                    && f64::from(row[3]) >= time - expected.error,
                "resolved leaf time{index}: {time} {row:?}"
            );
        }
    }
    eprintln!(
        "Leaf GPU certificates: {resolved}/1024 resolved; {hits} hits; {cpu_incomplete} f64 comparisons unresolved"
    );
    assert!(
        resolved >= 512,
        "leaf returns uncertainty instead of doing bounded work"
    );
    Ok(())
}
#[test]
fn leaf_gpu_coplanar_corner_never_becomes_a_certain_miss() -> Result<(), Box<dyn std::error::Error>>
{
    let data = fixture([[128; 4], [128; 4], [128; 4], [128; 4], [255; 4]]);
    let field = Field::new(&data, 2, [0.0; 4], [0.0; 2])?;
    let diagonal = std::f32::consts::FRAC_1_SQRT_2;
    let actual = run_field(
        "verify_height_ray",
        &[[-2.0, 0.0, 0.0, 0.0, diagonal, diagonal, 0.0, 64.0]],
        32,
        field,
    )?;
    assert!(
        actual[0] > 0.5 || actual[14] < 0.5,
        "GPU discarded a true corner contact"
    );
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
fn leaf_gpu_analytic_roots_preserve_order_cutouts_and_coplanar_entry()
-> Result<(), Box<dyn std::error::Error>> {
    let diagonal = std::f32::consts::FRAC_1_SQRT_2;
    for (mask, second) in [([255; 4], false), ([0, 255, 0, 255], true)] {
        let data = fixture([[128, 128, 128, 192], [128; 4], [128; 4], [128; 4], mask]);
        let field = Field::new(&data, 2, [1.0, 0.0, 0.0, 0.0], [0.0; 2])?;
        let input = [
            -1.0,
            0.0,
            64.0 / 255.0 * 3.0 / 16.0,
            0.0,
            diagonal,
            -diagonal,
            0.0,
            64.0,
        ];
        let actual = run_field("verify_height_ray", &[input], 32, field)?;
        let root = (1.0_f64 - 4.0 * f64::from(input[2]) * 255.0 / 64.0).sqrt();
        let expected = (1.0 + if second { root } else { -root }) * 0.5 / f64::from(diagonal);
        assert!(actual[0] > 0.5 && actual[14] > 0.5 && actual[15] < 0.5);
        assert!(f64::from(actual[2]) <= expected && f64::from(actual[3]) >= expected);
    }
    let data = fixture([[128; 4], [128; 4], [128; 4], [128; 4], [0, 255, 0, 255]]);
    let field = Field::new(&data, 2, [0.0; 4], [0.0; 2])?;
    let actual = run_field(
        "verify_height_ray",
        &[[-1.0, -0.3, 0.0, 0.0, 1.0, 0.0, 0.0, 64.0]],
        32,
        field,
    )?;
    assert!(actual[0] > 0.5 && actual[14] > 0.5 && actual[2] <= 0.5 && actual[3] >= 0.5);
    Ok(())
}

#[test]
fn leaf_gpu_reads_one_summed_scalar_normals_and_conservative_pyramids()
-> Result<(), Box<dyn std::error::Error>> {
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
    let mut inputs = [[0.0; 8]; 16];
    let mut nodes = [Node::default(); 16];
    for (index, point) in points.into_iter().enumerate() {
        let i = u32::try_from(index)?;
        let depth = if index >= 12 { 19 } else { i };
        let side = 1_u32 << depth;
        let ij = if index >= 12 {
            [side / 2 + i - 12, side / 2]
        } else {
            [(i * 7919) % side, (i * 1999) % side]
        };
        nodes[index] = Node { ij, depth };
        inputs[index] = [
            point[0],
            point[1],
            0.0,
            f32::from(u16::try_from(depth)?),
            f32::from(u16::try_from(ij[0] % 65_536)?),
            f32::from(u16::try_from(ij[1] % 65_536)?),
            0.0,
            0.0,
        ];
        // Each 19-bit dyadic index is exactly representable in the shader's f32 input.
        inputs[index][4] =
            f32::from(u16::try_from(ij[0] >> 8)?) * 256.0 + f32::from(u8::try_from(ij[0] & 255)?);
        inputs[index][5] =
            f32::from(u16::try_from(ij[1] >> 8)?) * 256.0 + f32::from(u8::try_from(ij[1] & 255)?);
    }
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
        if row[15] < 0.5 {
            assert!(LEAF.bounds(nodes[index])?.is_none());
            continue;
        }
        assert!(row[19] > 0.5);
        let [lo, hi] = nodes[index].domain()?;
        for probe in 0..16 {
            let xy = [f64::from(probe % 4) / 3.0, f64::from(probe / 4) / 3.0];
            let xy = [0, 1].map(|axis| xy[axis].mul_add(hi[axis] - lo[axis], lo[axis]));
            let sample = LEAF.sample(xy)?;
            if sample.coverage >= 127.5 {
                assert!(
                    f64::from(row[14]) <= sample.height && f64::from(row[18]) >= sample.height,
                    "node{index} excludes {}",
                    sample.height
                );
            }
        }
    }
    Ok(())
}
