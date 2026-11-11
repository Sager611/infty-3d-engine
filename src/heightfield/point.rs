//! One bounding-node visit plus <=2 surface probes per evaluation, capped at64.
use super::{Error, Field, Node, STEPS, Work, minimum, pop};
use crate::{
    geometry::point::{Closest, DistanceBounds, Multiplicity, Request, Resolution, Witness},
    math::{
        interval::{Interval as I, distance},
        types::finite3,
    },
};
fn candidate(
    field: Field<'_>,
    point: [f64; 3],
    xy: [f64; 2],
    best: &mut Closest,
) -> Result<(), Error> {
    let value = field.sample(xy)?;
    if value.coverage < (64.0 * f64::EPSILON).mul_add(1024.0, 127.5) {
        return Ok(());
    }
    let witness = [xy[0], xy[1], value.height];
    let error = field.roundoff();
    let upper = distance(point, witness)?.add(I::point(error)?)?.hi();
    if upper < best.distance.upper {
        best.distance.upper = upper;
        best.witness = Some(Witness {
            point: witness,
            error,
        });
    }
    Ok(())
}
fn lower(
    work: &[Work; STEPS],
    len: usize,
    terminal: Option<f64>,
    forgot: Option<f64>,
    upper: f64,
) -> f64 {
    let mut result = terminal.map_or(upper, |v| v.min(upper));
    if let Some(v) = forgot {
        result = result.min(v);
    }
    for item in work.iter().take(len) {
        result = result.min(item.lower);
    }
    result
}
pub fn query(field: Field<'_>, point: [f64; 3], request: Request) -> Result<Closest, Error> {
    request.validate()?;
    if !finite3(point) {
        return Err(Error::InvalidPoint);
    }
    let limit = usize::try_from(request.max_evaluations.min(64)).map_err(|_| Error::Numeric)?;
    let mut best = Closest::unknown();
    if limit == 0 {
        return Ok(best);
    }
    candidate(field, point, field.anchor(), &mut best)?;
    if best.witness.is_none() {
        return Err(Error::InvalidParameter);
    }
    let mut work = [Work {
        node: Node::default(),
        lower: 0.0,
    }; STEPS];
    let mut len = 1;
    let mut terminal = None;
    let mut forgot = None;
    for visit in 0..STEPS {
        if visit >= limit || len == 0 {
            break;
        }
        let item = pop(&mut work, &mut len)?;
        best.evaluations += 1;
        if item.lower >= best.distance.upper {
            continue;
        }
        let Some(bounds) = field.bounds(item.node)? else {
            continue;
        };
        let projection = [0, 1, 2].map(|i| point[i].clamp(bounds.lo[i], bounds.hi[i]));
        let node_lower = distance(point, projection)?.lo();
        if node_lower >= best.distance.upper {
            continue;
        }
        let projected = [projection[0], projection[1]];
        let center = [
            bounds.lo[0].midpoint(bounds.hi[0]),
            bounds.lo[1].midpoint(bounds.hi[1]),
        ];
        for xy in [projected, center] {
            candidate(field, point, xy, &mut best)?;
        }
        if item.node.depth == 24 {
            terminal = Some(minimum(terminal, node_lower));
        } else {
            for child in 0..4 {
                if len == STEPS {
                    forgot = Some(minimum(forgot, node_lower));
                    continue;
                }
                work[len] = Work {
                    node: Node {
                        ij: [
                            item.node.ij[0] * 2 + (child & 1),
                            item.node.ij[1] * 2 + (child >> 1),
                        ],
                        depth: item.node.depth + 1,
                    },
                    lower: node_lower,
                };
                len += 1;
            }
        }
        best.distance.lower = lower(&work, len, terminal, forgot, best.distance.upper);
        if best.distance.upper - best.distance.lower <= request.tolerance(best.distance.upper) {
            break;
        }
    }
    best.distance = DistanceBounds {
        lower: lower(&work, len, terminal, forgot, best.distance.upper),
        upper: best.distance.upper,
    };
    best.multiplicity = Multiplicity::Unknown;
    best.resolution = Resolution::Unresolved;
    Ok(best.with_accuracy(request))
}
