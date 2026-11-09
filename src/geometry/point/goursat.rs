//! Global closest bounds over six analytic charts, not spatial ray marching.
use super::{Closest, DistanceBounds, Multiplicity, Request, Resolution, Witness};
use crate::diagnostic::{Recorder, Tag};
use crate::{
    geometry::{Error, Geometry},
    math::interval::{Interval as I, distance, norm},
};
use std::{cmp::Ordering, collections::BinaryHeap};

#[derive(Clone, Copy)]
struct Patch {
    axis: usize,
    outer: bool,
    tangent: [I; 2],
}
#[derive(Clone, Copy)]
struct Cell {
    patch: Patch,
    lower: f64,
    serial: u32,
}
impl PartialEq for Cell {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Cell {}
impl PartialOrd for Cell {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Cell {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .lower
            .total_cmp(&self.lower)
            .then_with(|| other.serial.cmp(&self.serial))
    }
}
impl Patch {
    const fn axes(self) -> [usize; 2] {
        [(self.axis + 1) % 3, (self.axis + 2) % 3]
    }
    fn bounds(self, beta: f64, maximum: f64) -> Result<Option<[I; 3]>, Error> {
        let mut remaining = I::point(beta)?;
        for tangent in self.tangent {
            remaining = remaining.sub(tangent.square()?.sub(I::ONE)?.square()?)?;
        }
        let limit = if self.outer { beta } else { 1.0 };
        let Some(remaining) = remaining.intersect(I::new(0.0, limit)?) else {
            return Ok(None);
        };
        let root = remaining.sqrt()?;
        let squared = if self.outer {
            I::ONE.add(root)?
        } else {
            I::ONE.sub(root)?
        };
        let Some(squared) = squared.intersect(I::new(0.0, I::point(maximum)?.square()?.hi())?)
        else {
            return Ok(None);
        };
        let coordinate = squared.sqrt()?;
        let Some(coordinate) = coordinate.intersect(I::new(
            if self.outer { 1.0 } else { 0.0 },
            if self.outer { maximum } else { 1.0 },
        )?) else {
            return Ok(None);
        };
        let [a, b] = self.axes();
        let mut bounds = [I::ZERO; 3];
        bounds[a] = self.tangent[0];
        bounds[b] = self.tangent[1];
        bounds[self.axis] = coordinate;
        let [gx, gy, gz] = bounds.map(|q| q.mul(q.square()?.sub(I::ONE)?).map(I::abs));
        let gradients = [gx?, gy?, gz?];
        // Every nonsingular surface point has a maximum-gradient axis. Keep ties.
        if [a, b]
            .into_iter()
            .any(|i| gradients[i].lo() > gradients[self.axis].hi())
        {
            return Ok(None);
        }
        Ok(Some(bounds))
    }
    fn split(self) -> Result<Option<[Self; 2]>, Error> {
        let widths = self.tangent.map(|v| v.hi() - v.lo());
        let first = usize::from(widths[1] > widths[0]);
        for axis in [first, 1 - first] {
            let range = self.tangent[axis];
            let middle = range.middle();
            if middle.to_bits() == range.lo().to_bits() || middle.to_bits() == range.hi().to_bits()
            {
                continue;
            }
            let mut children = [self; 2];
            children[0].tangent[axis] = I::new(range.lo(), middle)?;
            children[1].tangent[axis] = I::new(middle, range.hi())?;
            return Ok(Some(children));
        }
        Ok(None)
    }
}

struct Problem {
    beta: f64,
    point: [f64; 3],
    scale: f64,
    maximum: f64,
}
struct Best {
    witness: Witness,
    upper: f64,
}

fn projection(reference: [f64; 3], beta: f64) -> [f64; 3] {
    // This seed need not be accurate; only the certificate establishes optimality.
    let scale = reference.into_iter().fold(1.0, f64::max);
    let inverse = scale.recip();
    let mut lifted = reference.map(|x| (x / scale).mul_add(x / scale, -inverse * inverse));
    let length = lifted[0].hypot(lifted[1]).hypot(lifted[2]);
    if length == 0.0 {
        lifted = [beta.sqrt(), 0.0, 0.0];
    } else {
        lifted = lifted.map(|x| x * (beta.sqrt() / length));
    }
    if let Some(axis) = lifted.iter().position(|&x| x < -1.0) {
        let a = (axis + 1) % 3;
        let b = (axis + 2) % 3;
        let length = lifted[a].hypot(lifted[b]);
        lifted[axis] = -1.0;
        if length == 0.0 {
            lifted[a] = (beta - 1.0).sqrt();
            lifted[b] = 0.0;
        } else {
            let factor = (beta - 1.0).sqrt() / length;
            lifted[a] *= factor;
            lifted[b] *= factor;
        }
    }
    lifted.map(|x| (1.0 + x).sqrt())
}

fn surface_witness(point: [f64; 3], beta: f64) -> Result<Witness, Error> {
    let mut axes = [0, 1, 2];
    let gradient = point.map(|x| (x * x.mul_add(x, -1.0)).abs());
    axes.sort_by(|&a, &b| gradient[b].total_cmp(&gradient[a]));
    for axis in axes {
        let mut remaining = I::point(beta)?;
        for (i, &coordinate) in point.iter().enumerate() {
            if i != axis {
                remaining =
                    remaining.sub(I::point(coordinate)?.square()?.sub(I::ONE)?.square()?)?;
            }
        }
        let outer = point[axis] >= 1.0;
        if remaining.lo() < 0.0 || (!outer && remaining.hi() > 1.0) {
            continue;
        }
        let root = remaining.sqrt()?;
        let squared = if outer {
            I::ONE.add(root)?
        } else {
            I::ONE.sub(root)?
        };
        let coordinate = I::new(squared.lo().max(0.0), squared.hi())?.sqrt()?;
        let mut corrected = point;
        corrected[axis] = coordinate.middle();
        let error = I::point(corrected[axis])?.sub(coordinate)?.abs().hi();
        return Ok(Witness {
            point: corrected,
            error,
        });
    }
    Err(Error::Numeric)
}

impl Problem {
    fn candidate(&self, reference: [f64; 3]) -> Result<Best, Error> {
        let witness = surface_witness(projection(reference, self.beta), self.beta)?;
        let upper = distance(self.point, witness.point)?
            .add(I::point(witness.error)?)?
            .hi();
        Ok(Best { witness, upper })
    }
    fn box_lower(&self, bounds: [I; 3]) -> Result<f64, Error> {
        let [x, y, z] = std::array::from_fn(|i| {
            let p = I::point(self.point[i])?;
            if self.point[i] < bounds[i].lo() {
                I::point(bounds[i].lo())?.sub(p)
            } else if self.point[i] > bounds[i].hi() {
                p.sub(I::point(bounds[i].hi())?)
            } else {
                Ok(I::ZERO)
            }
        });
        Ok(norm([x?, y?, z?])?.lo())
    }
    fn lagrange_lower(&self, bounds: [I; 3]) -> Result<f64, Error> {
        let center = bounds.map(I::middle);
        let gradients = center.map(|x| 4.0 * x * x.mul_add(x, -1.0));
        let squared = gradients.into_iter().map(|x| x * x).sum::<f64>();
        let dot = (0..3)
            .map(|i| ((center[i] - self.point[i]) / self.scale) * gradients[i])
            .sum::<f64>();
        let lambda = if squared == 0.0 {
            0.0
        } else {
            -2.0 * dot / squared / self.scale
        };
        if !lambda.is_finite() {
            return Err(Error::Numeric);
        }
        let mut field = I::ZERO;
        let mut value = I::ZERO;
        for (i, &c) in center.iter().enumerate() {
            let coordinate = I::point(c)?;
            field = field.add(coordinate.square()?.sub(I::ONE)?.square()?)?;
            value = value.add(
                coordinate
                    .sub(I::point(self.point[i])?)?
                    .divide(self.scale)?
                    .square()?,
            )?;
        }
        let mut certificate = value.add(field.sub(I::point(self.beta)?)?.scale(lambda)?)?;
        // On F=0, L=distance²/scale² for any finite lambda. Bound each Taylor remainder.
        for i in 0..3 {
            let c = I::point(center[i])?;
            let offset = bounds[i].sub(c)?;
            let gradient = c
                .sub(I::point(self.point[i])?)?
                .divide(self.scale)?
                .divide(self.scale)?
                .scale(2.0)?
                .add(c.mul(c.square()?.sub(I::ONE)?)?.scale(lambda)?.scale(4.0)?)?;
            let curvature = bounds[i]
                .square()?
                .scale(12.0)?
                .sub(I::point(4.0)?)?
                .scale(lambda)?
                .add(I::point(2.0)?.divide(self.scale)?.divide(self.scale)?)?;
            let quadratic = I::point(curvature.lo())?
                .scale(0.5)?
                .mul(offset.square()?)?;
            certificate = certificate.add(gradient.mul(offset)?.add(quadratic)?)?;
        }
        Ok(I::point(certificate.lo().max(0.0))?
            .sqrt()?
            .scale(self.scale)?
            .lo())
    }
    fn evaluate(
        &self,
        patch: Patch,
        inherited: f64,
        best: &mut Best,
        serial: u32,
        trace: &mut Recorder<'_>,
    ) -> Result<Option<Cell>, Error> {
        let axis = [0.0, 1.0, 2.0][patch.axis];
        let outer = f64::from(u8::from(patch.outer));
        let Some(bounds) = patch.bounds(self.beta, self.maximum)? else {
            trace.emit(
                Tag::Reject,
                [4.0, axis, outer, inherited, best.upper, f64::from(serial)],
            );
            return Ok(None);
        };
        let candidate = self.candidate(bounds.map(I::middle))?;
        if candidate.upper < best.upper {
            *best = candidate;
        }
        let lower = inherited
            .max(self.box_lower(bounds)?)
            .max(self.lagrange_lower(bounds)?);
        trace.emit(
            Tag::Patch,
            [
                axis,
                outer,
                lower,
                best.upper,
                f64::from(serial),
                f64::from(u8::from(lower <= best.upper)),
            ],
        );
        Ok((lower <= best.upper).then_some(Cell {
            patch,
            lower,
            serial,
        }))
    }
    fn multiplicity(&self, upper: f64) -> Result<Multiplicity, Error> {
        let zeros = self.point.into_iter().filter(|&x| x == 0.0).count();
        // Two zero coordinates cannot occur on this surface: beta < 2.
        if zeros >= 2 {
            return Ok(Multiplicity::NonUnique);
        }
        // Lift u_i=q_i²-1: outside points minimize a strictly convex objective
        // over the convex ball |u|²<=beta, u_i>=-1. Nonzero input fixes signs.
        if zeros == 0 && self.outside()? {
            return Ok(Multiplicity::Unique);
        }
        if self
            .point
            .into_iter()
            .all(|x| x.to_bits() == self.point[0].to_bits())
        {
            let radial = I::point(self.beta)?.divide(3.0)?.sqrt()?;
            let inner = I::ONE.sub(radial)?.sqrt()?;
            let outer = I::ONE.add(radial)?.sqrt()?;
            let coordinate = I::point(self.point[0])?;
            let inner_lower = norm([coordinate.sub(inner)?; 3])?.lo();
            let outer_lower = norm([coordinate.sub(outer)?; 3])?.lo();
            // A better feasible point excludes every permutation-fixed point.
            if upper < inner_lower && upper < outer_lower {
                return Ok(Multiplicity::NonUnique);
            }
        }
        Ok(Multiplicity::Unknown)
    }
    fn outside(&self) -> Result<bool, Error> {
        if self.point.into_iter().any(|x| x > self.maximum) {
            return Ok(true);
        }
        let mut field = I::ZERO;
        for coordinate in self.point {
            field = field.add(I::point(coordinate)?.square()?.sub(I::ONE)?.square()?)?;
        }
        Ok(field.lo() > self.beta)
    }
}

fn symmetric(point: [f64; 3], beta: f64) -> Result<Option<Closest>, Error> {
    let outer = I::ONE.add(I::point(beta)?.sqrt()?)?.sqrt()?;
    let unit = 1.0_f64.to_bits();
    let (coordinates, exact) = if point.into_iter().all(|x| x == 0.0) {
        let a = I::ONE.sub(I::point(beta)?.divide(3.0)?.sqrt()?)?.sqrt()?;
        (
            [a; 3],
            I::point(3.0)?
                .sub(I::point(beta)?.scale(3.0)?.sqrt()?)?
                .sqrt()?,
        )
    } else if point.into_iter().all(|x| x.to_bits() == unit) {
        ([outer, I::ONE, I::ONE], outer.sub(I::ONE)?)
    } else if point.into_iter().filter(|&x| x == 0.0).count() == 2
        && point.into_iter().any(|x| x.to_bits() == unit)
    {
        let a = I::ONE.sub(I::point(beta)?.divide(2.0)?.sqrt()?)?.sqrt()?;
        (
            point.map(|x| if x.to_bits() == unit { I::ONE } else { a }),
            I::point(2.0)?
                .sub(I::point(beta)?.scale(2.0)?.sqrt()?)?
                .sqrt()?,
        )
    } else {
        return Ok(None);
    };
    let witness_point = coordinates.map(I::middle);
    let [x, y, z] =
        std::array::from_fn(|i| I::point(witness_point[i])?.sub(coordinates[i]).map(I::abs));
    let error = norm([x?, y?, z?])?.hi();
    Ok(Some(Closest {
        distance: DistanceBounds {
            lower: exact.lo().max(0.0),
            upper: exact.hi(),
        },
        witness: Some(Witness {
            point: witness_point,
            error,
        }),
        resolution: Resolution::Unresolved,
        multiplicity: Multiplicity::NonUnique,
        evaluations: 1,
    }))
}

fn result(problem: &Problem, best: &Best, lower: f64, evaluations: u32) -> Result<Closest, Error> {
    if lower > best.upper {
        return Err(Error::Numeric);
    }
    let multiplicity = problem.multiplicity(best.upper)?;
    Ok(Closest {
        distance: DistanceBounds {
            lower,
            upper: best.upper,
        },
        witness: Some(best.witness),
        resolution: Resolution::Unresolved,
        multiplicity,
        evaluations,
    })
}

fn solve(
    geometry: Geometry,
    point: [f64; 3],
    request: Request,
    trace: &mut Recorder<'_>,
) -> Result<Closest, Error> {
    let beta = geometry.parameter();
    if let Some(exact) = symmetric(point, beta)? {
        return Ok(exact);
    }
    let maximum = I::ONE.add(I::point(beta)?.sqrt()?)?.sqrt()?.hi();
    let problem = Problem {
        beta,
        point,
        scale: point.into_iter().fold(1.0, f64::max),
        maximum,
    };
    let mut best = problem.candidate(point)?;
    let length = distance(point, [0.0; 3])?;
    let radial = I::point(beta)?.scale(3.0)?.sqrt()?;
    let inner = I::point(3.0)?.sub(radial)?.sqrt()?;
    let outer = I::point(3.0)?.add(radial)?.sqrt()?;
    let annulus = (inner.lo() - length.hi())
        .max(length.lo() - outer.hi())
        .next_down()
        .max(0.0);
    let mut evaluations = 1;
    trace.emit(
        Tag::Bounds,
        [annulus, best.upper, best.witness.error, 1.0, 1.0, 2.0],
    );
    if request.max_evaluations < 7 || best.upper - annulus <= request.tolerance(best.upper) {
        return result(&problem, &best, annulus, evaluations);
    }
    let mut cells = BinaryHeap::new();
    for axis in 0..3 {
        for outer in [false, true] {
            evaluations += 1;
            let patch = Patch {
                axis,
                outer,
                tangent: [I::new(0.0, maximum)?; 2],
            };
            if let Some(cell) = problem.evaluate(patch, annulus, &mut best, evaluations, trace)? {
                cells.push(cell);
            }
        }
    }
    let mut terminal = f64::INFINITY;
    loop {
        let pending = cells
            .peek()
            .map_or(f64::INFINITY, |cell| cell.lower)
            .min(terminal);
        if !pending.is_finite() {
            return Err(Error::Numeric);
        }
        let lower = annulus.max(pending);
        if best.upper - lower <= request.tolerance(best.upper)
            || evaluations + 2 > request.max_evaluations
        {
            return result(&problem, &best, lower, evaluations);
        }
        let Some(cell) = cells.pop() else {
            return result(&problem, &best, lower, evaluations);
        };
        if cell.lower > best.upper {
            continue;
        }
        let Some(children) = cell.patch.split()? else {
            terminal = terminal.min(cell.lower);
            continue;
        };
        for patch in children {
            evaluations += 1;
            if let Some(child) =
                problem.evaluate(patch, cell.lower, &mut best, evaluations, trace)?
            {
                cells.push(child);
            }
        }
    }
}

pub(super) fn query(
    geometry: Geometry,
    point: [f64; 3],
    request: Request,
    trace: &mut Recorder<'_>,
) -> Result<Closest, Error> {
    let mut result = solve(geometry, point.map(f64::abs), request, trace)?;
    if let Some(witness) = result.witness.as_mut() {
        for (i, coordinate) in witness.point.iter_mut().enumerate() {
            if point[i] < 0.0 {
                *coordinate = -*coordinate;
            }
        }
    }
    Ok(result)
}
