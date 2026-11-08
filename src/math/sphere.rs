use super::{
    types::{Ray, Sample, V},
    vector::{add, dot, madd, safe_normal, scale, sphere_uv, sub},
};

#[cfg(test)]
#[must_use]
pub fn sphere(ray: Ray, c: V, r: f64) -> Sample {
    sphere_at(ray, c, r)
}

#[must_use]
pub fn sphere_at(ray: Ray, c: V, r: f64) -> Sample {
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

#[cfg(test)]
mod tests {
    use super::{Ray, sphere, sphere_at};

    fn ray(o: [f64; 3], d: [f64; 3]) -> Ray {
        Ray { o, d }
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1.0e-12, "{a} != {b}");
    }

    fn same(a: super::Sample, b: super::Sample) {
        assert_eq!(a.hit, b.hit);
        close(a.t, b.t);
        close(a.gap, b.gap);
        for i in 0..3 {
            close(a.p[i], b.p[i]);
            close(a.n[i], b.n[i]);
        }
        for i in 0..2 {
            close(a.uv[i], b.uv[i]);
        }
    }

    #[test]
    fn translated_sphere_uses_explicit_center() {
        let r = ray([2.0, 0.0, -3.0], [0.0, 0.0, 1.0]);
        let sample = sphere_at(r, [2.0, 0.0, 0.0], 1.0);
        assert!(sample.hit);
        close(sample.t, 2.0);
        for (actual, expected) in sample.p.into_iter().zip([2.0, 0.0, -1.0]) {
            close(actual, expected);
        }
    }

    #[test]
    fn sphere_wrapper_preserves_legacy_center() {
        let r = ray([0.0, 1.0, -3.0], [0.0, 0.0, 1.0]);
        same(
            sphere(r, [0.0, 1.0, 0.0], 1.0),
            sphere_at(r, [0.0, 1.0, 0.0], 1.0),
        );
    }
}
