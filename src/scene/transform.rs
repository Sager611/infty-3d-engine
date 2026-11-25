//! Rigid pose and positive uniform scale; bounded point/error adapter for every kernel.
use super::{model::SceneError, rotation::Rotation};
use crate::diagnostic::{Recorder, Tag};
use crate::geometry::{
    Error, Geometry, checked,
    point::{Closest, Request},
};
#[cfg(test)]
#[path = "transform/point_tests.rs"]
mod point_tests;
use crate::math::{
    Ray, Sample,
    goursat_impl::GoursatSample,
    interval::{Interval as I, norm},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    position: [f64; 3],
    scale: f64,
    rotation: Rotation,
}

impl Transform {
    pub const IDENTITY: Self = Self {
        position: [0.0; 3],
        scale: 1.0,
        rotation: Rotation::IDENTITY,
    };
    pub fn new(position: [f64; 3], scale: f64) -> Result<Self, SceneError> {
        if position.iter().any(|x| !x.is_finite()) {
            return Err(SceneError::NonFinite);
        }
        if position.iter().any(|x| !(-20.0..=20.0).contains(x)) {
            return Err(SceneError::InvalidPosition);
        }
        if !scale.is_finite() {
            return Err(SceneError::NonFinite);
        }
        if !(0.01..=4.0).contains(&scale) {
            return Err(SceneError::InvalidSize);
        }
        Ok(Self {
            position: position.map(|x| if x == 0.0 { 0.0 } else { x }),
            scale,
            rotation: Rotation::IDENTITY,
        })
    }
    pub fn with_rotation(mut self, rotation: [f64; 4]) -> Result<Self, SceneError> {
        self.rotation = Rotation::new(rotation)?;
        Ok(self)
    }
    pub const fn rotation(self) -> Rotation {
        self.rotation
    }
    pub const fn position(self) -> [f64; 3] {
        self.position
    }
    pub const fn scale(self) -> f64 {
        self.scale
    }
    pub fn local_point(self, point: [f64; 3]) -> [f64; 3] {
        self.rotation.inverse().apply(self.centered_point(point))
    }
    fn centered_point(self, point: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|axis| (point[axis] - self.position[axis]) / self.scale)
    }
    pub fn world_point(self, point: [f64; 3]) -> [f64; 3] {
        let point = self.rotation.apply(point);
        std::array::from_fn(|axis| self.scale.mul_add(point[axis], self.position[axis]))
    }
    pub fn closest(
        self,
        geometry: Geometry,
        point: [f64; 3],
        request: Request,
    ) -> Result<Closest, Error> {
        self.closest_traced(geometry, point, request, &mut Recorder::off())
    }
    pub fn closest_traced(
        self,
        geometry: Geometry,
        point: [f64; 3],
        request: Request,
        trace: &mut Recorder<'_>,
    ) -> Result<Closest, Error> {
        let result = self.closest_core(geometry, point, request, trace)?;
        result.record(trace, Tag::WorldBounds);
        Ok(result)
    }
    fn closest_core(
        self,
        geometry: Geometry,
        point: [f64; 3],
        request: Request,
        trace: &mut Recorder<'_>,
    ) -> Result<Closest, Error> {
        request.validate()?;
        if point.iter().any(|x| !x.is_finite()) {
            return Err(Error::InvalidPoint);
        }
        if request.max_evaluations == 0 {
            return Ok(Closest::unknown());
        }
        if self == Self::IDENTITY {
            return geometry.closest_traced(point, request, trace);
        }
        let centered = self.centered_point(point);
        let local = self.rotation.inverse().apply(centered);
        for i in 0..3 {
            if !local[i].is_finite() || (centered[i] == 0.0 && point[i] - self.position[i] != 0.0) {
                return Err(Error::Numeric);
            }
        }
        let absolute = request.absolute / self.scale;
        if !absolute.is_finite() || (request.absolute > 0.0 && absolute == 0.0) {
            return Err(Error::Numeric);
        }
        let local_request = Request {
            absolute,
            ..request
        };
        let input_error = self.input_error(point, local)?;
        let closest = geometry
            .closest_traced(local, local_request, trace)?
            .with_input_error(geometry, local, input_error)?;
        let lower = I::point(closest.distance.lower)?
            .scale(self.scale)?
            .lo()
            .max(0.0);
        let upper = if closest.distance.upper.is_finite() {
            I::point(closest.distance.upper)?.scale(self.scale)?.hi()
        } else {
            f64::INFINITY
        };
        let witness = closest.witness.map(|w| self.world_witness(w)).transpose()?;
        Ok(Closest {
            distance: crate::geometry::point::DistanceBounds { lower, upper },
            witness,
            ..closest
        }
        .with_accuracy(request))
    }
    fn input_error(self, point: [f64; 3], local: [f64; 3]) -> Result<f64, Error> {
        let [x, y, z] = std::array::from_fn(|i| {
            I::point(point[i])?
                .sub(I::point(self.position[i])?)?
                .divide(self.scale)
        });
        let bounded = self.rotation.inverse().bounds([x?, y?, z?])?;
        let [x, y, z] = std::array::from_fn(|i| bounded[i].sub(I::point(local[i])?).map(I::abs));
        Ok(norm([x?, y?, z?])?.hi())
    }
    fn world_witness(
        self,
        witness: crate::geometry::point::Witness,
    ) -> Result<crate::geometry::point::Witness, Error> {
        let point = self.world_point(witness.point);
        let [x, y, z] = witness.point.map(I::point);
        let bounded = self.rotation.bounds([x?, y?, z?])?;
        let [x, y, z] = std::array::from_fn(|i| {
            bounded[i]
                .scale(self.scale)?
                .add(I::point(self.position[i])?)?
                .sub(I::point(point[i])?)
                .map(I::abs)
        });
        let rounding = norm([x?, y?, z?])?.hi();
        let error = I::point(witness.error)?
            .scale(self.scale)?
            .add(I::point(rounding)?)?
            .hi();
        Ok(crate::geometry::point::Witness { point, error })
    }
    pub fn local_ray(self, ray: Ray) -> Ray {
        Ray {
            o: self.local_point(ray.o),
            d: self.rotation.inverse().apply(ray.d),
        }
    }
    pub fn world_query(self, result: GoursatSample) -> Result<GoursatSample, Error> {
        let sample = result.sample;
        checked(GoursatSample {
            sample: Sample {
                hit: sample.hit,
                t: sample.t * self.scale,
                gap: if sample.hit {
                    0.0
                } else {
                    sample.gap * self.scale
                },
                p: self.world_point(sample.p),
                n: self.rotation.apply(sample.n),
                uv: sample.uv,
            },
            gap_lower: if sample.hit {
                0.0
            } else {
                result.gap_lower * self.scale
            },
            error: result.error * self.scale,
            uncertain: result.uncertain,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uniform_point_and_ray_transforms_preserve_direction() -> Result<(), SceneError> {
        let transform = Transform::new([2.0, -3.0, 4.0], 2.0)?;
        let point = [3.0, 1.0, 8.0];
        let expected = [0.5, 2.0, 2.0];
        assert_eq!(
            transform.local_point(point).map(f64::to_bits),
            expected.map(f64::to_bits)
        );
        assert_eq!(
            transform.world_point(expected).map(f64::to_bits),
            point.map(f64::to_bits)
        );
        let ray = transform.local_ray(Ray {
            o: point,
            d: [0.0, 0.0, -1.0],
        });
        assert_eq!(ray.o.map(f64::to_bits), expected.map(f64::to_bits));
        assert_eq!(ray.d.map(f64::to_bits), [0.0, 0.0, -1.0].map(f64::to_bits));
        assert_eq!(
            transform.position().map(f64::to_bits),
            [2.0, -3.0, 4.0].map(f64::to_bits)
        );
        assert_eq!(transform.scale().to_bits(), 2.0_f64.to_bits());
        assert_eq!(Transform::IDENTITY.scale().to_bits(), 1.0_f64.to_bits());
        Ok(())
    }
    #[test]
    fn invalid_transforms_are_errors_not_clamped_values() {
        for scale in [0.0, -1.0, 5.0, f64::INFINITY] {
            assert!(Transform::new([0.0; 3], scale).is_err());
        }
        assert_eq!(
            Transform::new([21.0, 0.0, 0.0], 1.0),
            Err(SceneError::InvalidPosition)
        );
        assert_eq!(
            Transform::new([f64::NAN, 0.0, 0.0], 1.0),
            Err(SceneError::NonFinite)
        );
    }
}
