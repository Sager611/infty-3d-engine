use super::{
    types::{Ray, Sample, V},
    vector::{dot, madd, plane_uv, safe_normal, scale, sub},
};

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
