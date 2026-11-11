//! Conservative fixed-work hierarchy; closed-form roots, never ray steps.
use super::{DEPTH, Error, Field, I, Node, Piece, STEPS, Work, minimum, pop, repeat};
use crate::{
    geometry::finite::{Range, box_range, ray_point, rounding},
    math::Ray,
};
#[derive(Clone, Copy)]
pub struct Contact {
    pub time: I,
    pub p: [f64; 3],
    pub normal: [f64; 3],
    pub error: f64,
}
pub struct Traversal {
    pub contact: Option<Contact>,
    pub visits: u32,
    pub complete: bool,
    pub uncertain: bool,
    pub pending_lower: Option<f64>,
}
#[derive(Clone, Copy)]
struct Root {
    range: I,
    exists: bool,
}
struct Roots {
    times: [Option<Root>; 2],
    coincident: bool,
    unknown: bool,
}
type Polynomial = [I; 3];

fn zero(value: I) -> bool {
    value.lo() == 0.0 && value.hi() == 0.0
}
fn contains_zero(value: I) -> bool {
    value.lo() <= 0.0 && value.hi() >= 0.0
}
fn quotient(a: I, b: I) -> Result<I, Error> {
    if contains_zero(b) {
        return Err(Error::Numeric);
    }
    Ok(a.mul(I::new(
        (1.0 / b.hi()).next_down(),
        (1.0 / b.lo()).next_up(),
    )?)?)
}
fn evaluate(p: Polynomial, x: I) -> Result<I, Error> {
    Ok(p[0].mul(x)?.add(p[1])?.mul(x)?.add(p[2])?)
}
fn scalar_roots(p: Polynomial) -> [Option<f64>; 2] {
    let [a, b, c] = p.map(I::middle);
    if a == 0.0 {
        return [if b == 0.0 { None } else { Some(-c / b) }, None];
    }
    let discriminant = b.mul_add(b, -4.0 * a * c);
    if discriminant < 0.0 {
        return [None; 2];
    }
    let stable = -0.5 * (b + discriminant.sqrt().copysign(b));
    if stable == 0.0 {
        [Some(-b / (2.0 * a)), None]
    } else {
        [Some(stable / a), Some(c / stable)]
    }
}
fn roots(p: Polynomial) -> Result<Roots, Error> {
    let mut result = Roots {
        times: [None; 2],
        coincident: false,
        unknown: false,
    };
    if zero(p[0]) && zero(p[1]) && zero(p[2]) {
        result.coincident = true;
        return Ok(result);
    }
    let unit = I::new(0.0, 1.0)?;
    if !contains_zero(evaluate(p, unit)?) {
        return Ok(result);
    }
    if zero(p[0]) && !contains_zero(p[1]) {
        let r = quotient(p[2].scale(-1.0)?, p[1])?;
        if r.intersect(unit).is_some() {
            result.times[0] = Some(Root {
                range: r,
                exists: true,
            });
        }
        return Ok(result);
    }
    if !contains_zero(p[0]) {
        let discriminant = p[1].square()?.sub(p[0].mul(p[2])?.scale(4.0)?)?;
        if discriminant.hi() < 0.0 {
            return Ok(result);
        }
        if discriminant.lo() > 0.0 {
            let d = discriminant.sqrt()?;
            let denominator = p[0].scale(2.0)?;
            let a = quotient(p[1].scale(-1.0)?.sub(d)?, denominator)?;
            let b = quotient(p[1].scale(-1.0)?.add(d)?, denominator)?;
            result.times = [
                if a.intersect(unit).is_some() {
                    Some(Root {
                        range: a,
                        exists: true,
                    })
                } else {
                    None
                },
                if b.intersect(unit).is_some() {
                    Some(Root {
                        range: b,
                        exists: true,
                    })
                } else {
                    None
                },
            ];
            return Ok(result);
        }
    }
    // One analytic candidate + mean-value enclosure handles nearly linear cells.
    let derivative = p[1].add(p[0].mul(I::new(0.0, 2.0)?)?)?;
    if contains_zero(derivative) {
        result.unknown = true;
    } else {
        let first = p[2];
        let last = evaluate(p, I::ONE)?;
        if (first.lo() > 0.0 && last.lo() > 0.0) || (first.hi() < 0.0 && last.hi() < 0.0) {
            return Ok(result);
        }
        let mut candidate = 0.5;
        for value in scalar_roots(p).into_iter().flatten() {
            if (0.0..=1.0).contains(&value) {
                candidate = value;
                break;
            }
        }
        let range =
            I::point(candidate)?.sub(quotient(evaluate(p, I::point(candidate)?)?, derivative)?)?;
        if range.intersect(unit).is_some() {
            result.times[0] = Some(Root {
                range,
                exists: (first.hi() < 0.0 && last.lo() > 0.0)
                    || (first.lo() > 0.0 && last.hi() < 0.0),
            });
        }
    }
    Ok(result)
}
fn compose(c: [I; 4], x: I, dx: I, y: I, dy: I) -> Result<Polynomial, Error> {
    Ok([
        c[3].mul(dx)?.mul(dy)?,
        c[1].mul(dx)?
            .add(c[2].mul(dy)?)?
            .add(c[3].mul(x.mul(dy)?.add(y.mul(dx)?)?)?)?,
        c[0].add(c[1].mul(x)?)?
            .add(c[2].mul(y)?)?
            .add(c[3].mul(x)?.mul(y)?)?,
    ])
}
fn coordinate(piece: Piece, axis: usize, ray: Ray, time: I) -> Result<I, Error> {
    let scale = f64::from(piece.resolution) * 0.5;
    Ok(time
        .scale(ray.d[axis])?
        .add(I::point(ray.o[axis])?)?
        .scale(scale)?
        .add(I::point(scale - f64::from(piece.ij[axis]))?)?)
}
fn contact(field: Field<'_>, ray: Ray, time: I) -> Result<Contact, Error> {
    let t = time.middle();
    let p = ray_point(ray, t);
    let xy = [p[0].clamp(-1.0, 1.0), p[1].clamp(-1.0, 1.0)];
    let value = field.sample(xy)?;
    let point = [xy[0], xy[1], value.height];
    let residual = (point[0] - p[0])
        .hypot(point[1] - p[1])
        .hypot(point[2] - p[2]);
    Ok(Contact {
        time,
        p: point,
        normal: value.normal(),
        error: (time.hi() - time.lo()).mul_add(0.5, residual) + rounding(ray, t) + field.roundoff(),
    })
}
fn slope(field: Field<'_>) -> f64 {
    let mut value = 0.0;
    for order in 0..4 {
        value = (field.amplitudes[order].abs() * f64::from(field.edge * repeat(order)))
            .mul_add(0.5, value);
    }
    value
}
fn mask_range(field: Field<'_>, ray: Ray, time: I) -> Result<Option<I>, Error> {
    let p = ray_point(ray, time.middle());
    if p[0].abs() > 1.0 || p[1].abs() > 1.0 {
        return Ok(None);
    }
    let value = field.sample([p[0], p[1]])?;
    let radius = (time.hi() - time.lo()) * 0.5;
    let delta = f64::EPSILON.mul_add(
        p[0].abs() + p[1].abs(),
        radius * (ray.d[0].abs() + ray.d[1].abs()),
    ) + f64::from_bits(4);
    let error = (delta * 255.0 * f64::from(field.edge)).mul_add(0.5, 64.0 * f64::EPSILON * 1024.0);
    Ok(Some(I::new(
        (value.coverage - error).next_down(),
        (value.coverage + error).next_up(),
    )?))
}
fn actual_sign(field: Field<'_>, ray: Ray, t: f64) -> Result<Option<I>, Error> {
    let p = ray_point(ray, t);
    let xy_error = [
        if ray.d[0] == 0.0 {
            0.0
        } else {
            f64::EPSILON.mul_add(p[0].abs(), f64::from_bits(2))
        },
        if ray.d[1] == 0.0 {
            0.0
        } else {
            f64::EPSILON.mul_add(p[1].abs(), f64::from_bits(2))
        },
    ];
    if p[0].abs() + xy_error[0] > 1.0 || p[1].abs() + xy_error[1] > 1.0 {
        return Ok(None);
    }
    let value = field.sample([p[0], p[1]])?;
    let residual = p[2] - value.height;
    let error = f64::EPSILON.mul_add(
        p[2].abs() + residual.abs(),
        slope(field).mul_add(xy_error[0] + xy_error[1], field.roundoff()),
    ) + f64::from_bits(8);
    Ok(Some(I::new(
        (residual - error).next_down(),
        (residual + error).next_up(),
    )?))
}
fn inside_piece(piece: Piece, ray: Ray, time: I) -> Result<bool, Error> {
    let x = coordinate(piece, 0, ray, time)?;
    let y = coordinate(piece, 1, ray, time)?;
    Ok(x.lo() >= 0.0 && x.hi() <= 1.0 && y.lo() >= 0.0 && y.hi() <= 1.0 && time.lo() >= 0.0)
}
fn coincident_entry(
    field: Field<'_>,
    piece: Piece,
    ray: Ray,
    range: Range,
    mask_coincident: bool,
    best: &mut Option<Contact>,
) -> Result<Option<f64>, Error> {
    let far = range.far.ok_or(Error::InvalidRay)?;
    let entry = range.near_upper.max(range.near);
    if entry > range.far_lower.unwrap_or(far) {
        let coverage = mask_range(field, ray, I::new(range.near, far)?)?;
        return Ok(coverage
            .is_none_or(|mask| mask.hi() >= 127.5)
            .then_some(range.near));
    }
    let at_entry = I::point(entry)?;
    let coverage = mask_range(field, ray, at_entry)?;
    if !mask_coincident && coverage.is_some_and(|mask| mask.hi() < 127.5) {
        return Ok(None);
    }
    if (mask_coincident || coverage.is_some_and(|mask| mask.lo() >= 127.5))
        && inside_piece(piece, ray, at_entry)?
    {
        choose(best, contact(field, ray, I::new(range.near, entry)?)?)?;
        Ok(None)
    } else {
        Ok(Some(range.near))
    }
}
fn certified_time(
    field: Field<'_>,
    piece: Piece,
    ray: Ray,
    time: I,
    exists: bool,
    mask_root: bool,
) -> Result<Option<I>, Error> {
    if time.hi() < 0.0 {
        return Ok(None);
    }
    let inside = inside_piece(piece, ray, time)?;
    if mask_root {
        return Ok((inside && exists).then_some(time));
    }
    if inside && exists && mask_range(field, ray, time)?.is_some_and(|m| m.lo() >= 127.5) {
        return Ok(Some(time));
    }
    // At a cell seam certify against the actual continuous summed surface.
    let pad = 32.0 * rounding(ray, time.middle());
    let bracket = I::new((time.lo() - pad).max(0.0), (time.hi() + pad).next_up())?;
    if !mask_range(field, ray, bracket)?.is_some_and(|m| m.lo() >= 127.5) {
        return Ok(None);
    }
    let Some(a) = actual_sign(field, ray, bracket.lo())? else {
        return Ok(None);
    };
    let Some(b) = actual_sign(field, ray, bracket.hi())? else {
        return Ok(None);
    };
    Ok(((a.hi() < 0.0 && b.lo() > 0.0) || (a.lo() > 0.0 && b.hi() < 0.0)).then_some(bracket))
}
fn choose(best: &mut Option<Contact>, mut candidate: Contact) -> Result<(), Error> {
    let Some(old) = *best else {
        *best = Some(candidate);
        return Ok(());
    };
    let time = I::new(
        old.time.lo().min(candidate.time.lo()),
        old.time.hi().min(candidate.time.hi()),
    )?;
    if candidate.time.middle() < old.time.middle() {
        candidate.error += (candidate.time.middle() - time.middle()).abs();
        candidate.time = time;
        *best = Some(candidate);
    } else {
        let mut retained = old;
        retained.error += (old.time.middle() - time.middle()).abs();
        retained.time = time;
        *best = Some(retained);
    }
    Ok(())
}
fn piece_query(
    field: Field<'_>,
    piece: Piece,
    ray: Ray,
    mut range: Range,
    best: &mut Option<Contact>,
) -> Result<Option<f64>, Error> {
    let resolution = f64::from(piece.resolution);
    for axis in 0..2 {
        let lo = I::point(2.0 * f64::from(piece.ij[axis]))?
            .divide(resolution)?
            .sub(I::ONE)?;
        let hi = I::point(2.0 * f64::from(piece.ij[axis] + 1))?
            .divide(resolution)?
            .sub(I::ONE)?;
        let Some(next) = range.axis(ray.o[axis], ray.d[axis], lo.lo(), hi.hi())? else {
            return Ok(None);
        };
        range = next;
    }
    let far = range.far.ok_or(Error::InvalidRay)?;
    let pad = 2.0 * rounding(ray, range.near);
    let start = (range.near - pad).max(0.0);
    let span = (far + pad) - start;
    if !span.is_finite() || span <= 0.0 {
        return Ok(Some(range.near));
    }
    let x = coordinate(piece, 0, ray, I::point(start)?)?;
    let y = coordinate(piece, 1, ray, I::point(start)?)?;
    let dx = I::point(ray.d[0])?.scale(span * resolution * 0.5)?;
    let dy = I::point(ray.d[1])?.scale(span * resolution * 0.5)?;
    let mut geometry = compose(piece.height, x, dx, y, dy)?;
    geometry[0] = geometry[0].scale(-1.0)?;
    geometry[1] = I::point(ray.d[2])?.scale(span)?.sub(geometry[1])?;
    geometry[2] = I::point(ray.d[2])?
        .scale(start)?
        .add(I::point(ray.o[2])?)?
        .sub(geometry[2])?;
    let geometry_roots = roots(geometry)?;
    let mut pending = geometry_roots.unknown.then_some(range.near);
    let mut candidates = geometry_roots.times;
    let coincident = geometry_roots.coincident;
    if coincident {
        let mut mask = compose(piece.mask, x, dx, y, dy)?;
        mask[2] = mask[2].sub(I::point(127.5)?)?;
        let mask_roots = roots(mask)?;
        candidates = mask_roots.times;
        if mask_roots.unknown {
            pending = Some(range.near);
        }
        if let Some(lower) =
            coincident_entry(field, piece, ray, range, mask_roots.coincident, best)?
        {
            pending = Some(pending.map_or(lower, |old| old.min(lower)));
        }
    }
    for root in candidates.into_iter().flatten() {
        let time = root.range.scale(span)?.add(I::point(start)?)?;
        if time.hi() < 0.0 {
            continue;
        }
        if !coincident && mask_range(field, ray, time)?.is_some_and(|m| m.hi() < 127.5) {
            continue;
        }
        if let Some(time) = certified_time(field, piece, ray, time, root.exists, coincident)? {
            choose(best, contact(field, ray, time)?)?;
        } else {
            pending = Some(pending.map_or(range.near, |v| v.min(range.near)));
        }
    }
    Ok(pending)
}
fn perpendicular(field: Field<'_>, ray: Ray) -> Result<Traversal, Error> {
    let mut q = Traversal {
        contact: None,
        visits: 1,
        complete: true,
        uncertain: false,
        pending_lower: None,
    };
    if ray.o[0].abs() > 1.0 || ray.o[1].abs() > 1.0 {
        return Ok(q);
    }
    let value = field.sample([ray.o[0], ray.o[1]])?;
    let mask_error = 64.0 * f64::EPSILON * 1024.0;
    if value.coverage < 127.5 - mask_error {
        return Ok(q);
    }
    let height = I::new(
        (value.height - field.roundoff()).next_down(),
        (value.height + field.roundoff()).next_up(),
    )?;
    let time = height
        .sub(I::point(ray.o[2])?)?
        .divide(ray.d[2].abs())?
        .scale(ray.d[2].signum())?;
    if time.hi() < 0.0 {
        return Ok(q);
    }
    if time.lo() < 0.0 || value.coverage < 127.5 + mask_error {
        q.complete = false;
        q.uncertain = true;
        q.pending_lower = Some(time.lo().max(0.0));
        return Ok(q);
    }
    q.contact = Some(contact(field, ray, time)?);
    Ok(q)
}
fn children(parent: Node, ray: Ray, range: Range) -> Result<[Option<Work>; 4], Error> {
    let mut children = [None; 4];
    for (out, child) in children.iter_mut().zip(0..4) {
        let node = Node {
            ij: [
                parent.ij[0] * 2 + (child & 1),
                parent.ij[1] * 2 + (child >> 1),
            ],
            depth: parent.depth + 1,
        };
        let [lo, hi] = node.domain()?;
        let mut xy = Some(range);
        for axis in 0..2 {
            if let Some(current) = xy {
                xy = current.axis(ray.o[axis], ray.d[axis], lo[axis], hi[axis])?;
            }
        }
        if let Some(xy) = xy {
            *out = Some(Work {
                node,
                lower: xy.near,
            });
        }
    }
    Ok(children)
}
fn checked_limit(ray: Ray, budget: u32) -> Result<usize, Error> {
    let norm = ray.d[0].hypot(ray.d[1]).hypot(ray.d[2]);
    if !crate::math::types::finite3(ray.o)
        || !crate::math::types::finite3(ray.d)
        || (norm - 1.0).abs() > 32.0 * f64::EPSILON
    {
        return Err(Error::InvalidRay);
    }
    usize::try_from(budget.min(64)).map_err(|_| Error::Numeric)
}
pub fn query(field: Field<'_>, ray: Ray, budget: u32) -> Result<Traversal, Error> {
    let limit = checked_limit(ray, budget)?;
    if limit == 0 {
        return Ok(Traversal {
            contact: None,
            visits: 0,
            complete: false,
            uncertain: false,
            pending_lower: Some(0.0),
        });
    }
    if ray.d[0] == 0.0 && ray.d[1] == 0.0 {
        return perpendicular(field, ray);
    }
    let resolution = field.resolution()?;
    let depth = (32 - (resolution - 1).leading_zeros()).min(DEPTH);
    let mut work = [Work {
        node: Node::default(),
        lower: 0.0,
    }; STEPS];
    let mut len = 1;
    let mut best: Option<Contact> = None;
    let mut pending = None;
    let mut visits = 0;
    for visit in 0..STEPS {
        if visit >= limit || len == 0 {
            break;
        }
        let item = pop(&mut work, &mut len)?;
        if best.is_some_and(|hit| item.lower > hit.time.hi()) {
            continue;
        }
        visits += 1;
        let Some(bounds) = field.bounds(item.node)? else {
            continue;
        };
        let Some(range) = box_range(ray, bounds.lo, bounds.hi)? else {
            continue;
        };
        if best.is_some_and(|hit| range.near > hit.time.hi()) {
            continue;
        }
        if item.node.depth < depth {
            for child in children(item.node, ray, range)?.into_iter().flatten() {
                if len == STEPS {
                    pending = Some(minimum(pending, child.lower));
                    continue;
                }
                work[len] = child;
                len += 1;
            }
        } else {
            let side = 1_u64 << item.node.depth;
            let first = item
                .node
                .ij
                .map(|v| u64::from(v) * u64::from(resolution) / side);
            let last = item
                .node
                .ij
                .map(|v| (u64::from(v + 1) * u64::from(resolution) - 1) / side);
            for dy in 0..2 {
                for dx in 0..2 {
                    let ij = [first[0] + dx, first[1] + dy];
                    if ij[0] > last[0]
                        || ij[1] > last[1]
                        || ij[0] >= u64::from(resolution)
                        || ij[1] >= u64::from(resolution)
                    {
                        continue;
                    }
                    let ij = [
                        u32::try_from(ij[0]).map_err(|_| Error::Numeric)?,
                        u32::try_from(ij[1]).map_err(|_| Error::Numeric)?,
                    ];
                    if let Some(lower) =
                        piece_query(field, field.piece(ij)?, ray, range, &mut best)?
                    {
                        pending = Some(minimum(pending, lower));
                    }
                }
            }
        }
    }
    for item in work.iter().take(len) {
        pending = Some(minimum(pending, item.lower));
    }
    if best.is_some_and(|hit| pending.is_some_and(|v| v > hit.time.hi())) {
        pending = None;
    }
    Ok(Traversal {
        contact: best,
        visits,
        complete: pending.is_none(),
        uncertain: pending.is_some(),
        pending_lower: pending,
    })
}
