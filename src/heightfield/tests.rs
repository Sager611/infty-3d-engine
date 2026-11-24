#![expect(clippy::float_cmp, reason = "Exact layout and same-surface identities")]
use super::*;

#[test]
fn leaf_exaggerates_only_its_broadest_geometry_band() -> Result<(), Error> {
    assert_eq!(AMPLITUDES, [1.0, 1.0 / 1024.0, 1.0 / 8192.0, 1.0 / 32768.0]);
    let original = Field::new(
        DATA,
        512,
        [1.0 / 32.0, AMPLITUDES[1], AMPLITUDES[2], AMPLITUDES[3]],
        LEAF.anchor(),
    )?;
    let added = Field::new(DATA, 512, [31.0 / 32.0, 0.0, 0.0, 0.0], LEAF.anchor())?;
    let mut changed = false;
    for point in [[0.0; 2], [-0.5, 0.25], [0.75, -0.25], [1.0; 2]] {
        let before = original.sample(point)?;
        let after = LEAF.sample(point)?;
        let broad = added.sample(point)?;
        assert!((after.height - before.height - broad.height).abs() < 1e-15);
        for axis in 0..2 {
            assert!(
                (after.gradient[axis] - before.gradient[axis] - broad.gradient[axis]).abs() < 1e-12
            );
        }
        assert_eq!(after.coverage, before.coverage);
        changed |= broad.height.abs() > 1e-6;
    }
    assert!(changed);
    Ok(())
}

#[test]
fn blade_rises_smoothly_from_stem_to_either_edge() -> Result<(), Error> {
    for (y, stem, edges) in [
        (168.0, 237.0, [172.0_f64, 302.0]),
        (256.0, 234.0, [146.0, 322.0]),
        (384.0, 232.0, [169.0, 295.0]),
    ] {
        let at = |x: f64| LEAF.sample([x.mul_add(1.0 / 256.0, -1.0), y / 256.0 - 1.0]);
        let spine = at(stem)?.height;
        assert!(spine < -0.06, "the stem must sit below the carrier");
        for edge in edges {
            let mid = (edge - stem).mul_add(0.5, stem);
            let outer = (edge - stem).mul_add(0.85, stem);
            let inner = at(mid)?;
            let blade = at(outer)?;
            assert!(blade.coverage >= 127.5 && inner.coverage >= 127.5);
            assert!(inner.height > spine + 0.035 && blade.height > inner.height + 0.095);
            assert!(blade.height > spine + if y == 256.0 { 0.32 } else { 0.15 });
        }
    }
    Ok(())
}

#[test]
fn checked_fields_require_a_layout_and_a_covered_anchor() -> Result<(), Error> {
    let field = Field::new(DATA, 512, AMPLITUDES, [0.0; 2])?;
    assert_eq!(
        field.sample([0.0; 2])?.height,
        LEAF.sample([0.0; 2])?.height
    );
    assert!(Field::new(DATA, 511, AMPLITUDES, [0.0; 2]).is_err());
    assert!(Field::new(&DATA[..DATA.len() - 1], 512, AMPLITUDES, [0.0; 2]).is_err());
    assert!(Field::new(DATA, 512, [f64::NAN; 4], [0.0; 2]).is_err());
    assert!(Field::new(DATA, 512, [f64::MAX; 4], [0.0; 2]).is_err());
    assert!(Field::new(DATA, 512, AMPLITUDES, [-0.96; 2]).is_err());
    Ok(())
}

#[test]
fn gpu_header_preserves_scalar_bytes_and_exact_canonical_amplitudes()
-> Result<(), Box<dyn std::error::Error>> {
    let bytes = LEAF.gpu_bytes()?;
    assert_eq!(bytes.len(), 5 * 512 * 512 + 32);
    for (index, expected) in [512, 262_144, 512_000, 1].into_iter().enumerate() {
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
    for (gpu, cpu) in bytes[32..]
        .chunks_exact(512 * 512)
        .zip(DATA.chunks_exact(436_908))
    {
        assert_eq!(gpu, &cpu[..512 * 512]);
    }
    assert!(
        Field {
            amplitudes: [0.1; 4],
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
        amplitudes: AMPLITUDES,
        anchor: [0.0; 2],
        cup: false,
    }
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
            amplitudes,
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
