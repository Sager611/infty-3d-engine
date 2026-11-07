use super::{
    types::{Ray, Sample, V},
    vector::{add, dot, madd, safe_normal, scale, sphere_uv, sub},
};

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
