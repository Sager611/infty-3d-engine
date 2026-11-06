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
    pub const fn packed(self, ray: Ray) -> [f64; 16] {
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
            0.0,
            self.t.mul_add(ray.d[0], ray.o[0]),
            self.t.mul_add(ray.d[1], ray.o[1]),
            self.t.mul_add(ray.d[2], ray.o[2]),
            0.0,
        ]
    }
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
    if horizontal <= 0.0 {
        return [0.0, (n[1].clamp(-1.0, 1.0)).acos() / std::f64::consts::PI];
    }
    let mut u = n[2].atan2(n[0]) / (2.0 * std::f64::consts::PI) + 0.5;
    if u >= 1.0 {
        u -= 1.0;
    }
    [u, (n[1].clamp(-1.0, 1.0)).acos() / std::f64::consts::PI]
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
        let packed = s.packed(r);
        close(packed[0], 0.0);
        close(packed[1], 0.0);
        close(packed[2], 2.0);
        close_v([packed[5], packed[6], packed[7]], s.p);
        close_v([packed[12], packed[13], packed[14]], r.o);
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
