//! Validated canonical kernels; instances and presentation do not own geometry.
pub mod point;
use crate::diagnostic::{Recorder, Tag};
use crate::math::{
    Ray,
    goursat_impl::{GoursatError, GoursatSample},
    goursat_traced, plane, sphere_at,
};

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Sphere = 1,
    Goursat = 2,
    Plane = 3,
}

impl Kind {
    pub const fn from_tag(tag: u32) -> Option<Self> {
        match tag {
            1 => Some(Self::Sphere),
            2 => Some(Self::Goursat),
            3 => Some(Self::Plane),
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
    };
    pub fn new(kind: Kind, parameter: f64) -> Result<Self, Error> {
        let valid = match kind {
            Kind::Goursat => (1.05..=1.95).contains(&parameter),
            Kind::Sphere | Kind::Plane => parameter == 0.0,
        };
        if !valid || !parameter.is_finite() {
            return Err(Error::InvalidParameter);
        }
        Ok(Self {
            kind,
            parameter: if parameter == 0.0 { 0.0 } else { parameter },
        })
    }
    pub const fn kind(self) -> Kind {
        self.kind
    }
    pub const fn parameter(self) -> f64 {
        self.parameter
    }
    pub fn radius(self) -> Option<f64> {
        match self.kind {
            Kind::Sphere => Some(1.0),
            Kind::Goursat => Some((3.0 + (3.0 * self.parameter).sqrt()).sqrt()),
            Kind::Plane => None,
        }
    }
    /// Query a finite unit-direction ray in canonical coordinates.
    pub fn query(self, ray: Ray, need_gap: bool) -> Result<GoursatSample, Error> {
        self.query_traced(ray, need_gap, &mut Recorder::off())
    }
    pub fn query_traced(
        self,
        ray: Ray,
        need_gap: bool,
        trace: &mut Recorder<'_>,
    ) -> Result<GoursatSample, Error> {
        let squared = ray.d.iter().map(|x| x * x).sum::<f64>();
        if ray.o.iter().chain(ray.d.iter()).any(|x| !x.is_finite())
            || (squared - 1.0).abs() > 32.0 * f64::EPSILON
        {
            return Err(Error::InvalidRay);
        }
        let sample = match self.kind {
            Kind::Sphere => sphere_at(ray, [0.0; 3], 1.0),
            Kind::Plane => plane(ray, [0.0; 3], [0.0, 1.0, 0.0]),
            Kind::Goursat => {
                let result = checked(
                    goursat_traced(ray, self.parameter, need_gap, trace).map_err(Error::Kernel)?,
                )?;
                record_ray(result, trace, Tag::Ray);
                return Ok(result);
            }
        };
        let result = checked(GoursatSample {
            sample,
            gap_lower: sample.gap,
            error: 0.0,
            uncertain: false,
            gap_computed: true,
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

pub fn record_ray(result: GoursatSample, trace: &mut Recorder<'_>, tag: Tag) {
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
            if result.gap_computed {
                f64::from(u8::from(result.uncertain))
            } else {
                2.0
            },
        ],
    );
}

pub fn checked(result: GoursatSample) -> Result<GoursatSample, Error> {
    let sample = result.sample;
    let distances = [sample.t, sample.gap, result.gap_lower, result.error];
    if distances.iter().any(|x| !x.is_finite() || *x < 0.0)
        || sample
            .p
            .iter()
            .chain(sample.n.iter())
            .chain(sample.uv.iter())
            .any(|x| !x.is_finite())
    {
        return Err(Error::Numeric);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kernel_rejects_invalid_rays() -> Result<(), Error> {
        for kind in [Kind::Sphere, Kind::Plane, Kind::Goursat] {
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
                    geometry.query(ray, false).is_err(),
                    "{kind:?} accepted an invalid ray"
                );
            }
        }
        assert!(
            matches!(
                Geometry::SPHERE.query(
                    Ray {
                        o: [f64::MAX, 0.0, 0.0],
                        d: [0.0, 0.0, -1.0]
                    },
                    false
                ),
                Err(Error::Numeric)
            ),
            "unrepresentable ray arithmetic returned a sample"
        );
        assert!(Geometry::new(Kind::Sphere, 1.0).is_err());
        assert!(Geometry::new(Kind::Goursat, f64::NAN).is_err());
        Ok(())
    }
}
