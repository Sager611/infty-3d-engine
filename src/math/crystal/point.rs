use super::{
    ArithmeticError, I, MIN_SCALE, Patch, Projection, V, better, cycle, distance, norm, normal,
    power_floor, quotient, turn, uv,
};
struct Radial {
    r: I,
    direction: [I; 2],
    approximate: [f64; 2],
}
impl Radial {
    fn new(p: V) -> Result<Self, ArithmeticError> {
        let x = I::point(p[0])?;
        let y = I::point(p[1])?;
        let magnitude = p[0].abs().max(p[1].abs());
        if magnitude == 0.0 {
            return Ok(Self {
                r: I::ZERO,
                direction: [I::ONE, I::ZERO],
                approximate: [1.0, 0.0],
            });
        }
        let x = x.divide(magnitude)?;
        let y = y.divide(magnitude)?;
        let length = norm([x, y, I::ZERO])?;
        let direction = [quotient(x, length)?, quotient(y, length)?];
        Ok(Self {
            r: length.scale(magnitude)?,
            direction,
            approximate: [direction[0].middle(), direction[1].middle()],
        })
    }
}
fn clamp_unit(value: I) -> Result<I, ArithmeticError> {
    I::new(value.lo().clamp(0.0, 1.0), value.hi().clamp(0.0, 1.0))
}
fn segment(p: V, radial: &Radial, patch: Patch) -> Result<Projection, ArithmeticError> {
    let z = I::point(p[2])?;
    let numerator = radial
        .r
        .sub(I::point(patch.r)?)?
        .scale(patch.dr)?
        .add(z.sub(I::point(patch.z)?)?.scale(patch.dz)?)?;
    let length = patch.dr.mul_add(patch.dr, patch.dz * patch.dz);
    let t = clamp_unit(numerator.divide(length)?)?;
    let qr = t.scale(patch.dr)?.add(I::point(patch.r)?)?;
    let qz = t.scale(patch.dz)?.add(I::point(patch.z)?)?;
    let qx = qr.mul(radial.direction[0])?;
    let qy = qr.mul(radial.direction[1])?;
    let q = [qx.middle(), qy.middle(), qz.middle()];
    let error = norm([
        qx.sub(I::point(q[0])?)?.abs(),
        qy.sub(I::point(q[1])?)?.abs(),
        qz.sub(I::point(q[2])?)?.abs(),
    ])?
    .hi();
    let lower = norm([radial.r.sub(qr)?, z.sub(qz)?, I::ZERO])?.lo();
    let upper = I::point(distance(p, q)?.hi())?.add(I::point(error)?)?.hi();
    let n = normal(radial.approximate, patch.slope());
    Ok(Projection {
        p: q,
        n,
        uv: uv(q, n),
        lower,
        upper,
        error,
    })
}
fn cone_scale(p: V, radial: &Radial) -> Result<[I; 2], ArithmeticError> {
    let r = radial.r.middle();
    let z = I::point(p[2])?;
    let rpoint = I::point(r)?;
    let (qr, qz) = if p[2] < 0.5 * r {
        let q = rpoint.scale(4.0)?.add(z.scale(2.0)?)?.divide(5.0)?;
        let q = I::new(q.lo().max(0.0), q.hi().max(0.0))?;
        (q, q.scale(0.5)?)
    } else if p[2] > 2.0 * r {
        let q = rpoint.add(z.scale(2.0)?)?.divide(5.0)?;
        (q, q.scale(2.0)?)
    } else {
        (rpoint, z)
    };
    // Convex projection is nonexpansive; radial input error expands each coordinate.
    let delta = radial.r.sub(rpoint)?.abs().hi();
    let padding = I::new(-delta, delta)?;
    let qr = qr.add(padding)?;
    let qz = qz.add(padding)?;
    Ok([
        qr.scale(5.0)?.sub(qz)?.divide(4.5)?,
        qr.scale(4.0)?.add(qz)?.divide(9.0)?,
    ])
}
fn family(
    p: V,
    radial: &Radial,
    scale: I,
    rising: bool,
) -> Result<(Projection, bool), ArithmeticError> {
    let low = power_floor(scale.lo().clamp(MIN_SCALE, 0.5));
    let high = power_floor(scale.hi().clamp(MIN_SCALE, 0.5));
    let patch = |s| {
        if rising {
            Patch::rising(s)
        } else {
            Patch::falling(s)
        }
    };
    let a = better(
        segment(p, radial, patch(low))?,
        segment(p, radial, patch((2.0 * low).min(0.5)))?,
    );
    let b = better(
        segment(p, radial, patch(high))?,
        segment(p, radial, patch((2.0 * high).min(0.5)))?,
    );
    Ok((better(a, b), high <= 2.0 * low))
}
fn component(p: V) -> Result<Projection, ArithmeticError> {
    I::point(p[2])?;
    let radial = Radial::new(p)?;
    let scales = cone_scale(p, &radial)?;
    let (a, cover_a) = family(p, &radial, scales[0], true)?;
    let (b, cover_b) = family(p, &radial, scales[1], false)?;
    let mut result = better(better(a, b), segment(p, &radial, Patch::core())?);
    if !(cover_a && cover_b) {
        result.lower = 0.0;
    }
    Ok(result)
}
pub(super) fn patch(p: V, patch: Patch) -> Result<Projection, ArithmeticError> {
    segment(p, &Radial::new(p)?, patch)
}
pub fn closest(p: V) -> Result<Projection, ArithmeticError> {
    Ok(better(
        better(component(p)?, turn(component(cycle(p))?)),
        turn(turn(component(cycle(cycle(p)))?)),
    ))
}
