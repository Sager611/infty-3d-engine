use super::{
    polynomial::{cubic, quartic_traced},
    types::{Ray, Sample, V},
    vector::dot,
};

use crate::diagnostic::{Recorder, Tag};

pub use super::types::GeometrySample as GoursatSample;

/// Errors returned when a Goursat query cannot be evaluated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoursatError {
    InvalidRay,
    InvalidBeta,
    Polynomial,
    NoWitness,
}

impl core::fmt::Display for GoursatError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let message = match self {
            Self::InvalidRay => "invalid ray",
            Self::InvalidBeta => "invalid beta",
            Self::Polynomial => "polynomial evaluation failed",
            Self::NoWitness => "surface witness unavailable",
        };
        f.write_str(message)
    }
}

const BETA_MIN: f64 = 1.05_f32 as f64;
const BETA_MAX: f64 = 1.95_f32 as f64;

fn value(p: V, beta: f64) -> f64 {
    let mut out = -beta;
    for x in p {
        let q = x.mul_add(x, -1.0);
        out = q.mul_add(q, out);
    }
    out
}

fn gradient(p: V) -> V {
    [
        4.0 * p[0] * p[0].mul_add(p[0], -1.0),
        4.0 * p[1] * p[1].mul_add(p[1], -1.0),
        4.0 * p[2] * p[2].mul_add(p[2], -1.0),
    ]
}

fn normal_uv(p: V) -> (V, [f64; 2]) {
    let g = gradient(p);
    let n = {
        let l = dot(g, g).sqrt();
        if l > f64::EPSILON {
            [g[0] / l, g[1] / l, g[2] / l]
        } else {
            [0.0, 1.0, 0.0]
        }
    };
    let ax = n[0].abs();
    let ay = n[1].abs();
    let az = n[2].abs();
    let uv = if ax >= ay && ax >= az {
        [p[1], p[2]]
    } else if ay >= az {
        [p[0], p[2]]
    } else {
        [p[0], p[1]]
    };
    (n, uv)
}

fn coefficients(p: V, d: V, beta: f64) -> [f64; 5] {
    let mut c = [0.0; 5];
    for i in 0..3 {
        let x = p[i];
        let y = d[i];
        let x2 = x * x;
        let y2 = y * y;
        c[0] = y2.mul_add(y2, c[0]);
        c[1] = (4.0 * x * y2).mul_add(y, c[1]);
        c[2] += 2.0f64.mul_add(-y2, 6.0 * x2 * y2);
        c[3] += (4.0 * x).mul_add(-y, 4.0 * x * x2 * y);
        c[4] = (x2 - 1.0).mul_add(x2 - 1.0, c[4]);
    }
    c[4] -= beta;
    c
}

const fn point(ray: Ray, t: f64) -> V {
    [
        t.mul_add(ray.d[0], ray.o[0]),
        t.mul_add(ray.d[1], ray.o[1]),
        t.mul_add(ray.d[2], ray.o[2]),
    ]
}

fn value_error(p: V, beta: f64) -> f64 {
    let mut scale = beta.abs();
    for x in p {
        let q = x.mul_add(x, 1.0);
        scale = q.mul_add(q, scale);
    }
    16.0 * f64::EPSILON / 16.0f64.mul_add(-f64::EPSILON, 1.0) * scale
}

struct Witness {
    p: V,
    upper: f64,
    error: f64,
}

fn distance_to_ray(p: V, ray: Ray) -> f64 {
    let t = dot([p[0] - ray.o[0], p[1] - ray.o[1], p[2] - ray.o[2]], ray.d).max(0.0);
    let q = point(ray, t);
    let d = [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
    dot(d, d).sqrt()
}

fn bisect_surface(mut a: V, mut b: V, beta: f64) -> Option<(V, f64)> {
    let fa = value(a, beta);
    let fb = value(b, beta);
    if !fa.is_finite()
        || !fb.is_finite()
        || fa.abs() <= value_error(a, beta)
        || fb.abs() <= value_error(b, beta)
        || fa.is_sign_negative() == fb.is_sign_negative()
    {
        return None;
    }
    for _ in 0..56 {
        let p = std::array::from_fn(|i| 0.5 * (a[i] + b[i]));
        let f = value(p, beta);
        if !f.is_finite() {
            return None;
        }
        // Keep the proven opposite-sign endpoints when evaluation is unresolved.
        if f.abs() <= value_error(p, beta) {
            break;
        }
        if f.is_sign_negative() == fa.is_sign_negative() {
            a = p;
        } else {
            b = p;
        }
    }
    let p = std::array::from_fn(|i| 0.5 * (a[i] + b[i]));
    let delta = std::array::from_fn(|i| b[i] - a[i]);
    let error = (8.0 * f64::EPSILON).mul_add(
        1.0 + dot(a, a).sqrt() + dot(b, b).sqrt(),
        0.5 * dot(delta, delta).sqrt(),
    );
    Some((p, error))
}

fn witness(ray: Ray, p0: V, d: V, t0: f64, beta: f64) -> Option<Witness> {
    let coeff = coefficients(p0, d, beta);
    let critical = cubic([4.0 * coeff[0], 3.0 * coeff[1], 2.0 * coeff[2], coeff[3]]).ok()?;
    let mut best: Option<Witness> = None;
    let mut candidates = [0.0; 4];
    let mut count = 1;
    candidates[0] = 0.0;
    for &s in &critical.values[..critical.len] {
        let t = s + t0;
        if t >= 0.0 && t.is_finite() && count < candidates.len() {
            candidates[count] = t;
            count += 1;
        }
    }
    for &t in &candidates[..count] {
        let p = point(ray, t);
        let f = value(p, beta);
        let other = if f < 0.0 {
            [0.0; 3]
        } else {
            p.map(|x| if x < 0.0 { -1.0 } else { 1.0 })
        };
        let surface = bisect_surface(p, other, beta);
        if let Some((s, e)) = surface {
            let dist = distance_to_ray(s, ray);
            let projection = dot([s[0] - ray.o[0], s[1] - ray.o[1], s[2] - ray.o[2]], d).max(0.0);
            let error = (16.0 * f64::EPSILON).mul_add(
                1.0 + dot(s, s).sqrt() + dot(ray.o, ray.o).sqrt() + projection,
                e,
            );
            let upper = (dist + error) * 4.0f64.mul_add(f64::EPSILON, 1.0);
            if upper.is_finite()
                && best
                    .as_ref()
                    .is_none_or(|old| upper < old.upper - (error + old.error))
            {
                best = Some(Witness { p: s, upper, error });
            }
        }
    }
    best
}

/// Intersect a normalized forward ray with the canonical Goursat surface.
#[cfg(test)]
pub fn goursat(ray: Ray, beta: f64, need_gap: bool) -> Result<GoursatSample, GoursatError> {
    goursat_traced(ray, beta, need_gap, &mut Recorder::off())
}

pub fn goursat_traced(
    ray: Ray,
    beta: f64,
    need_gap: bool,
    trace: &mut Recorder<'_>,
) -> Result<GoursatSample, GoursatError> {
    if !beta.is_finite() || !(BETA_MIN..=BETA_MAX).contains(&beta) {
        return Err(GoursatError::InvalidBeta);
    }
    let dd = dot(ray.d, ray.d);
    if !dd.is_finite()
        || dd <= f64::EPSILON
        || ray.o.iter().chain(ray.d.iter()).any(|x| !x.is_finite())
    {
        return Err(GoursatError::InvalidRay);
    }
    let length = dd.sqrt();
    let d = [ray.d[0] / length, ray.d[1] / length, ray.d[2] / length];
    let nray = Ray { o: ray.o, d };
    let t0 = -dot(ray.o, d);
    let p0 = point(nray, t0);
    let radius = (3.0 + (3.0 * beta).sqrt()).sqrt();
    let lo = (-t0).max(-radius);
    let hi = radius;
    let coeff = coefficients(p0, d, beta);
    trace.emit(Tag::Enclosure, [lo, hi, t0, radius, 0.0, 0.0]);
    let roots = if lo <= hi {
        Some(quartic_traced(coeff, lo, hi, trace).map_err(|_| GoursatError::Polynomial)?)
    } else {
        trace.emit(Tag::Reject, [2.0, lo, hi, t0, 0.0, 0.0]);
        None
    };
    if let Some(roots) = roots.filter(|roots| roots.len != 0) {
        let t = t0 + roots.values[0];
        let p = point(nray, t);
        let (n, uv) = normal_uv(p);
        return Ok(GoursatSample {
            sample: Sample {
                hit: true,
                t,
                gap: 0.0,
                p,
                n,
                uv,
            },
            gap_lower: 0.0,
            error: roots.error,
            uncertain: roots.uncertain,
            gap_computed: true,
        });
    }
    let forward_t = t0.max(0.0);
    let closest = point(nray, forward_t);
    let norm = dot(closest, closest).sqrt();
    let slack = 16.0 * f64::EPSILON * (1.0 + dot(ray.o, ray.o).sqrt() + t0.abs() + radius);
    let gap_lower = (norm - radius - slack).max(0.0);
    let (wp, gap_upper, error) = if need_gap {
        let witness = witness(nray, p0, d, t0, beta).ok_or(GoursatError::NoWitness)?;
        (witness.p, witness.upper, witness.error)
    } else {
        let q = if norm > f64::EPSILON {
            [
                closest[0] * radius / norm,
                closest[1] * radius / norm,
                closest[2] * radius / norm,
            ]
        } else {
            [radius, 0.0, 0.0]
        };
        (q, gap_lower, 0.0)
    };
    let (n, uv) = normal_uv(wp);
    let witness_t = dot([wp[0] - ray.o[0], wp[1] - ray.o[1], wp[2] - ray.o[2]], d).max(0.0);
    let root_uncertain = roots.is_some_and(|roots| roots.uncertain);
    Ok(GoursatSample {
        sample: Sample {
            hit: false,
            t: witness_t,
            gap: gap_upper,
            p: wp,
            n,
            uv,
        },
        gap_lower,
        error,
        uncertain: root_uncertain,
        gap_computed: need_gap,
    })
}

#[cfg(test)]
mod tests {
    use super::{GoursatError, Ray, goursat};

    #[test]
    fn coefficients_match_direct_surface_on_oblique_ray() {
        let p = [0.37, -0.82, 1.21];
        let d = [0.23, -0.71, 0.66];
        let beta = 1.37;
        let coeff = super::coefficients(p, d, beta);
        for t in [0.0, 0.17, -0.31, 0.83, 1.37] {
            let point = super::point(Ray { o: p, d }, t);
            let direct = point
                .into_iter()
                .map(|coordinate| {
                    let q = coordinate.mul_add(coordinate, -1.0);
                    q * q
                })
                .sum::<f64>()
                - beta;
            let polynomial = coeff[0]
                .mul_add(t, coeff[1])
                .mul_add(t, coeff[2])
                .mul_add(t, coeff[3])
                .mul_add(t, coeff[4]);
            let limit = 1.0e-12 * direct.abs().max(1.0);
            assert!(
                (polynomial - direct).abs() <= limit,
                "t={t}: polynomial {polynomial} != direct {direct} (limit {limit})"
            );
        }
    }

    #[test]
    fn f32_boundary_betas_are_valid() {
        let ray = Ray {
            o: [0.0, 0.0, -3.0],
            d: [0.0, 0.0, 1.0],
        };
        for beta in [1.05_f32, 1.95_f32] {
            assert!(
                goursat(ray, f64::from(beta), false).is_ok(),
                "f32 beta {beta} was rejected"
            );
        }
    }

    #[test]
    fn far_miss_witness_failure_is_explicit() {
        let ray = Ray {
            o: [1.0e150, 1.0e150, 0.0],
            d: [1.0, 0.0, 0.0],
        };
        assert!(matches!(
            goursat(ray, 1.125, true),
            Err(GoursatError::NoWitness)
        ));
    }
}
