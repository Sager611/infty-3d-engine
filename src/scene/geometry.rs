//! Canonical geometry interface: finite enclosure and analytic query.
//! Transforms, hierarchy, materials and packet adapters do not own kernels.
use super::model::Kind;
use crate::math::{
    Ray, goursat,
    goursat_impl::{GoursatError, GoursatSample},
    plane, sphere_at,
};

#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub kind: Kind,
    pub parameter: f64,
}

impl Geometry {
    pub fn radius(self) -> Option<f64> {
        match self.kind {
            Kind::Sphere => Some(1.0),
            Kind::Goursat => Some((3.0 + (3.0 * self.parameter).sqrt()).sqrt()),
            Kind::Plane => None,
        }
    }

    pub fn query(self, ray: Ray, need_gap: bool) -> Result<GoursatSample, GoursatError> {
        let sample = match self.kind {
            Kind::Sphere => sphere_at(ray, [0.0; 3], 1.0),
            Kind::Plane => plane(ray, [0.0; 3], [0.0, 1.0, 0.0]),
            Kind::Goursat => return goursat(ray, self.parameter, need_gap),
        };
        Ok(GoursatSample {
            sample,
            gap_lower: sample.gap,
            error: 0.0,
            uncertain: false,
        })
    }
}
