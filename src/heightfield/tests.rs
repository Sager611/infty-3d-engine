#![expect(clippy::float_cmp, reason = "Exact layout and same-surface identities")]
use super::*;

#[test]
fn any_height_order_count_follows_the_data() -> Result<(), Error> {
    for (orders, byte) in [(1_usize, 200_u8), (2, 64), (6, 150)] {
        let mut pixels = Vec::new();
        for _ in 0..orders {
            pixels.extend_from_slice(&tile([byte; 4]));
        }
        pixels.extend_from_slice(&tile([255; 4]));
        let amplitudes: Vec<f64> = (0..orders)
            .map(|o| 1.0 / f64::from(1_u32 << (2 * o + 3)))
            .collect();
        let field = Field::new(&pixels, 2, &amplitudes, [0.0; 2])?;
        let expected: f64 = amplitudes
            .iter()
            .map(|a| a * (f64::from(byte) - 128.0) / 255.0)
            .sum();
        let sample = field.sample([0.31, -0.42])?;
        assert!(
            (sample.height - expected).abs() < 1e-12,
            "{orders}: {}",
            sample.height
        );
        let total: f64 = amplitudes.iter().map(|a| a.abs()).sum();
        assert_eq!(field.reach(), (total * (128.0 / 255.0)).next_up());
        let finest = u32::try_from(orders - 1).map_err(|_| Error::Numeric)?;
        assert_eq!(field.resolution()?, 2 * 10_u32.pow(finest));
        let bounds = field.bounds(Node::default())?.ok_or(Error::Numeric)?;
        assert!(bounds.lo[2] <= sample.height && sample.height <= bounds.hi[2]);
        assert!(Field::new(&pixels[8..], 2, &amplitudes, [0.0; 2]).is_err());
        let header = 16 + 16 * orders.div_ceil(4);
        assert_eq!(field.gpu_bytes()?.len(), header + pixels.len());
    }
    Ok(())
}
#[test]
fn leaf_preserves_fine_height_gains() {
    assert_eq!(AMPLITUDES[1..], [1.0 / 1024.0, 1.0 / 8192.0, 1.0 / 32768.0]);
}
#[test]
fn leaf_fine_bands_do_not_repeat_one_tile_apart() -> Result<(), Error> {
    let pixels = data([
        [128; 4],
        [128; 4],
        [16, 96, 208, 240],
        [16, 96, 208, 240],
        [255; 4],
    ]);
    for order in 2..4 {
        let mut amplitudes = [0.0; 4];
        amplitudes[order] = 1.0 / 1024.0;
        let field = Field::new(&pixels, 2, &amplitudes, [0.0; 2])?;
        let mut different = 0.0_f64;
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        for n in 0..8 {
            let p = [
                f64::from(n).mul_add(0.137, -0.75),
                f64::from(n).mul_add(0.073, -0.413),
            ];
            let a = field.sample(p)?;
            let b = field.sample([p[0] + 2.0 / f64::from(repeat(order)), p[1]])?;
            different = different.max((a.height - b.height).abs());
            low = low.min(a.height);
            high = high.max(a.height);
            assert_eq!(a.coverage, 255.0);
        }
        assert!(
            different > 1e-5,
            "fine band {order} is still an exact tile: {different}"
        );
        assert!(high - low > 1e-4, "fine band {order} was flattened");
    }
    Ok(())
}

#[test]
fn leaf_fine_phase_wraps_without_seams_and_differentiates_the_same_field() -> Result<(), Error> {
    let pixels = data([
        [128; 4],
        [128; 4],
        [16, 96, 208, 240],
        [240, 32, 192, 64],
        [255; 4],
    ]);
    for order in 2..4 {
        let mut amplitudes = [0.0; 4];
        amplitudes[order] = 0.125;
        let field = Field::new(&pixels, 2, &amplitudes, [0.0; 2])?;
        for axis in 0..2 {
            let mut a = [0.12345; 2];
            let mut b = a;
            a[axis] = -1.0;
            b[axis] = 1.0;
            let left = field.sample(a)?;
            let right = field.sample(b)?;
            assert!(
                (left.height - right.height).abs() < 1e-11,
                "fine seam {order}"
            );
            for component in 0..2 {
                assert!((left.gradient[component] - right.gradient[component]).abs() < 1e-6);
            }
            let p = [-0.12733, 0.24534];
            let step = 1e-5 / f64::from(repeat(order));
            let mut lo = p;
            let mut hi = p;
            lo[axis] -= step;
            hi[axis] += step;
            let numerical = (field.sample(hi)?.height - field.sample(lo)?.height) / (2.0 * step);
            assert!((field.sample(p)?.gradient[axis] - numerical).abs() < 1e-5);
        }
    }
    Ok(())
}

#[test]
fn leaf_fine_nodes_enclose_samples_and_keep_refining() -> Result<(), Error> {
    let pixels = data([
        [128; 4],
        [128; 4],
        [16, 96, 208, 240],
        [240, 32, 192, 64],
        [255; 4],
    ]);
    let field = Field::new(&pixels, 2, &[0.0, 0.0, 0.125, -0.125], [0.0; 2])?;
    let mut seed = 17_u32;
    for depth in [0, 4, 12, 18, 24] {
        for _ in 0..32 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let x = seed % (1 << depth);
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let node = Node {
                ij: [x, seed % (1 << depth)],
                depth,
            };
            let [lo, hi] = node.domain()?;
            let bounds = field.bounds(node)?.ok_or(Error::Numeric)?;
            for x in [0.0, 0.17, 0.5, 0.83, 1.0] {
                for y in [0.0, 0.17, 0.5, 0.83, 1.0] {
                    let p = [x, y];
                    let at = [0, 1].map(|axis| (hi[axis] - lo[axis]).mul_add(p[axis], lo[axis]));
                    let value = field.sample(at)?;
                    assert!(
                        bounds.lo[2] <= value.height && value.height <= bounds.hi[2],
                        "{depth} {:?}: {} outside [{}, {}]",
                        node.ij,
                        value.height,
                        bounds.lo[2],
                        bounds.hi[2]
                    );
                }
            }
            if depth == 24 {
                assert!(bounds.hi[2] - bounds.lo[2] < 0.0001);
            }
        }
    }
    Ok(())
}

const HALF_SIZE_MM: f64 = 70.0;

#[test]
fn leaf_depth_tracks_the_source_tip_blade_and_veins() -> Result<(), Error> {
    for (pixel, height_mm) in [
        ([230.0, 94.0], 1.53),
        ([229.0, 330.0], -3.40),
        ([190.0, 370.0], -1.37),
        ([210.0, 370.0], -2.78),
        ([253.0, 370.0], -1.34),
        ([272.0, 370.0], -2.13),
    ] {
        let sample = LEAF.sample(pixel.map(|p| p / 256.0 - 1.0))?;
        assert!(sample.coverage >= 127.5);
        let actual = sample.height * HALF_SIZE_MM;
        assert!((actual - height_mm).abs() < 0.12, "{pixel:?}: {actual} mm");
    }
    Ok(())
}

#[test]
fn checked_fields_require_a_layout_and_a_covered_anchor() -> Result<(), Error> {
    let field = Field::new(DATA, 512, &AMPLITUDES, [0.0; 2])?;
    assert_eq!(
        field.sample([0.0; 2])?.height,
        LEAF.sample([0.0; 2])?.height
    );
    assert!(Field::new(DATA, 511, &AMPLITUDES, [0.0; 2]).is_err());
    assert!(Field::new(&DATA[..DATA.len() - 1], 512, &AMPLITUDES, [0.0; 2]).is_err());
    assert!(Field::new(DATA, 512, &[f64::NAN; 4], [0.0; 2]).is_err());
    assert!(Field::new(DATA, 512, &[f64::MAX; 4], [0.0; 2]).is_err());
    assert!(Field::new(DATA, 512, &AMPLITUDES, [-0.96; 2]).is_err());
    Ok(())
}

#[test]
fn gpu_header_preserves_scalar_bytes_and_exact_canonical_amplitudes()
-> Result<(), Box<dyn std::error::Error>> {
    let bytes = LEAF.gpu_bytes()?;
    assert_eq!(bytes.len(), DATA.len() + 32);
    for (index, expected) in [512, 436_908, 512_000, 1].into_iter().enumerate() {
        assert_eq!(
            u32::from_le_bytes(bytes[index * 4..index * 4 + 4].try_into()?),
            expected
        );
    }
    for (index, expected) in AMPLITUDES.into_iter().enumerate() {
        assert_eq!(
            f64::from(f32::from_le_bytes(
                bytes[16 + index * 4..20 + index * 4].try_into()?
            )),
            expected
        );
    }
    assert_eq!(&bytes[32..], DATA);
    assert!(
        Field {
            amplitudes: &[0.1; 4],
            ..LEAF
        }
        .gpu_bytes()
        .is_err()
    );
    Ok(())
}

#[path = "point_tests.rs"]
mod point_tests;
fn tile(v: [u8; 4]) -> [u8; 8] {
    [
        v[0],
        v[1],
        v[2],
        v[3],
        v[0].min(v[1]).min(v[2]).min(v[3]),
        v[0].max(v[1]).max(v[2]).max(v[3]),
        0,
        0,
    ]
}
fn data(tiles: [[u8; 4]; 5]) -> [u8; 40] {
    let mut out = [0; 40];
    for i in 0..5 {
        out[i * 8..i * 8 + 8].copy_from_slice(&tile(tiles[i]));
    }
    out
}
fn fixture(data: &[u8]) -> Field<'_> {
    Field {
        data,
        edge: 2,
        amplitudes: &AMPLITUDES,
        anchor: [0.0; 2],
    }
}

#[test]
fn gpu_marks_only_zero_coverage_borders() -> Result<(), Error> {
    for coverage in [128, 255] {
        let pixels = data([[128; 4], [128; 4], [128; 4], [128; 4], [coverage; 4]]);
        let bytes = fixture(&pixels).gpu_bytes()?;
        assert_eq!(&bytes[12..16], &u32::from(coverage == 0).to_le_bytes());
    }
    assert_eq!(&LEAF.gpu_bytes()?[12..16], &1_u32.to_le_bytes());
    Ok(())
}

#[test]
fn field_source_and_root_node_have_explicit_immutable_layouts() {
    assert_eq!(LEAF.edge, 512);
    assert_eq!(LEAF.data.len(), 2_184_540);
    assert_eq!(LEAF.amplitudes, AMPLITUDES);
    assert_eq!(LEAF.anchor(), [0.0; 2]);
    assert_eq!(Node::default().ij, [0; 2]);
    assert_eq!(Node::default().depth, 0);
}

#[test]
fn four_height_orders_add_before_geometry_or_normals() -> Result<(), Error> {
    let values = [133, 148, 111, 190];
    let pixels = data([[values[0]; 4], [148; 4], [111; 4], [190; 4], [255; 4]]);
    let field = fixture(&pixels);
    let actual = field.sample([0.125, -0.25])?;
    let mut expected = 0.0;
    for i in 0..4 {
        let contribution = AMPLITUDES[i] * (f64::from(values[i]) - 128.0) / 255.0;
        expected += contribution;
        let mut amplitudes = AMPLITUDES;
        amplitudes[i] = 0.0;
        let without = Field {
            amplitudes: &amplitudes,
            ..field
        }
        .sample([0.125, -0.25])?;
        assert!((actual.height - without.height - contribution).abs() < 1e-16);
    }
    assert!((actual.height - expected).abs() < 1e-16);
    assert_eq!(actual.gradient, [0.0; 2]);
    assert_eq!(actual.normal(), [0.0, 0.0, 1.0]);
    assert!(actual.coverage >= 127.5);
    Ok(())
}

#[test]
fn field_normals_differentiate_the_same_bilinear_scalar() -> Result<(), Error> {
    let pixels = data([[128, 160, 192, 240], [128; 4], [128; 4], [128; 4], [255; 4]]);
    let field = fixture(&pixels);
    let p = [-0.5, -0.5];
    let value = field.sample(p)?;
    for axis in 0..2 {
        let mut a = p;
        let mut b = p;
        a[axis] -= 1e-5;
        b[axis] += 1e-5;
        let derivative = (field.sample(b)?.height - field.sample(a)?.height) / 2e-5;
        assert!((derivative - value.gradient[axis]).abs() < 1e-10);
    }
    assert!(value.normal()[2] > 0.0);
    assert!(field.sample([f64::NAN, 0.0]).is_err());
    assert!(field.sample([1.001, 0.0]).is_err());
    Ok(())
}

#[test]
fn conservative_node_bounds_cover_all_four_orders_and_independent_cutout() -> Result<(), Error> {
    let pixels = data([
        [10, 90, 170, 250],
        [200, 20, 240, 80],
        [40, 230, 70, 190],
        [190, 10, 210, 110],
        [0, 0, 0, 255],
    ]);
    let field = fixture(&pixels);
    for node in [
        Node::default(),
        Node {
            ij: [1, 2],
            depth: 2,
        },
        Node {
            ij: [4, 3],
            depth: 3,
        },
        Node {
            ij: [262_144, 262_144],
            depth: 19,
        },
    ] {
        let bounds = field.bounds(node)?.ok_or(Error::Numeric)?;
        for index in 0..16 {
            let x = f64::from(index % 4) / 3.0;
            let y = f64::from(index / 4) / 3.0;
            let p = [
                x.mul_add(bounds.hi[0] - bounds.lo[0], bounds.lo[0]),
                y.mul_add(bounds.hi[1] - bounds.lo[1], bounds.lo[1]),
            ];
            let value = field.sample(p)?;
            if value.coverage >= 127.5 {
                assert!(bounds.lo[2] <= value.height && bounds.hi[2] >= value.height);
            }
        }
    }
    assert!(
        field
            .bounds(Node {
                ij: [0, 0],
                depth: 3
            })?
            .is_none()
    );
    assert!(
        field
            .bounds(Node {
                ij: [8, 0],
                depth: 2
            })
            .is_err()
    );
    Ok(())
}

#[test]
fn preset_height_bytes_equal_the_uploaded_rgba_height_channels()
-> Result<(), Box<dyn std::error::Error>> {
    let files: [&[u8]; 4] = [
        include_bytes!("../material/tiles/leaf-0.rgba.z"),
        include_bytes!("../material/tiles/leaf-1.rgba.z"),
        include_bytes!("../material/tiles/leaf-2.rgba.z"),
        include_bytes!("../material/tiles/leaf-3.rgba.z"),
    ];
    for order in 0..4 {
        let rgba =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(files[order], 512 * 512 * 4)
                .map_err(|e| format!("{e:?}"))?;
        assert_eq!(rgba.len(), 512 * 512 * 4);
        for probe in 0..16 {
            let pixel = (probe * 7919 + 32767) % (512 * 512);
            assert_eq!(rgba[pixel * 4 + 3], DATA[order * 436_908 + pixel]);
        }
    }
    assert!(LEAF.sample([0.0; 2])?.coverage >= 127.5);
    assert!(LEAF.sample([-0.96; 2])?.coverage < 127.5);
    assert_eq!(STEPS, 64);
    Ok(())
}

#[test]
fn coverage_outline_is_a_small_tight_superset_of_every_covered_uv() -> Result<(), Error> {
    let outline = LEAF.outline(16)?;
    assert!((3..=16).contains(&outline.len()), "{} sides", outline.len());
    let inside = |p: [f64; 2]| {
        (0..outline.len()).all(|k| cross(outline[k], outline[(k + 1) % outline.len()], p) >= 0.0)
    };
    let (mut covered, mut enclosed) = (0_u32, 0_u32);
    for j in 0..512_u32 {
        for i in 0..512_u32 {
            let uv = [(f64::from(i) + 0.37) / 512.0, (f64::from(j) + 0.71) / 512.0];
            let hit = LEAF.uv(uv)?.coverage >= 127.5;
            assert!(!hit || inside(uv), "covered UV outside the outline: {uv:?}");
            covered += u32::from(hit);
            enclosed += u32::from(inside(uv));
        }
    }
    assert!(
        f64::from(enclosed) < 1.35 * f64::from(covered),
        "{enclosed} vs {covered}"
    );
    Ok(())
}
