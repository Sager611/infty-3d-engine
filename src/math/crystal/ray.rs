use super::{
    ArithmeticError, GeometrySample, I, MIN_SCALE, Patch, RADIUS, Ray, Sample, V, closest, cycle,
    norm, power_floor, quotient, uncycle,
};
#[derive(Clone, Copy)]
struct Root {
    t: f64,
    error: f64,
    uncertain: bool,
}
impl Root {
    const NONE: Self = Self {
        t: f64::INFINITY,
        error: 0.0,
        uncertain: false,
    };
}
#[derive(Clone, Copy)]
struct Span {
    lo: f64,
    hi: f64,
    error: f64,
    uncertain: bool,
}
impl Span {
    const ALL: Self = Self {
        lo: 0.0,
        hi: f64::INFINITY,
        error: 0.0,
        uncertain: false,
    };
    const EMPTY: Self = Self {
        lo: 1.0,
        hi: 0.0,
        error: 0.0,
        uncertain: false,
    };
    fn intersect(self, b: Self) -> Self {
        Self {
            lo: self.lo.max(b.lo),
            hi: self.hi.min(b.hi),
            error: self.error + b.error,
            uncertain: self.uncertain || b.uncertain,
        }
    }
}
#[derive(Clone, Copy)]
struct Shot {
    root: Root,
    patch: Patch,
    axis: u32,
}
impl Shot {
    const NONE: Self = Self {
        root: Root::NONE,
        patch: Patch::core(),
        axis: 0,
    };
}
fn choose(a: Shot, b: Shot) -> Shot {
    let mut result = if b.root.t < a.root.t { b } else { a };
    result.root.uncertain = a.root.uncertain || b.root.uncertain;
    result
}
fn divide(n: I, d: I) -> Result<I, ArithmeticError> {
    if d.hi() < 0.0 {
        quotient(n.scale(-1.0)?, d.scale(-1.0)?)
    } else {
        quotient(n, d)
    }
}
fn contains_zero(v: I) -> bool {
    v.lo() <= 0.0 && v.hi() >= 0.0
}
fn root(value: f64, interval: Option<I>, uncertain: bool) -> Root {
    if !value.is_finite() {
        return Root {
            uncertain,
            ..Root::NONE
        };
    }
    interval.map_or_else(
        || Root {
            t: value,
            error: 64.0 * f64::EPSILON * (1.0 + value.abs()),
            uncertain: true,
        },
        |range| {
            let error = (range.lo() - value).abs().max((range.hi() - value).abs());
            Root {
                t: value,
                error,
                uncertain: uncertain || error > 1e-8 * (1.0 + value.abs()),
            }
        },
    )
}
fn roots(a: I, b: I, c: I) -> Result<[Root; 2], ArithmeticError> {
    let av = a.middle();
    let bv = b.middle();
    let cv = c.middle();
    if av == 0.0 {
        if bv == 0.0 {
            return Ok([Root {
                uncertain: contains_zero(a),
                ..Root::NONE
            }; 2]);
        }
        let t = -cv / bv;
        let range = divide(c.scale(-1.0)?, b).ok();
        return Ok([root(t, range, true), Root::NONE]);
    }
    let disc = b.square()?.sub(a.mul(c)?.scale(4.0)?)?;
    if disc.hi() < 0.0 {
        return Ok([Root::NONE; 2]);
    }
    let square = I::new(disc.lo().max(0.0), disc.hi().max(0.0))?.sqrt()?;
    let sign = if bv >= 0.0 { 1.0 } else { -1.0 };
    let qi = b.add(square.scale(sign)?)?.scale(-0.5)?;
    let q = -0.5 * (bv + sign * disc.middle().max(0.0).sqrt());
    let uncertain = contains_zero(a) || disc.lo() <= 0.0;
    let pair = if q == 0.0 {
        let t = -bv / (2.0 * av);
        let range = divide(b.scale(-1.0)?, a.scale(2.0)?).ok();
        [root(t, range, uncertain); 2]
    } else {
        [
            root(q / av, divide(qi, a).ok(), uncertain),
            root(cv / q, divide(c, qi).ok(), uncertain),
        ]
    };
    Ok(if pair[0].t <= pair[1].t {
        pair
    } else {
        [pair[1], pair[0]]
    })
}
fn dot(a: V, b: V) -> f64 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}
fn fused_origin(ray: Ray, t: f64) -> Result<[I; 3], ArithmeticError> {
    let p = at(ray, t);
    Ok([
        I::new(p[0].next_down(), p[0].next_up())?,
        I::new(p[1].next_down(), p[1].next_up())?,
        I::new(p[2].next_down(), p[2].next_up())?,
    ])
}
fn shifted_root(mut value: Root, shift: f64) -> Result<Root, ArithmeticError> {
    if !value.t.is_finite() {
        return Ok(value);
    }
    let sum = I::point(value.t)?.add(I::point(shift)?)?;
    value.t += shift;
    let error = (sum.lo() - value.t).abs().max((sum.hi() - value.t).abs());
    value.error = I::point(value.error)?.add(I::point(error)?)?.hi();
    Ok(value)
}
fn shifted_roots(a: I, b: I, c: I, shift: f64) -> Result<[Root; 2], ArithmeticError> {
    let rr = roots(a, b, c)?;
    Ok([shifted_root(rr[0], shift)?, shifted_root(rr[1], shift)?])
}
fn coefficients(ray: Ray, slope: f64, intercept: f64) -> Result<([I; 3], f64), ArithmeticError> {
    let shift = -dot(ray.o, ray.d) / dot(ray.d, ray.d);
    let [ox, oy, oz] = fused_origin(ray, shift)?;
    let oz = oz.sub(I::point(intercept)?)?;
    let dx = I::point(ray.d[0])?;
    let dy = I::point(ray.d[1])?;
    let dz = I::point(ray.d[2])?;
    let squared = slope * slope;
    Ok((
        [
            dz.square()?
                .sub(dx.square()?.add(dy.square()?)?.scale(squared)?)?,
            oz.mul(dz)?
                .sub(ox.mul(dx)?.add(oy.mul(dy)?)?.scale(squared)?)?
                .scale(2.0)?,
            oz.square()?
                .sub(ox.square()?.add(oy.square()?)?.scale(squared)?)?,
        ],
        shift,
    ))
}
fn cone(ray: Ray, slope: f64, strict: bool) -> Result<Span, ArithmeticError> {
    let ([a, b, c], shift) = coefficients(ray, slope, 0.0)?;
    let av = a.middle();
    let bv = b.middle();
    let cv = c.middle();
    let [left, right] = shifted_roots(a, b, c, shift)?;
    if strict && contains_zero(a) && contains_zero(b) && contains_zero(c) {
        return Ok(Span {
            uncertain: true,
            ..Span::EMPTY
        });
    }
    let mut result = Span::ALL;
    if av > 0.0 {
        let crossing = if ray.d[2] > 0.0 { right } else { left };
        if !crossing.t.is_finite() {
            return Ok(Span {
                uncertain: crossing.uncertain,
                ..Span::EMPTY
            });
        }
        if ray.d[2] > 0.0 {
            result.lo = crossing.t;
        } else {
            result.hi = crossing.t;
        }
        result.error = crossing.error;
        result.uncertain = crossing.uncertain;
    } else if av < 0.0 {
        if !left.t.is_finite() {
            return Ok(Span {
                uncertain: left.uncertain,
                ..Span::EMPTY
            });
        }
        result.lo = left.t;
        result.hi = right.t;
        result.error = left.error.max(right.error);
        result.uncertain = left.uncertain || right.uncertain;
    } else if bv > 0.0 {
        result.lo = left.t;
        result.error = left.error;
        result.uncertain = true;
    } else if bv < 0.0 {
        result.hi = left.t;
        result.error = left.error;
        result.uncertain = true;
    } else if cv < 0.0 {
        return Ok(Span {
            uncertain: contains_zero(a),
            ..Span::EMPTY
        });
    } else {
        result.uncertain = true;
    }
    if ray.d[2] > 0.0 {
        result.lo = result.lo.max(-ray.o[2] / ray.d[2]);
    } else if ray.d[2] < 0.0 {
        result.hi = result.hi.min(-ray.o[2] / ray.d[2]);
    } else if ray.o[2] < 0.0 {
        return Ok(Span::EMPTY);
    }
    result.lo = result.lo.max(0.0);
    Ok(result)
}
fn cylinder(ray: Ray, radius: f64) -> Result<Span, ArithmeticError> {
    let dx = I::point(ray.d[0])?;
    let dy = I::point(ray.d[1])?;
    if ray.d[0] == 0.0 && ray.d[1] == 0.0 {
        let c = I::point(ray.o[0])?
            .square()?
            .add(I::point(ray.o[1])?.square()?)?
            .sub(I::point(radius * radius)?)?;
        return Ok(if c.lo() > 0.0 {
            Span::EMPTY
        } else {
            Span {
                uncertain: c.hi() > 0.0,
                ..Span::ALL
            }
        });
    }
    let denominator = ray.d[0].mul_add(ray.d[0], ray.d[1] * ray.d[1]);
    let shift = -ray.o[0].mul_add(ray.d[0], ray.o[1] * ray.d[1]) / denominator;
    let [ox, oy, _] = fused_origin(ray, shift)?;
    let a = dx.square()?.add(dy.square()?)?;
    let b = ox.mul(dx)?.add(oy.mul(dy)?)?.scale(2.0)?;
    let c = ox
        .square()?
        .add(oy.square()?)?
        .sub(I::point(radius * radius)?)?;
    let [left, right] = shifted_roots(a, b, c, shift)?;
    if !left.t.is_finite() {
        return Ok(Span {
            uncertain: left.uncertain,
            ..Span::EMPTY
        });
    }
    Ok(Span {
        lo: left.t.max(0.0),
        hi: right.t,
        error: left.error.max(right.error),
        uncertain: left.uncertain || right.uncertain,
    })
}
const fn at(ray: Ray, t: f64) -> V {
    [
        ray.d[0].mul_add(t, ray.o[0]),
        ray.d[1].mul_add(t, ray.o[1]),
        ray.d[2].mul_add(t, ray.o[2]),
    ]
}
fn accept(ray: Ray, patch: Patch, window: Span, mut candidate: Root) -> Shot {
    let t = candidate.t;
    if !t.is_finite() {
        return Shot::NONE;
    }
    let allowance = (256.0 * f64::EPSILON).mul_add(
        1.0 + ray.o[0].abs() + ray.o[1].abs() + ray.o[2].abs() + t.abs(),
        window.error + candidate.error,
    );
    if t < 0.0 {
        let mut miss = Shot::NONE;
        miss.root.uncertain = candidate.uncertain || t + candidate.error >= 0.0;
        return miss;
    }
    if t < window.lo - allowance || t > window.hi + allowance {
        return Shot::NONE;
    }
    let p = at(ray, t.max(0.0));
    let r = p[0].hypot(p[1]);
    let high = patch.r + patch.dr;
    let residual = p[2] - patch.slope().mul_add(r, patch.intercept());
    if r < patch.r - allowance || r > high + allowance || residual.abs() > 16.0 * allowance {
        return Shot::NONE;
    }
    candidate.t = t.max(0.0);
    candidate.uncertain |= window.uncertain
        || t < 0.0
        || r < patch.r
        || r > high
        || allowance > 1e-8 * (1.0 + t.abs());
    candidate.error += allowance;
    Shot {
        root: candidate,
        patch,
        axis: 0,
    }
}
fn coincident(ray: Ray, patch: Patch, window: Span) -> Result<Shot, ArithmeticError> {
    let outside = cylinder(ray, patch.r + patch.dr)?.intersect(window);
    let inside = cylinder(ray, patch.r)?;
    let first = outside.lo;
    let t = if first >= inside.lo && first < inside.hi {
        inside.hi
    } else {
        first
    };
    let candidate = Root {
        t,
        error: outside.error + inside.error,
        uncertain: true,
    };
    Ok(accept(ray, patch, outside, candidate))
}
fn intersect(ray: Ray, patch: Patch, window: Span) -> Result<Shot, ArithmeticError> {
    if window.lo > window.hi {
        return Ok(Shot::NONE);
    }
    let ([a, b, c], shift) = coefficients(ray, patch.slope(), patch.intercept())?;
    if contains_zero(a) && contains_zero(b) && contains_zero(c) {
        return coincident(ray, patch, window);
    }
    let rr = shifted_roots(a, b, c, shift)?;
    let mut result = choose(
        accept(ray, patch, window, rr[0]),
        accept(ray, patch, window, rr[1]),
    );
    result.root.uncertain |= rr[0].uncertain || rr[1].uncertain;
    Ok(result)
}
fn band(ray: Ray, scale: f64, above: bool, window: Span) -> Result<Shot, ArithmeticError> {
    let (fall, rise) = if above {
        (scale, 2.0 * scale)
    } else {
        (scale, scale)
    };
    let a = if (MIN_SCALE..=0.5).contains(&fall) {
        intersect(ray, Patch::falling(fall), window)?
    } else {
        Shot::NONE
    };
    let b = if (MIN_SCALE..=0.5).contains(&rise) {
        intersect(ray, Patch::rising(rise), window)?
    } else {
        Shot::NONE
    };
    Ok(choose(a, b))
}
fn window(ray: Ray, span: Span) -> Result<Shot, ArithmeticError> {
    if span.lo > span.hi || !span.lo.is_finite() {
        return Ok(Shot::NONE);
    }
    let p = at(ray, span.lo);
    let r = p[0].hypot(p[1]);
    let s = power_floor(r.max(MIN_SCALE)).min(0.5);
    let height = if r < MIN_SCALE {
        0.5 * r
    } else if r <= 1.5 * s {
        5.0_f64.mul_add(r, -4.5 * s)
    } else {
        (-4.0_f64).mul_add(r, 9.0 * s)
    };
    let above = p[2] >= height;
    let scale = power_floor(r.max(MIN_SCALE) / if above { 1.5 } else { 1.0 });
    Ok(choose(
        choose(
            band(ray, 0.5 * scale, above, span)?,
            band(ray, scale, above, span)?,
        ),
        band(ray, 2.0 * scale, above, span)?,
    ))
}
fn component(ray: Ray) -> Result<Shot, ArithmeticError> {
    if (ray.o[2] < 0.0 && ray.d[2] <= 0.0) || (ray.o[2] > 1.5 && ray.d[2] >= 0.0) {
        return Ok(Shot::NONE);
    }
    let outer = cone(ray, 0.5, false)?.intersect(cylinder(ray, 1.0)?);
    let inner = cone(ray, 2.0, true)?.intersect(outer);
    let core = intersect(ray, Patch::core(), Span::ALL)?;
    let mut result = if outer.lo <= outer.hi {
        let span = Span {
            error: outer.error + inner.error,
            uncertain: outer.uncertain || inner.uncertain,
            ..outer
        };
        let a = Span {
            hi: inner.lo.min(outer.hi),
            ..span
        };
        let b = Span {
            lo: inner.hi.max(outer.lo),
            ..span
        };
        let body = if inner.lo > inner.hi {
            window(ray, outer)?
        } else {
            choose(window(ray, a)?, window(ray, b)?)
        };
        choose(core, body)
    } else {
        core
    };
    result.root.uncertain |= outer.uncertain || inner.uncertain;
    Ok(result)
}
fn line_lower(ray: Ray) -> Result<f64, ArithmeticError> {
    let magnitude = ray.o[0].abs().max(ray.o[1].abs()).max(ray.o[2].abs());
    if magnitude == 0.0 {
        return Ok(0.0);
    }
    let x = I::point(ray.o[0])?.divide(magnitude)?;
    let y = I::point(ray.o[1])?.divide(magnitude)?;
    let z = I::point(ray.o[2])?.divide(magnitude)?;
    let dx = I::point(ray.d[0])?;
    let dy = I::point(ray.d[1])?;
    let dz = I::point(ray.d[2])?;
    let a = dx.square()?.add(dy.square()?)?.add(dz.square()?)?;
    let dot = x.mul(dx)?.add(y.mul(dy)?)?.add(z.mul(dz)?)?;
    let c = x.square()?.add(y.square()?)?.add(z.square()?)?;
    let square = c.sub(quotient(dot.square()?, a)?)?;
    let low = I::new(square.lo().max(0.0), square.hi().max(0.0))?
        .sqrt()?
        .scale(magnitude)?
        .lo();
    Ok((low - RADIUS).next_down().max(0.0))
}
fn miss(ray: Ray, uncertain: bool) -> Result<GeometrySample, ArithmeticError> {
    let a = ray.d[0].mul_add(ray.d[0], ray.d[1].mul_add(ray.d[1], ray.d[2] * ray.d[2]));
    let dot = ray.o[0].mul_add(ray.d[0], ray.o[1].mul_add(ray.d[1], ray.o[2] * ray.d[2]));
    let t = (-dot / a).max(0.0);
    let p = at(ray, t);
    let witness = closest(p)?;
    let error = norm([
        I::point(ray.d[0])?
            .scale(t)?
            .add(I::point(ray.o[0])?)?
            .sub(I::point(p[0])?)?,
        I::point(ray.d[1])?
            .scale(t)?
            .add(I::point(ray.o[1])?)?
            .sub(I::point(p[1])?)?,
        I::point(ray.d[2])?
            .scale(t)?
            .add(I::point(ray.o[2])?)?
            .sub(I::point(p[2])?)?,
    ])?
    .hi();
    let gap = I::point(witness.upper)?.add(I::point(error)?)?.hi();
    Ok(GeometrySample {
        sample: Sample {
            hit: false,
            t,
            gap,
            p: witness.p,
            n: witness.n,
            uv: witness.uv,
        },
        gap_lower: line_lower(ray)?,
        error: witness.error,
        uncertain,
    })
}
pub fn query(ray: Ray) -> Result<GeometrySample, ArithmeticError> {
    let a = component(ray)?;
    let mut b = component(Ray {
        o: cycle(ray.o),
        d: cycle(ray.d),
    })?;
    b.axis = 1;
    let mut c = component(Ray {
        o: cycle(cycle(ray.o)),
        d: cycle(cycle(ray.d)),
    })?;
    c.axis = 2;
    let shot = choose(choose(a, b), c);
    if !shot.root.t.is_finite() {
        return miss(ray, shot.root.uncertain);
    }
    let local = match shot.axis {
        1 => Ray {
            o: cycle(ray.o),
            d: cycle(ray.d),
        },
        2 => Ray {
            o: cycle(cycle(ray.o)),
            d: cycle(cycle(ray.d)),
        },
        _ => ray,
    };
    let witness = super::point::patch(at(local, shot.root.t), shot.patch)?;
    let (p, n) = match shot.axis {
        1 => (uncycle(witness.p), uncycle(witness.n)),
        2 => (uncycle(uncycle(witness.p)), uncycle(uncycle(witness.n))),
        _ => (witness.p, witness.n),
    };
    let error = I::point(shot.root.error)?
        .add(I::point(witness.upper)?)?
        .hi();
    Ok(GeometrySample {
        sample: Sample {
            hit: true,
            t: shot.root.t,
            gap: 0.0,
            p,
            n,
            uv: witness.uv,
        },
        gap_lower: 0.0,
        error,
        uncertain: shot.root.uncertain,
    })
}
