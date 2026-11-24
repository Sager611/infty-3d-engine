//! Validated canonical kernels; instances and presentation do not own geometry.
pub mod finite;
pub mod point;
use crate::diagnostic::{Recorder, Tag};
use crate::math::{
    Ray, crystal,
    goursat_impl::GoursatError,
    goursat_traced, plane, sphere_at,
    types::{GeometrySample, finite3},
};

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Sphere = 1,
    Goursat = 2,
    Plane = 3,
    CrystallineCluster = 4,
    FinitePlane = 5,
}

impl Kind {
    pub const fn from_tag(tag: u32) -> Option<Self> {
        match tag {
            1 => Some(Self::Sphere),
            2 => Some(Self::Goursat),
            3 => Some(Self::Plane),
            4 => Some(Self::CrystallineCluster),
            5 => Some(Self::FinitePlane),
            _ => None,
        }
    }
    pub const fn tag(self) -> u32 {
        self as u32
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    kind: Kind,
    parameter: f64,
    leaf: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidParameter,
    InvalidRay,
    InvalidPoint,
    InvalidRequest,
    Numeric,
    Kernel(GoursatError),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidParameter => f.write_str("invalid geometry parameter"),
            Self::InvalidRay => f.write_str("canonical ray must be finite and unit length"),
            Self::InvalidPoint => f.write_str("point must be finite"),
            Self::InvalidRequest => f.write_str("invalid query accuracy or evaluation budget"),
            Self::Numeric => f.write_str("query arithmetic is not representable"),
            Self::Kernel(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for Error {}
impl From<crate::math::interval::ArithmeticError> for Error {
    fn from(_: crate::math::interval::ArithmeticError) -> Self {
        Self::Numeric
    }
}

impl Geometry {
    pub const SPHERE: Self = Self {
        kind: Kind::Sphere,
        parameter: 0.0,
        leaf: false,
    };
    pub fn new(kind: Kind, parameter: f64) -> Result<Self, Error> {
        let valid = match kind {
            Kind::Goursat => (1.05..=1.95).contains(&parameter),
            Kind::FinitePlane => matches!(parameter, 0.0 | 1.0 | 2.0),
            Kind::Sphere | Kind::Plane | Kind::CrystallineCluster => parameter == 0.0,
        };
        if !valid || !parameter.is_finite() {
            return Err(Error::InvalidParameter);
        }
        Ok(Self {
            kind,
            parameter: if parameter == 0.0 { 0.0 } else { parameter },
            leaf: false,
        })
    }
    pub const fn kind(self) -> Kind {
        self.kind
    }
    pub const fn with_leaf_surface(self, enabled: bool) -> Self {
        Self {
            leaf: enabled && matches!(self.kind, Kind::FinitePlane),
            ..self
        }
    }
    pub const fn parameter(self) -> f64 {
        self.parameter
    }
    pub fn radius(self) -> Option<f64> {
        match self.kind {
            Kind::Sphere => Some(1.0),
            Kind::Goursat => Some((3.0 + (3.0 * self.parameter).sqrt()).sqrt()),
            Kind::Plane => None,
            Kind::CrystallineCluster => Some(crystal::RADIUS),
            Kind::FinitePlane => Some(1.5),
        }
    }
    /// Query a finite unit-direction ray in canonical coordinates.
    pub fn query(self, ray: Ray) -> Result<GeometrySample, Error> {
        self.query_traced(ray, &mut Recorder::off())
    }
    pub fn query_traced(self, ray: Ray, trace: &mut Recorder<'_>) -> Result<GeometrySample, Error> {
        let squared = ray.d[0].mul_add(ray.d[0], ray.d[1].mul_add(ray.d[1], ray.d[2] * ray.d[2]));
        if !finite3(ray.o) || !finite3(ray.d) || (squared - 1.0).abs() > 32.0 * f64::EPSILON {
            return Err(Error::InvalidRay);
        }
        let sample = match self.kind {
            Kind::Sphere => sphere_at(ray, [0.0; 3], 1.0),
            Kind::Plane => plane(ray, [0.0; 3], [0.0, 1.0, 0.0]),
            Kind::Goursat | Kind::CrystallineCluster | Kind::FinitePlane => {
                let result = if self.kind == Kind::Goursat {
                    goursat_traced(ray, self.parameter, trace).map_err(Error::Kernel)?
                } else if self.kind == Kind::FinitePlane {
                    finite::query(ray, self.parameter)?
                } else {
                    crystal::query(ray)?
                };
                let result = checked(result)?;
                record_ray(result, trace, Tag::Ray);
                return Ok(result);
            }
        };
        let result = checked(GeometrySample {
            sample,
            gap_lower: sample.gap,
            error: 0.0,
            uncertain: false,
        })?;
        record_ray(result, trace, Tag::Ray);
        Ok(result)
    }
    pub fn closest_traced(
        self,
        point: [f64; 3],
        request: point::Request,
        trace: &mut crate::diagnostic::Recorder<'_>,
    ) -> Result<point::Closest, Error> {
        point::query_traced(self, point, request, trace)
    }
    pub fn closest(
        self,
        point: [f64; 3],
        request: point::Request,
    ) -> Result<point::Closest, Error> {
        point::query(self, point, request)
    }
}

pub fn record_ray(result: GeometrySample, trace: &mut Recorder<'_>, tag: Tag) {
    if !trace.enabled() {
        return;
    }
    trace.emit(
        tag,
        [
            f64::from(u8::from(result.sample.hit)),
            result.sample.t,
            result.gap_lower,
            result.sample.gap,
            result.error,
            f64::from(u8::from(result.uncertain)),
        ],
    );
}

pub fn checked(result: GeometrySample) -> Result<GeometrySample, Error> {
    let sample = result.sample;
    let valid_distance = |x: f64| x.is_finite() && x >= 0.0;
    if !valid_distance(sample.t)
        || !valid_distance(sample.gap)
        || !valid_distance(result.gap_lower)
        || !valid_distance(result.error)
        || !finite3(sample.p)
        || !finite3(sample.n)
        || !sample.uv[0].is_finite()
        || !sample.uv[1].is_finite()
    {
        return Err(Error::Numeric);
    }
    Ok(result)
}

#[cfg(test)]
#[path = "geometry/crystal_tests.rs"]
mod crystal_tests;
#[cfg(test)]
#[path = "geometry/finite_tests.rs"]
mod finite_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_geometry_returns_a_gap_with_a_surface_witness() -> Result<(), Error> {
        use crate::math::vector::{dot, madd, sub};
        let ray = Ray {
            o: [3.0, 3.0, 4.0],
            d: [0.0, 0.0, 1.0],
        };
        for (kind, parameter) in [
            (Kind::Sphere, 0.0),
            (Kind::Plane, 0.0),
            (Kind::FinitePlane, 0.0),
            (Kind::Goursat, 1.5),
            (Kind::CrystallineCluster, 0.0),
        ] {
            let result = Geometry::new(kind, parameter)?.query(ray)?;
            let sample = result.sample;
            let delta = sub(madd(ray.o, ray.d, sample.t), sample.p);
            assert!(
                !sample.hit && sample.gap > 0.0 && result.gap_lower <= sample.gap,
                "{kind:?}"
            );
            assert!(
                (dot(delta, delta).sqrt() - sample.gap).abs() <= result.error + 1e-10,
                "{kind:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn every_kernel_rejects_invalid_rays() -> Result<(), Error> {
        for kind in [
            Kind::Sphere,
            Kind::Plane,
            Kind::Goursat,
            Kind::CrystallineCluster,
        ] {
            let parameter = if kind == Kind::Goursat { 1.5 } else { 0.0 };
            let geometry = Geometry::new(kind, parameter)?;
            for ray in [
                Ray {
                    o: [f64::NAN, 0.0, 0.0],
                    d: [0.0, 0.0, -1.0],
                },
                Ray {
                    o: [0.0; 3],
                    d: [0.0; 3],
                },
            ] {
                assert!(
                    geometry.query(ray).is_err(),
                    "{kind:?} accepted an invalid ray"
                );
            }
        }
        assert!(
            matches!(
                Geometry::SPHERE.query(Ray {
                    o: [f64::MAX, 0.0, 0.0],
                    d: [0.0, 0.0, -1.0]
                }),
                Err(Error::Numeric)
            ),
            "unrepresentable ray arithmetic returned a sample"
        );
        assert!(Geometry::new(Kind::Sphere, 1.0).is_err());
        assert!(Geometry::new(Kind::Goursat, f64::NAN).is_err());
        Ok(())
    }
}
