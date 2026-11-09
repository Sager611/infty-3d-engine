//! Canonical closest-point requests and bounded results.
use super::{Error, Geometry};
use crate::diagnostic::{Recorder, Tag};
use crate::math::interval::{Interval as I, distance};
mod crystal;
mod goursat;

/// Absolute/relative distance accuracy and a finite evaluation budget.
#[derive(Clone, Copy, Debug)]
pub struct Request {
    pub absolute: f64,
    pub relative: f64,
    pub max_evaluations: u32,
}

impl Default for Request {
    fn default() -> Self {
        Self {
            absolute: 1e-8,
            relative: 1e-8,
            max_evaluations: 4096,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    Converged,
    Unresolved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Multiplicity {
    Unique,
    NonUnique,
    Unknown,
}

#[derive(Clone, Copy, Debug)]
pub struct DistanceBounds {
    pub lower: f64,
    pub upper: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Witness {
    pub point: [f64; 3],
    pub error: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Closest {
    pub distance: DistanceBounds,
    pub witness: Option<Witness>,
    pub resolution: Resolution,
    pub multiplicity: Multiplicity,
    pub evaluations: u32,
}

impl Closest {
    pub fn packed(self) -> [f64; 12] {
        let mut out = [0.0; 12];
        out[0] = f64::from(self.resolution as u32);
        out[1] = f64::from(self.multiplicity as u32);
        out[3] = self.distance.lower;
        out[4] = self.distance.upper;
        out[9] = f64::from(self.evaluations);
        if let Some(witness) = self.witness {
            out[2] = 1.0;
            out[5..8].copy_from_slice(&witness.point);
            out[8] = witness.error;
        }
        out
    }
}

impl Request {
    pub fn validate(self) -> Result<(), Error> {
        if !self.absolute.is_finite()
            || self.absolute < 0.0
            || !self.relative.is_finite()
            || !(0.0..=1.0).contains(&self.relative)
            || self.absolute + self.relative == 0.0
            || self.max_evaluations > 65_536
        {
            return Err(Error::InvalidRequest);
        }
        Ok(())
    }
    pub const fn tolerance(self, distance: f64) -> f64 {
        self.relative.mul_add(distance, self.absolute)
    }
}

impl Closest {
    pub fn record(self, trace: &mut Recorder<'_>, tag: Tag) {
        if !trace.enabled() {
            return;
        }
        let p = self.packed();
        trace.emit(tag, [p[3], p[4], p[8], p[9], p[0], p[1]]);
    }
    /// Input uncertainty changes distance bounds, not the surface witness.
    pub fn with_input_error(
        mut self,
        geometry: Geometry,
        point: [f64; 3],
        error: f64,
    ) -> Result<Self, Error> {
        if !error.is_finite() || error < 0.0 {
            return Err(Error::Numeric);
        }
        if error == 0.0 {
            return Ok(self);
        }
        let uncertainty = I::point(error)?;
        self.distance.lower = I::point(self.distance.lower)?
            .sub(uncertainty)?
            .lo()
            .max(0.0);
        if self.distance.upper.is_finite() {
            self.distance.upper = I::point(self.distance.upper)?.add(uncertainty)?.hi();
        }
        self.multiplicity = match geometry.kind() {
            super::Kind::Plane => Multiplicity::Unique,
            super::Kind::Sphere if distance(point, [0.0; 3])?.lo() > error => Multiplicity::Unique,
            _ => Multiplicity::Unknown,
        };
        self.resolution = Resolution::Unresolved;
        Ok(self)
    }
    pub fn with_accuracy(mut self, request: Request) -> Self {
        self.resolution = if self.distance.upper.is_finite()
            && self.distance.upper - self.distance.lower <= request.tolerance(self.distance.upper)
        {
            Resolution::Converged
        } else {
            Resolution::Unresolved
        };
        self
    }
    pub const fn unknown() -> Self {
        Self {
            distance: DistanceBounds {
                lower: 0.0,
                upper: f64::INFINITY,
            },
            witness: None,
            resolution: Resolution::Unresolved,
            multiplicity: Multiplicity::Unknown,
            evaluations: 0,
        }
    }
}

fn sphere(point: [f64; 3]) -> Result<Closest, Error> {
    let magnitude = point
        .iter()
        .map(|x| x.abs())
        .fold(f64::MIN_POSITIVE, f64::max);
    let direction = point.map(|x| x / magnitude);
    let normalizer = direction[0].hypot(direction[1]).hypot(direction[2]);
    let length = normalizer * magnitude;
    if !length.is_finite() {
        return Err(Error::Numeric);
    }
    let mut witness = Witness {
        point: [1.0, 0.0, 0.0],
        error: 0.0,
    };
    let mut distance = DistanceBounds {
        lower: 1.0,
        upper: 1.0,
    };
    let multiplicity = if length > 0.0 {
        witness.point = direction.map(|x| x / normalizer);
        witness.error = 16.0 * f64::EPSILON;
        let exact = (length - 1.0).abs();
        let rounding = 32.0 * f64::EPSILON * (length + 1.0);
        distance = DistanceBounds {
            lower: (exact - rounding).next_down().max(0.0),
            upper: (exact + rounding + witness.error).next_up(),
        };
        Multiplicity::Unique
    } else {
        Multiplicity::NonUnique
    };
    Ok(Closest {
        distance,
        witness: Some(witness),
        resolution: Resolution::Unresolved,
        multiplicity,
        evaluations: 1,
    })
}

pub fn query(geometry: Geometry, point: [f64; 3], request: Request) -> Result<Closest, Error> {
    query_traced(geometry, point, request, &mut Recorder::off())
}

pub fn query_traced(
    geometry: Geometry,
    point: [f64; 3],
    request: Request,
    trace: &mut Recorder<'_>,
) -> Result<Closest, Error> {
    request.validate()?;
    if !crate::math::types::finite3(point) {
        return Err(Error::InvalidPoint);
    }
    if request.max_evaluations == 0 {
        let result = Closest::unknown();
        result.record(trace, Tag::Bounds);
        return Ok(result);
    }
    let result = match geometry.kind() {
        super::Kind::Sphere => sphere(point)?,
        super::Kind::Plane => Closest {
            distance: DistanceBounds {
                lower: point[1].abs(),
                upper: point[1].abs(),
            },
            witness: Some(Witness {
                point: [point[0], 0.0, point[2]],
                error: 0.0,
            }),
            resolution: Resolution::Unresolved,
            multiplicity: Multiplicity::Unique,
            evaluations: 1,
        },
        super::Kind::Goursat => goursat::query(geometry, point, request, trace)?,
        super::Kind::CrystallineCluster => crystal::query(point, request)?,
    };
    let result = result.with_accuracy(request);
    result.record(trace, Tag::Bounds);
    Ok(result)
}

#[cfg(test)]
mod goursat_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Kind;

    #[test]
    fn sphere_witness_error_covers_subnormal_and_large_points() -> Result<(), Error> {
        for point in [
            [f64::from_bits(1); 3],
            [1e-300, -2e-300, 0.0],
            [1e200, 2e200, 0.0],
        ] {
            let result = Geometry::SPHERE.closest(point, Request::default())?;
            let Some(witness) = result.witness else {
                return Err(Error::Numeric);
            };
            let radius = witness.point[0]
                .hypot(witness.point[1])
                .hypot(witness.point[2]);
            assert!(
                (radius - 1.0).abs() <= witness.error,
                "sphere witness exceeds its position error"
            );
        }
        Ok(())
    }

    #[test]
    fn analytic_closest_points_cover_inside_outside_and_non_unique_centers() -> Result<(), Error> {
        let sphere = Geometry::new(Kind::Sphere, 0.0)?;
        for (point, expected, distance) in [
            ([3.0, 0.0, 0.0], [1.0, 0.0, 0.0], 2.0),
            ([0.0, 0.5, 0.0], [0.0, 1.0, 0.0], 0.5),
            ([1.0, 0.0, 0.0], [1.0, 0.0, 0.0], 0.0),
        ] {
            let result = sphere.closest(point, Request::default())?;
            assert!(result.distance.lower <= distance && distance <= result.distance.upper);
            assert_eq!(result.witness.map(|w| w.point), Some(expected));
            assert_eq!(result.resolution, Resolution::Converged);
            assert_eq!(result.multiplicity, Multiplicity::Unique);
            assert_eq!(result.evaluations, 1);
            assert_eq!(result.packed()[2].to_bits(), 1.0_f64.to_bits());
        }
        let center = sphere.closest([0.0; 3], Request::default())?;
        assert_eq!(center.multiplicity, Multiplicity::NonUnique);
        assert_eq!(center.distance.lower.to_bits(), 1.0_f64.to_bits());
        assert_eq!(center.distance.upper.to_bits(), 1.0_f64.to_bits());
        let plane =
            Geometry::new(Kind::Plane, 0.0)?.closest([3.0, -2.0, 4.0], Request::default())?;
        assert_eq!(plane.witness.map(|w| w.point), Some([3.0, 0.0, 4.0]));
        assert_eq!(plane.distance.lower.to_bits(), 2.0_f64.to_bits());
        assert_eq!(plane.distance.upper.to_bits(), 2.0_f64.to_bits());
        Ok(())
    }

    #[test]
    fn invalid_requests_are_errors_and_no_work_is_not_a_miss() -> Result<(), Error> {
        let geometry = Geometry::new(Kind::Sphere, 0.0)?;
        for request in [
            Request {
                absolute: f64::NAN,
                ..Request::default()
            },
            Request {
                absolute: 0.0,
                relative: 0.0,
                ..Request::default()
            },
            Request {
                relative: -1.0,
                ..Request::default()
            },
            Request {
                max_evaluations: u32::MAX,
                ..Request::default()
            },
        ] {
            assert!(matches!(
                geometry.closest([2.0, 0.0, 0.0], request),
                Err(Error::InvalidRequest)
            ));
        }
        assert!(matches!(
            geometry.closest([f64::INFINITY; 3], Request::default()),
            Err(Error::InvalidPoint)
        ));
        let none = geometry.closest(
            [2.0, 0.0, 0.0],
            Request {
                max_evaluations: 0,
                ..Request::default()
            },
        )?;
        assert_eq!(none.resolution, Resolution::Unresolved);
        assert_eq!(none.multiplicity, Multiplicity::Unknown);
        assert!(none.witness.is_none());
        assert_eq!(none.distance.lower.to_bits(), 0.0_f64.to_bits());
        assert_eq!(none.distance.upper.to_bits(), f64::INFINITY.to_bits());
        assert_eq!(none.evaluations, 0);
        assert_eq!(none.packed()[2].to_bits(), 0.0_f64.to_bits());
        Ok(())
    }
}
