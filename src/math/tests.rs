use super::vector::{add, scale};
use super::{Ray, Sample, V, camera, footprint, plane, pureweight, sphere};

const EPS: f64 = 1.0e-10;

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < EPS, "{a} != {b}");
}

fn close_v(a: V, b: V) {
    for i in 0..3 {
        close(a[i], b[i]);
    }
}

fn ray(o: V, d: V) -> Ray {
    Ray { o, d }
}

fn assert_miss(s: Sample, t: f64, gap: f64) {
    assert!(!s.hit);
    close(s.t, t);
    close(s.gap, gap);
}

#[test]
fn sphere_uv_preserves_tiny_polar_angle() {
    let n = [1.0e-10, 1.0, 0.0];
    let uv = super::vector::sphere_uv(n);
    assert!(uv[1] > 0.0);
    close(uv[1], n[0].hypot(n[2]).atan2(n[1]) / std::f64::consts::PI);
}

#[test]
fn pureweight_is_periodic_bounded_and_varies_at_deep_zoom() {
    let cases = [[0.13, 0.27], [0.41, 0.83], [0.76, 0.19]];
    for uv in cases {
        let a = pureweight(uv, 0.0);
        let b = pureweight([uv[0] + 2.0, uv[1] - 3.0], 0.0);
        assert!(a.is_finite() && (0.0..=1.0).contains(&a));
        close(a, b);
    }
    for uv in cases {
        let values = [
            pureweight([uv[0], uv[1]], 1.0e-6),
            pureweight([uv[0] + 0.003, uv[1] - 0.002], 1.0e-6),
            pureweight([uv[0] - 0.004, uv[1] + 0.001], 1.0e-6),
        ];
        assert!(
            values
                .iter()
                .all(|x| x.is_finite() && (0.0..=1.0).contains(x))
        );
        assert!((values[0] - values[1]).abs() > EPS || (values[1] - values[2]).abs() > EPS);
    }
}

#[test]
fn pureweight_is_continuous_at_lod_and_cell_boundaries() {
    let uv = [0.271, 0.619];
    let threshold = 1.0 / 32.0;
    assert!(
        (pureweight(uv, threshold - 1.0e-9) - pureweight(uv, threshold + 1.0e-9)).abs() < 1.0e-5
    );
    let seam = [0.25, 0.619];
    assert!(
        (pureweight(seam, 1.0e-6) - pureweight([seam[0] + 1.0e-9, seam[1]], 1.0e-6)).abs() < 1.0e-5
    );
}

#[test]
fn footprint_is_conservative_and_finite() {
    let r = ray([0.0, 4.0, 0.0], [0.0, -1.0, 0.0]);
    let s = plane(r, [0.0; 3], [0.0, 1.0, 0.0]);
    let coarse = footprint(s, r, 2.0, 1.0, false);
    let fine = footprint(s, r, 8.0, 1.0, false);
    assert_eq!(coarse.to_bits(), 1.0f64.to_bits());
    assert!(fine < coarse && fine > 0.0);
    let hit = sphere(ray([0.0, 0.0, -3.0], [0.0, 0.0, 1.0]), [0.0; 3], 1.0);
    let base = footprint(hit, ray([0.0, 0.0, -3.0], [0.0, 0.0, 1.0]), 2.0, 1.0, true);
    let double_height = footprint(hit, ray([0.0, 0.0, -3.0], [0.0, 0.0, 1.0]), 4.0, 1.0, true);
    let farther = footprint(
        Sample {
            t: hit.t * 2.0,
            ..hit
        },
        ray([0.0, 0.0, -3.0], [0.0, 0.0, 1.0]),
        2.0,
        1.0,
        true,
    );
    assert!(base > 0.0 && double_height > 0.0 && farther > base);
    close(double_height * 2.0, base);
    assert_eq!(
        footprint(Sample { hit: false, ..s }, r, 2.0, 1.0, false).to_bits(),
        1.0f64.to_bits()
    );
    close(
        footprint(s, r, 2.0, 1.0, false),
        footprint(s, r, 2.0, 1.0, false),
    );
    let pole = Sample {
        hit: true,
        t: 1.0,
        n: [0.0, 1.0, 0.0],
        ..s
    };
    assert_eq!(
        footprint(pole, r, 2.0, 1.0, true).to_bits(),
        1.0f64.to_bits()
    );
    let grazing = Sample {
        n: [1.0, 0.0, 0.0],
        ..s
    };
    assert_eq!(
        footprint(grazing, r, 2.0, 1.0, false).to_bits(),
        1.0f64.to_bits()
    );
}

#[test]
fn sphere_table_and_surface_invariants() {
    let cases = [
        (ray([0.0, 0.0, -3.0], [0.0, 0.0, 1.0]), true, 2.0, 0.0),
        (ray([0.0, 0.0, -3.0], [0.0, 1.0, 0.0]), false, 0.0, 2.0),
        (ray([0.0, 0.0, -3.0], [0.0, 0.0, -1.0]), false, 0.0, 2.0),
        (ray([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]), true, 1.0, 0.0),
        (ray([1.0, 0.0, -3.0], [0.0, 0.0, 1.0]), true, 3.0, 0.0),
    ];
    for (r, hit, t, gap) in cases {
        let s = sphere(r, [0.0; 3], 1.0);
        assert_eq!(s.hit, hit);
        close(s.t, t);
        close(s.gap, gap);
        if hit {
            close_v(
                s.p,
                [
                    t.mul_add(r.d[0], r.o[0]),
                    t.mul_add(r.d[1], r.o[1]),
                    t.mul_add(r.d[2], r.o[2]),
                ],
            );
            close(
                s.p[2]
                    .mul_add(s.p[2], s.p[1].mul_add(s.p[1], s.p[0] * s.p[0]))
                    .sqrt(),
                1.0,
            );
            close_v(s.n, s.p);
            assert!((0.0..1.0).contains(&s.uv[0]));
            assert!((0.0..=1.0).contains(&s.uv[1]));
        }
    }
}

#[test]
fn sphere_miss_point_uv_and_packed_query() {
    let r = ray([0.0, 0.0, -3.0], [0.0, 1.0, 0.0]);
    let s = sphere(r, [0.0; 3], 1.0);
    assert_miss(s, 0.0, 2.0);
    close_v(s.p, [0.0, 0.0, -1.0]);
    close_v(s.n, [0.0, 0.0, -1.0]);
    close(s.uv[0], 0.25);
    close(s.uv[1], 0.5);
    let packed = s.packed(r, 0.25);
    close(packed[0], 0.0);
    close(packed[1], 0.0);
    close(packed[2], 2.0);
    close_v([packed[5], packed[6], packed[7]], s.p);
    close_v([packed[12], packed[13], packed[14]], r.o);
    close(packed[11], pureweight(s.uv, 0.25));
    close(packed[15], 0.25);
}

#[test]
fn plane_table_parallel_coplanar_and_uv() {
    let cases = [
        (ray([2.0, 3.0, 4.0], [0.0, -1.0, 0.0]), true, 3.0, 0.0),
        (ray([2.0, 3.0, 4.0], [0.0, 1.0, 0.0]), false, 0.0, 3.0),
        (ray([2.0, 0.0, 4.0], [1.0, 0.0, 0.0]), true, 0.0, 0.0),
        (ray([2.0, 3.0, 4.0], [1.0, 0.0, 0.0]), false, 0.0, 3.0),
    ];
    for (r, hit, t, gap) in cases {
        let s = plane(r, [0.0; 3], [0.0, 1.0, 0.0]);
        assert_eq!(s.hit, hit);
        close(s.t, t);
        close(s.gap, gap);
        close_v(s.n, [0.0, 1.0, 0.0]);
        close(s.uv[0], 2.0);
        close(s.uv[1], 4.0);
        if hit {
            close_v(s.p, [2.0, 0.0, 4.0]);
        }
    }
}

#[test]
fn plane_precision_gap_normal_projection_and_basis() {
    let shallow = plane(
        ray([0.0, 1.0, 0.0], [0.0, -1.0e-17, 1.0]),
        [0.0; 3],
        [0.0, 1.0, 0.0],
    );
    assert!(shallow.hit);
    assert!(shallow.t > 1.0e16);
    close(shallow.gap, 0.0);
    close_v(shallow.n, [0.0, 1.0, 0.0]);
    let behind = plane(
        ray([2.0, 3.0, 4.0], [0.0, 1.0, 0.0]),
        [0.0; 3],
        [0.0, 1.0, 0.0],
    );
    assert_miss(behind, 0.0, 3.0);
    close_v(behind.p, [2.0, 0.0, 4.0]);
    close_v([behind.uv[0], behind.uv[1], 0.0], [2.0, 4.0, 0.0]);
    let n = [0.0, 0.8, 0.6];
    let a = [1.0, 2.0, 3.0];
    let tangent = [-1.0, 0.0, 0.0];
    let bitangent = [0.0, 0.6, -0.8];
    let p = add(add(a, scale(tangent, 2.0)), scale(bitangent, 3.0));
    let tilted = plane(ray(add(p, scale(n, 5.0)), scale(n, -1.0)), a, n);
    assert!(tilted.hit);
    close(tilted.t, 5.0);
    close_v(tilted.n, n);
    close(tilted.uv[0], 2.0);
    close(tilted.uv[1], 3.0);
}

#[test]
fn camera_origin_and_pixel_consistency() {
    let view = [0.0, 0.0, 5.0, 1.0];
    let center = camera(view, [200.0, 100.0], [100.0, 50.0]);
    close_v(center.o, [0.0, 1.0, 5.0]);
    close_v(center.d, [0.0, 0.0, -1.0]);
    let left = camera(view, [200.0, 100.0], [0.0, 50.0]);
    let right = camera(view, [200.0, 100.0], [200.0, 50.0]);
    assert!(left.d[0] < center.d[0]);
    assert!(right.d[0] > center.d[0]);
    for r in [center, left, right] {
        close(
            r.d[2]
                .mul_add(r.d[2], r.d[1].mul_add(r.d[1], r.d[0] * r.d[0]))
                .sqrt(),
            1.0,
        );
    }
}
