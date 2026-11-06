#![allow(clippy::many_single_char_names, reason = "Matches analytic equations.")]

pub type V = [f64; 3];

#[derive(Clone, Copy)]
pub struct Ray {
    pub o: V,
    pub d: V,
}

#[derive(Clone, Copy)]
pub struct Sample {
    pub hit: bool,
    pub t: f64,
    pub gap: f64,
    pub p: V,
    pub n: V,
    pub uv: [f64; 2],
}

impl Sample {
    pub fn packed(self, ray: Ray, rho: f64) -> [f64; 16] {
        [
            if self.hit { 1.0 } else { 0.0 },
            self.t,
            self.gap,
            self.uv[0],
            self.uv[1],
            self.p[0],
            self.p[1],
            self.p[2],
            self.n[0],
            self.n[1],
            self.n[2],
            pureweight(self.uv, rho),
            self.t.mul_add(ray.d[0], ray.o[0]),
            self.t.mul_add(ray.d[1], ray.o[1]),
            self.t.mul_add(ray.d[2], ray.o[2]),
            rho,
        ]
    }
}

pub fn pureweight(uv: [f64; 2], rho: f64) -> f64 {
    let rho = rho.clamp(0.0, 1.0);
    let lod = (-(rho.max(1.0e-12) * 32.0).log2()).clamp(0.0, 18.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let level = lod.floor() as u32;
    let t = smoothstep(0.0, 1.0, lod.fract());
    let a = tile(uv, level, rho);
    let b = tile(uv, level + 1, rho);
    let c = tile(uv, level + 2, rho);
    mix(mix(a, b, 0.4), mix(b, c, 0.4), t)
}

fn tile(uv: [f64; 2], level: u32, rho: f64) -> f64 {
    let f = [
        uv[0].abs() - uv[0].abs().floor(),
        uv[1].abs() - uv[1].abs().floor(),
    ];
    let scale_u = 1u32 << level;
    let scale = f64::from(scale_u);
    let s = [f[0] * scale, f[1] * scale];
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let mut cell = [s[0].floor() as u32, s[1].floor() as u32];
    let p = [s[0] - s[0].floor(), s[1] - s[1].floor()];
    if uv[0] < 0.0 {
        cell[0] = (0u32
            .wrapping_sub(cell[0])
            .wrapping_sub(u32::from(p[0] > 0.0)))
            & (scale_u - 1);
    }
    if uv[1] < 0.0 {
        cell[1] = (0u32
            .wrapping_sub(cell[1])
            .wrapping_sub(u32::from(p[1] > 0.0)))
            & (scale_u - 1);
    }
    let edge = p[0].min(1.0 - p[0]).min(p[1].min(1.0 - p[1]));
    let mut hash = cell[0] ^ cell[1].wrapping_mul(0x9e37_79b9) ^ level.wrapping_mul(0x85eb_ca6b);
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7feb_352d);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x846c_a68b);
    hash ^= hash >> 16;
    let fill = 0.6f64.mul_add(f64::from(hash >> 24) / 255.0, 0.2);
    let inside = smoothstep(0.0, rho.mul_add(scale, 0.04), edge);
    let signal = mix(0.15, fill, inside);
    mix(signal, 0.5, smoothstep(0.25, 1.0, rho * scale))
}

fn mix(a: f64, b: f64, t: f64) -> f64 {
    b.mul_add(t, a * (1.0 - t))
}

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * 2.0f64.mul_add(-t, 3.0)
}

pub fn footprint(sample: Sample, ray: Ray, height: f64, radius: f64, sphere: bool) -> f64 {
    if !sample.hit
        || !sample.t.is_finite()
        || sample.t <= 0.0
        || !height.is_finite()
        || height <= 0.0
    {
        return 1.0;
    }
    let nd = ray.d[2].mul_add(
        sample.n[2],
        ray.d[1].mul_add(sample.n[1], ray.d[0] * sample.n[0]),
    );
    if !nd.is_finite() || nd.abs() == 0.0 {
        return 1.0;
    }
    let density = if sphere {
        let radial = sample.n[0].hypot(sample.n[2]);
        if radial == 0.0 || !radius.is_finite() || radius <= 0.0 {
            return 1.0;
        }
        (1.0 / std::f64::consts::PI).max(1.0 / (std::f64::consts::TAU * radial)) / radius
    } else {
        1.0
    };
    (2.0 * sample.t * density / (1.8 * height * nd.abs())).min(1.0)
}

pub fn sphere(ray: Ray, c: V, r: f64) -> Sample {
    let radius = r;
    let oc = sub(ray.o, c);
    let b = dot(oc, ray.d);
    let constant = radius.mul_add(-radius, dot(oc, oc));
    let discriminant = b.mul_add(b, -constant);

    if discriminant >= 0.0 {
        let root = discriminant.sqrt();
        let q = -b - if b >= 0.0 { root } else { -root };
        let roots = if q == 0.0 {
            [-b, -b]
        } else {
            [q, constant / q]
        };
        let mut t = f64::INFINITY;
        for &candidate in &roots {
            if candidate >= 0.0 && candidate < t {
                t = candidate;
            }
        }
        if t.is_finite() {
            let p = madd(ray.o, ray.d, t);
            let normal = safe_normal(sub(p, c), [0.0, 1.0, 0.0]);
            return Sample {
                hit: true,
                t,
                gap: 0.0,
                p,
                n: normal,
                uv: sphere_uv(normal),
            };
        }
    }

    let t = (-b).max(0.0);
    let closest = madd(ray.o, ray.d, t);
    let normal = safe_normal(sub(closest, c), [0.0, 1.0, 0.0]);
    let p = add(c, scale(normal, radius));
    Sample {
        hit: false,
        t,
        gap: (dot(sub(closest, c), sub(closest, c)).sqrt() - radius).max(0.0),
        p,
        n: normal,
        uv: sphere_uv(normal),
    }
}

pub fn plane(ray: Ray, a: V, n: V) -> Sample {
    let normal = safe_normal(n, [0.0, 1.0, 0.0]);
    let h = dot(normal, sub(ray.o, a));
    let q = dot(normal, ray.d);
    if q.abs() > 0.0 {
        let t = -h / q;
        if t >= 0.0 {
            let p = madd(ray.o, ray.d, t);
            return Sample {
                hit: true,
                t,
                gap: 0.0,
                p,
                n: normal,
                uv: plane_uv(p, a, normal),
            };
        }
    } else if h.abs() == 0.0 {
        return Sample {
            hit: true,
            t: 0.0,
            gap: 0.0,
            p: ray.o,
            n: normal,
            uv: plane_uv(ray.o, a, normal),
        };
    }

    let p = sub(ray.o, scale(normal, h));
    Sample {
        hit: false,
        t: 0.0,
        gap: h.abs(),
        p,
        n: normal,
        uv: plane_uv(p, a, normal),
    }
}

pub fn camera(view: [f64; 4], size: [f64; 2], pixel: [f64; 2]) -> Ray {
    let [yaw, pitch, distance, radius] = view;
    let [width, height] = size;
    let target = [0.0, radius, 0.0];
    let orbit = [
        pitch.cos() * yaw.sin(),
        pitch.sin(),
        pitch.cos() * yaw.cos(),
    ];
    let origin = add(target, scale(orbit, distance));
    let forward = safe_normal(sub(target, origin), [0.0, 0.0, -1.0]);
    let right = safe_normal(cross(forward, [0.0, 1.0, 0.0]), [1.0, 0.0, 0.0]);
    let up = cross(right, forward);
    let sx = 2.0f64.mul_add(pixel[0], -width) / height;
    let sy = 2.0f64.mul_add(-pixel[1], height) / height;
    let direction = add(add(scale(forward, 1.8), scale(right, sx)), scale(up, sy));
    Ray {
        o: origin,
        d: safe_normal(direction, forward),
    }
}

fn dot(a: V, b: V) -> f64 {
    a[2].mul_add(b[2], a[1].mul_add(b[1], a[0] * b[0]))
}

fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: V, factor: f64) -> V {
    [a[0] * factor, a[1] * factor, a[2] * factor]
}

fn cross(a: V, b: V) -> V {
    [
        a[2].mul_add(-b[1], a[1] * b[2]),
        a[0].mul_add(-b[2], a[2] * b[0]),
        a[1].mul_add(-b[0], a[0] * b[1]),
    ]
}

fn madd(a: V, b: V, factor: f64) -> V {
    add(a, scale(b, factor))
}

fn safe_normal(a: V, fallback: V) -> V {
    let length = dot(a, a).sqrt();
    if length > f64::EPSILON {
        scale(a, 1.0 / length)
    } else {
        fallback
    }
}

fn sphere_uv(n: V) -> [f64; 2] {
    let horizontal = n[0].hypot(n[2]);
    let v = horizontal.atan2(n[1]) / std::f64::consts::PI;
    if horizontal <= 0.0 {
        return [0.0, v];
    }
    let mut u = n[2].atan2(n[0]) / (2.0 * std::f64::consts::PI) + 0.5;
    if u >= 1.0 {
        u -= 1.0;
    }
    [u, v]
}

fn plane_uv(p: V, a: V, n: V) -> [f64; 2] {
    let reference = if n[1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    let tangent = safe_normal(cross(n, reference), [1.0, 0.0, 0.0]);
    let bitangent = cross(tangent, n);
    let delta = sub(p, a);
    [dot(delta, tangent), dot(delta, bitangent)]
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let uv = sphere_uv(n);
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
            (pureweight(uv, threshold - 1.0e-9) - pureweight(uv, threshold + 1.0e-9)).abs()
                < 1.0e-5
        );
        let seam = [0.25, 0.619];
        assert!(
            (pureweight(seam, 1.0e-6) - pureweight([seam[0] + 1.0e-9, seam[1]], 1.0e-6)).abs()
                < 1.0e-5
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
}
