use super::types::V;

pub(super) fn dot(a: V, b: V) -> f64 {
    a[2].mul_add(b[2], a[1].mul_add(b[1], a[0] * b[0]))
}

pub(super) fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(super) fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(super) fn scale(a: V, factor: f64) -> V {
    [a[0] * factor, a[1] * factor, a[2] * factor]
}

pub(super) fn cross(a: V, b: V) -> V {
    [
        a[2].mul_add(-b[1], a[1] * b[2]),
        a[0].mul_add(-b[2], a[2] * b[0]),
        a[1].mul_add(-b[0], a[0] * b[1]),
    ]
}

pub(super) fn madd(a: V, b: V, factor: f64) -> V {
    add(a, scale(b, factor))
}

pub(super) fn safe_normal(a: V, fallback: V) -> V {
    let length = dot(a, a).sqrt();
    if length > f64::EPSILON {
        scale(a, 1.0 / length)
    } else {
        fallback
    }
}

pub(super) fn sphere_uv(n: V) -> [f64; 2] {
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

pub(super) fn plane_uv(p: V, a: V, n: V) -> [f64; 2] {
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
