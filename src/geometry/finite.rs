//! Double-sided finite square. Nine convex ray/rectangle regions; no marching.
use super::{
    Error,
    point::{Closest, DistanceBounds, Multiplicity, Resolution, Witness},
};
use crate::math::{
    Ray, Sample,
    interval::{Interval as I, distance},
    types::GeometrySample,
};

pub const fn frame(p: [f64; 3], axis: f64) -> [f64; 3] {
    match axis {
        1.0 => [p[1], p[2], p[0]],
        2.0 => [p[2], p[0], p[1]],
        _ => p,
    }
}
pub const fn unframe(p: [f64; 3], axis: f64) -> [f64; 3] {
    match axis {
        1.0 => [p[2], p[0], p[1]],
        2.0 => [p[1], p[2], p[0]],
        _ => p,
    }
}

/// An unbounded upper end is state, never a numeric sentinel.
#[derive(Clone, Copy, Default)]
pub struct Range {
    pub near: f64,
    pub far: Option<f64>,
    pub near_upper: f64,
    pub far_lower: Option<f64>,
}
impl Range {
    pub fn clip(mut self, o: f64, d: f64, bound: f64, lower: bool) -> Result<Option<Self>, Error> {
        if d == 0.0 {
            return Ok((if lower { o >= bound } else { o <= bound }).then_some(self));
        }
        let t = I::point(bound)?
            .sub(I::point(o)?)?
            .divide(d.abs())?
            .scale(d.signum())?;
        if lower == (d > 0.0) {
            self.near = self.near.max(t.lo());
            self.near_upper = self.near_upper.max(t.hi());
        } else {
            self.far = Some(self.far.map_or_else(|| t.hi(), |v| v.min(t.hi())));
            self.far_lower = Some(self.far_lower.map_or_else(|| t.lo(), |v| v.min(t.lo())));
        }
        Ok(self.far.is_none_or(|v| v >= self.near).then_some(self))
    }
    pub fn axis(self, o: f64, d: f64, lo: f64, hi: f64) -> Result<Option<Self>, Error> {
        let Some(range) = self.clip(o, d, lo, true)? else {
            return Ok(None);
        };
        range.clip(o, d, hi, false)
    }
    pub fn clamp(self, t: f64) -> f64 {
        self.far
            .map_or_else(|| t.max(self.near), |far| t.clamp(self.near, far))
    }
}
pub fn box_range(ray: Ray, lo: [f64; 3], hi: [f64; 3]) -> Result<Option<Range>, Error> {
    let mut range = Range::default();
    for axis in 0..3 {
        let Some(next) = range.axis(ray.o[axis], ray.d[axis], lo[axis], hi[axis])? else {
            return Ok(None);
        };
        range = next;
    }
    Ok(Some(range))
}
pub fn ray_point(ray: Ray, t: f64) -> [f64; 3] {
    [0, 1, 2].map(|i| ray.d[i].mul_add(t, ray.o[i]))
}
pub fn rounding(ray: Ray, t: f64) -> f64 {
    128.0
        * f64::EPSILON
        * t.abs()
            .max(ray.o[0].abs())
            .max(ray.o[1].abs())
            .max(ray.o[2].abs())
            .max(1.0)
}
pub const fn sample(p: [f64; 3], t: f64, hit: bool) -> Sample {
    Sample {
        hit,
        t,
        gap: 0.0,
        p,
        n: [0.0, 0.0, 1.0],
        uv: [0.5_f64.mul_add(p[0], 0.5), 0.5_f64.mul_add(p[1], 0.5)],
    }
}
pub fn query(ray: Ray, axis: f64, leaf: bool, need_gap: bool) -> Result<GeometrySample, Error> {
    let ray = Ray {
        o: frame(ray.o, axis),
        d: frame(ray.d, axis),
    };
    let mut result = if leaf {
        leaf_query(ray, need_gap)?
    } else {
        flat_query(ray, need_gap)?
    };
    result.sample.p = unframe(result.sample.p, axis);
    result.sample.n = unframe(result.sample.n, axis);
    Ok(result)
}
fn leaf_query(ray: Ray, need_gap: bool) -> Result<GeometrySample, Error> {
    let field = crate::heightfield::LEAF;
    let query = crate::heightfield::ray::query(field, ray, 64)?;
    if query.visits > 64 {
        return Err(Error::Unresolved);
    }
    if !query.complete && !need_gap {
        return Err(Error::Unresolved);
    }
    if let Some(hit) = query.contact {
        let mut sample = sample(hit.p, hit.time.middle(), true);
        sample.n = hit.normal;
        return Ok(GeometrySample {
            sample,
            gap_lower: 0.0,
            error: hit.error,
            uncertain: !query.complete || query.uncertain,
            gap_computed: true,
        });
    }
    if !need_gap {
        return Ok(GeometrySample {
            sample: sample([0.0; 3], 0.0, false),
            gap_lower: 0.0,
            error: 0.0,
            uncertain: false,
            gap_computed: false,
        });
    }
    // A cutout is only a subset of the displaced carrier: no carrier upper-bound shortcut.
    let xy = field.anchor();
    let value = field.sample(xy)?;
    let point = [xy[0], xy[1], value.height];
    let delta = [0, 1, 2].map(|i| point[i] - ray.o[i]);
    let t = (dot(delta, ray.d) / dot(ray.d, ray.d)).max(0.0);
    let error = rounding(ray, t) + field.roundoff();
    let upper = distance(ray_point(ray, t), point)?
        .add(I::point(error)?)?
        .hi();
    let bounds = field
        .bounds(crate::heightfield::Node::default())?
        .ok_or(Error::Numeric)?;
    let envelope = bounds.lo[2].abs().max(bounds.hi[2].abs());
    let lower = I::point(gap(ray)?.gap_lower)?
        .sub(I::point(envelope)?)?
        .lo()
        .max(0.0);
    let mut sample = sample(point, t, false);
    sample.gap = upper;
    sample.n = value.normal();
    Ok(GeometrySample {
        sample,
        gap_lower: lower,
        error,
        uncertain: !query.complete,
        gap_computed: true,
    })
}
fn flat_query(ray: Ray, need_gap: bool) -> Result<GeometrySample, Error> {
    let mut hit = None;
    let mut uncertain = false;
    if ray.d[2] != 0.0 {
        if ray.o[2] == 0.0 || ray.o[2].signum() != ray.d[2].signum() {
            let time = I::ZERO
                .sub(I::point(ray.o[2])?)?
                .divide(ray.d[2].abs())?
                .scale(ray.d[2].signum())?;
            let t = (-ray.o[2] / ray.d[2]).max(0.0);
            let p = ray_point(ray, t);
            let mut possible = true;
            let mut certain = true;
            for i in 0..2 {
                let value = time.scale(ray.d[i])?.add(I::point(ray.o[i])?)?;
                possible &= value.hi() >= -1.0 && value.lo() <= 1.0;
                certain &= value.lo() >= -1.0 && value.hi() <= 1.0;
            }
            if possible {
                uncertain = !certain;
                if p[0].abs() <= 1.0 && p[1].abs() <= 1.0 {
                    hit = Some(sample([p[0], p[1], 0.0], t, true));
                } else {
                    uncertain = true;
                }
            }
        }
    } else if ray.o[2] == 0.0
        && let Some(range) = box_range(ray, [-1.0, -1.0, 0.0], [1.0, 1.0, 0.0])?
    {
        let p = ray_point(ray, range.near);
        uncertain = range.far_lower.is_some_and(|v| v < range.near_upper);
        hit = Some(sample(
            [p[0].clamp(-1.0, 1.0), p[1].clamp(-1.0, 1.0), 0.0],
            range.near,
            true,
        ));
    }
    if uncertain && !need_gap {
        return Err(Error::Unresolved);
    }
    let mut result = if let Some(sample) = hit {
        GeometrySample {
            sample,
            gap_lower: 0.0,
            error: rounding(ray, sample.t),
            uncertain: false,
            gap_computed: true,
        }
    } else if need_gap {
        gap(ray)?
    } else {
        GeometrySample {
            sample: sample([0.0; 3], 0.0, false),
            gap_lower: 0.0,
            error: 0.0,
            uncertain: false,
            gap_computed: false,
        }
    };
    result.uncertain |= uncertain;
    Ok(result)
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}
fn gap(ray: Ray) -> Result<GeometrySample, Error> {
    let mut best: Option<GeometrySample> = None;
    let mut lower: Option<f64> = None;
    // Inside / below / above on each axis partition every possible nearest point.
    for x in [-1.0, 0.0, 1.0] {
        for y in [-1.0, 0.0, 1.0] {
            let regions = [x, y];
            let mut range = Some(Range::default());
            for (axis, region) in regions.into_iter().enumerate() {
                if let Some(current) = range {
                    range = if region == 0.0 {
                        current.axis(ray.o[axis], ray.d[axis], -1.0, 1.0)?
                    } else {
                        current.clip(ray.o[axis], ray.d[axis], region, region > 0.0)?
                    };
                }
            }
            let Some(range) = range else { continue };
            let offset = [
                if x == 0.0 { 0.0 } else { ray.o[0] - x },
                if y == 0.0 { 0.0 } else { ray.o[1] - y },
                ray.o[2],
            ];
            let tangent = [
                if x == 0.0 { 0.0 } else { ray.d[0] },
                if y == 0.0 { 0.0 } else { ray.d[1] },
                ray.d[2],
            ];
            let scale = tangent[0].abs().max(tangent[1].abs()).max(tangent[2].abs());
            let t = if scale == 0.0 {
                range.near
            } else {
                let unit = tangent.map(|c| c / scale);
                range.clamp(-dot(offset, unit) / (scale * dot(unit, unit)))
            };
            if !t.is_finite() {
                return Err(Error::Numeric);
            }
            let position = ray_point(ray, t);
            let witness = [
                position[0].clamp(-1.0, 1.0),
                position[1].clamp(-1.0, 1.0),
                0.0,
            ];
            let metric = distance(position, witness)?;
            let error = rounding(ray, t);
            let bounds = metric.add(I::new(-error, error)?)?;
            let bound = bounds.lo().max(0.0);
            lower = Some(lower.map_or(bound, |value| value.min(bound)));
            let mut sample = sample(witness, t, false);
            sample.gap = bounds.hi();
            let candidate = GeometrySample {
                sample,
                gap_lower: 0.0,
                error,
                uncertain: false,
                gap_computed: true,
            };
            if best.is_none_or(|v| candidate.sample.gap < v.sample.gap) {
                best = Some(candidate);
            }
        }
    }
    let mut best = best.ok_or(Error::Numeric)?;
    best.gap_lower = lower.ok_or(Error::Numeric)?;
    Ok(best)
}
pub fn closest(
    p: [f64; 3],
    axis: f64,
    leaf: bool,
    request: super::point::Request,
) -> Result<Closest, Error> {
    let p = frame(p, axis);
    if leaf {
        let mut q = crate::heightfield::point::query(crate::heightfield::LEAF, p, request)?;
        if let Some(ref mut witness) = q.witness {
            witness.point = unframe(witness.point, axis);
        }
        return Ok(q);
    }
    let witness = [p[0].clamp(-1.0, 1.0), p[1].clamp(-1.0, 1.0), 0.0];
    let bounds = distance(p, witness)?;
    Ok(Closest {
        distance: DistanceBounds {
            lower: bounds.lo(),
            upper: bounds.hi(),
        },
        witness: Some(Witness {
            point: unframe(witness, axis),
            error: 0.0,
        }),
        resolution: Resolution::Unresolved,
        multiplicity: Multiplicity::Unique,
        evaluations: 1,
    })
}
