//! Translation and positive uniform scale; one instance adapter for every kernel.
use super::model::SceneError;
use crate::geometry::{Error, checked};
use crate::math::{Ray, Sample, goursat_impl::GoursatSample};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    position: [f64; 3],
    scale: f64,
}

impl Transform {
    pub const IDENTITY: Self = Self {
        position: [0.0; 3],
        scale: 1.0,
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
        })
    }
    pub const fn position(self) -> [f64; 3] {
        self.position
    }
    pub const fn scale(self) -> f64 {
        self.scale
    }
    pub fn local_point(self, point: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|axis| (point[axis] - self.position[axis]) / self.scale)
    }
    pub fn world_point(self, point: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|axis| self.scale.mul_add(point[axis], self.position[axis]))
    }
    pub fn local_ray(self, ray: Ray) -> Ray {
        Ray {
            o: self.local_point(ray.o),
            d: ray.d,
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
                n: sample.n,
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
