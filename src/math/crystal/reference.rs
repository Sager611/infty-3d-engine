//! Exhaustive verification model; absent from production builds.
use crate::math::{Ray, V};

fn segments() -> impl Iterator<Item = [f64; 4]> {
    let tiny = 2.0_f64.powi(-20);
    std::iter::once([0.0, 0.0, tiny, 0.5 * tiny]).chain((-20..0).flat_map(|exponent| {
        let s = 2.0_f64.powi(exponent);
        [
            [s, 0.5 * s, 1.5 * s, 3.0 * s],
            [1.5 * s, 3.0 * s, 2.0 * s, s],
        ]
    }))
}
const fn frame(p: V, axis: usize) -> V {
    match axis {
        0 => p,
        1 => [p[1], p[2], p[0]],
        _ => [p[2], p[0], p[1]],
    }
}
const fn dot(a: V, b: V) -> f64 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}
pub fn magnitude(p: V) -> f64 {
    p[0].hypot(p[1]).hypot(p[2])
}

pub fn distance(p: V) -> f64 {
    let mut best = f64::INFINITY;
    for axis in 0..3 {
        let p = frame(p, axis);
        let r = p[0].hypot(p[1]);
        for [r0, z0, r1, z1] in segments() {
            let dr = r1 - r0;
            let dz = z1 - z0;
            let t =
                ((r - r0).mul_add(dr, (p[2] - z0) * dz) / dr.mul_add(dr, dz * dz)).clamp(0.0, 1.0);
            let qr = dr.mul_add(t, r0);
            let qz = dz.mul_add(t, z0);
            best = best.min((r - qr).hypot(p[2] - qz));
        }
    }
    best
}
fn quadratic(a: f64, b: f64, c: f64) -> [f64; 2] {
    if a == 0.0 {
        return if b == 0.0 {
            [f64::INFINITY; 2]
        } else {
            [-c / b; 2]
        };
    }
    let discriminant = b.mul_add(b, -4.0 * a * c);
    if discriminant < 0.0 {
        return [f64::INFINITY; 2];
    }
    let q = -0.5 * b.signum().mul_add(discriminant.sqrt(), b);
    if q == 0.0 {
        [-b / (2.0 * a); 2]
    } else {
        [q / a, c / q]
    }
}
fn parts(value: f64) -> Option<(bool, u64, i32)> {
    if !value.is_finite() {
        return None;
    }
    let bits = value.to_bits();
    let exponent = u16::try_from((bits >> 52) & 0x7ff).ok()?;
    let fraction = bits & 0x000f_ffff_ffff_ffff;
    Some((
        bits >> 63 != 0,
        if exponent == 0 {
            fraction
        } else {
            fraction | (1_u64 << 52)
        },
        if exponent == 0 {
            -1074
        } else {
            i32::from(exponent) - 1075
        },
    ))
}
fn product(a: f64, b: f64) -> Option<(bool, u128, i32)> {
    let (sa, ma, ea) = parts(a)?;
    let (sb, mb, eb) = parts(b)?;
    let m = u128::from(ma) * u128::from(mb);
    if m == 0 {
        return Some((false, 0, 0));
    }
    let zeros = m.trailing_zeros();
    Some((sa != sb, m >> zeros, ea + eb + i32::try_from(zeros).ok()?))
}
fn same_product(a: f64, b: f64, c: f64, d: f64) -> bool {
    product(a, b)
        .zip(product(c, d))
        .is_some_and(|(a, b)| a == b)
}
fn apex(ray: Ray) -> Option<f64> {
    if !(same_product(ray.o[0], ray.d[1], ray.o[1], ray.d[0])
        && same_product(ray.o[0], ray.d[2], ray.o[2], ray.d[0])
        && same_product(ray.o[1], ray.d[2], ray.o[2], ray.d[1]))
    {
        return None;
    }
    let a = ray.d.map(f64::abs);
    let axis = if a[0] >= a[1] && a[0] >= a[2] {
        0
    } else if a[1] >= a[2] {
        1
    } else {
        2
    };
    let t = -ray.o[axis] / ray.d[axis];
    (t.is_finite() && t >= 0.0).then_some(t)
}

pub fn first_hit(ray: Ray) -> Option<f64> {
    let mut first = apex(ray).unwrap_or(f64::INFINITY);
    for axis in 0..3 {
        let o = frame(ray.o, axis);
        let d = frame(ray.d, axis);
        let shift = -dot(o, d) / dot(d, d);
        let center = [
            d[0].mul_add(shift, o[0]),
            d[1].mul_add(shift, o[1]),
            d[2].mul_add(shift, o[2]),
        ];
        let radial = d[0].mul_add(d[0], d[1] * d[1]);
        let radial_b = 2.0 * center[0].mul_add(d[0], center[1] * d[1]);
        let radial_c = center[0].mul_add(center[0], center[1] * center[1]);
        for [r0, z0, r1, z1] in segments() {
            let m = (z1 - z0) / (r1 - r0);
            let b = (-m).mul_add(r0, z0);
            let z = center[2] - b;
            let mm = m * m;
            let [a, c] = quadratic(
                d[2].mul_add(d[2], -mm * radial),
                2.0 * z.mul_add(d[2], -0.5 * mm * radial_b),
                z.mul_add(z, -mm * radial_c),
            );
            let [e, f] = quadratic(radial, radial_b, r0.mul_add(-r0, radial_c));
            let [g, h] = quadratic(radial, radial_b, r1.mul_add(-r1, radial_c));
            for local in [-shift, a, c, e, f, g, h] {
                let t = local + shift;
                if !t.is_finite() || t < 0.0 || t >= first {
                    continue;
                }
                let p = [
                    d[0].mul_add(local, center[0]),
                    d[1].mul_add(local, center[1]),
                    d[2].mul_add(local, center[2]),
                ];
                let r = p[0].hypot(p[1]);
                let slack = 128.0 * f64::EPSILON * (r + p[2].abs() + b.abs());
                if r >= r0 - slack && r <= r1 + slack && (p[2] - m.mul_add(r, b)).abs() <= slack {
                    first = t;
                }
            }
        }
    }
    first.is_finite().then_some(first)
}

#[test]
fn exhaustive_model_checks_all_twenty_bands_and_component_ties()
-> Result<(), super::ArithmeticError> {
    let vectors = [
        [0.2, -0.3, 0.7],
        [2.0, -2.0, 1.0],
        [0.75, 0.0, 1.5],
        [0.0, 0.0, 1.0],
        [-0.5, -1.0, -2.0],
        [1.0; 3],
    ];
    for exponent in -20..=2 {
        let scale = 2.0_f64.powi(exponent);
        for v in vectors {
            let p = v.map(|x| x * scale);
            let reference = distance(p);
            let result = super::closest(p)?;
            let slack = 256.0 * f64::EPSILON * magnitude(p);
            assert!(
                result.lower <= reference + slack && result.upper + slack >= reference,
                "global distance at {p:?}: {:?} versus {reference}",
                [result.lower, result.upper]
            );
            assert!(distance(result.p) <= result.error + slack);
            let length = magnitude(p);
            let ray = Ray {
                o: p,
                d: p.map(|x| -x / length),
            };
            let result = super::query(ray)?;
            match (first_hit(ray), result.sample.hit) {
                (Some(t), true) => assert!(
                    (result.sample.t - t).abs() <= result.error + slack,
                    "ray {p:?}: got {}, reference {t}, error {}, uncertain {}",
                    result.sample.t,
                    result.error,
                    result.uncertain
                ),
                (None, false) => {}
                _ => assert!(
                    result.uncertain,
                    "resolved ray classification disagrees at {p:?}"
                ),
            }
        }
    }
    Ok(())
}
