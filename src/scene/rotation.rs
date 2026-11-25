//! Canonical finite quaternions; pure vector maps and directed-rounding bounds.
use super::model::SceneError;
use crate::math::{
    interval::{ArithmeticError, Interval as I},
    types::V,
    vector::cross,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rotation([f64; 4]);

impl Rotation {
    pub const IDENTITY: Self = Self([0.0, 0.0, 0.0, 1.0]);
    pub fn new(mut values: [f64; 4]) -> Result<Self, SceneError> {
        if values.iter().any(|v| !v.is_finite()) {
            return Err(SceneError::NonFinite);
        }
        let scale = values.into_iter().map(f64::abs).fold(0.0_f64, f64::max);
        if scale == 0.0 {
            return Err(SceneError::InvalidRotation);
        }
        let length = values.into_iter().fold(0.0_f64, f64::hypot);
        if (length - 1.0).abs() > 4.0 * f64::EPSILON {
            values = values.map(|v| v / scale);
            let length = values.into_iter().fold(0.0_f64, f64::hypot);
            values = values.map(|v| v / length);
        }
        let first = [values[3], values[0], values[1], values[2]]
            .into_iter()
            .find(|&v| v != 0.0)
            .unwrap_or(1.0);
        Ok(Self(
            values.map(|v| if v == 0.0 { 0.0 } else { v * first.signum() }),
        ))
    }
    pub const fn components(self) -> [f64; 4] {
        self.0
    }
    pub const fn inverse(self) -> Self {
        let [x, y, z, w] = self.0;
        Self([-x, -y, -z, w])
    }
    pub fn apply(self, point: V) -> V {
        if self == Self::IDENTITY {
            return point;
        }
        let [x, y, z, w] = self.0;
        let axis = [x, y, z];
        let crossed = cross(axis, point);
        let twice = cross(axis, crossed);
        let factor = 2.0 / w.mul_add(w, z.mul_add(z, y.mul_add(y, x * x)));
        std::array::from_fn(|i| factor.mul_add(w.mul_add(crossed[i], twice[i]), point[i]))
    }
    pub fn bounds(self, point: [I; 3]) -> Result<[I; 3], ArithmeticError> {
        if self == Self::IDENTITY {
            return Ok(point);
        }
        let [x, y, z, w] = self.0;
        let axis = [I::point(x)?, I::point(y)?, I::point(z)?];
        let crossed = interval_cross(axis, point)?;
        let twice = interval_cross(axis, crossed)?;
        let squared = self
            .0
            .into_iter()
            .try_fold(I::ZERO, |sum, v| sum.add(I::point(v)?.square()?))?;
        let factor = I::new(
            (2.0 / squared.hi()).next_down(),
            (2.0 / squared.lo()).next_up(),
        )?;
        let [bound_x, bound_y, bound_z] =
            std::array::from_fn(|i| point[i].add(factor.mul(crossed[i].scale(w)?.add(twice[i])?)?));
        Ok([bound_x?, bound_y?, bound_z?])
    }
}
fn interval_cross(a: [I; 3], b: [I; 3]) -> Result<[I; 3], ArithmeticError> {
    let component = |i: usize, j: usize| a[i].mul(b[j])?.sub(a[j].mul(b[i])?);
    Ok([component(1, 2)?, component(2, 0)?, component(0, 1)?])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalized_rotations_enclose_vector_rounding_and_invert()
    -> Result<(), Box<dyn std::error::Error>> {
        for values in [
            [0.0, 10.0, 0.0, 10.0],
            [0.2, -0.3, 0.4, 0.5],
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, 1e300, 0.0, 1e300],
            [0.0, 1e-300, 0.0, 1e-300],
        ] {
            let rotation = Rotation::new(values)?;
            assert_eq!(Rotation::new(rotation.components())?, rotation);
            assert_eq!(Rotation::new(values.map(|v| -v))?, rotation);
            for point in [
                [1.0, 2.0, -3.0],
                [1e-120, -2e-120, 3e-120],
                [1e120, -2e120, 3e120],
            ] {
                let transformed = rotation.apply(point);
                let [a, b, c] = point.map(|v| I::point(v).map_err(|_| "point interval"));
                let bounds = rotation
                    .bounds([a?, b?, c?])
                    .map_err(|_| "rotation bounds")?;
                let inverse = rotation.inverse().apply(transformed);
                for i in 0..3 {
                    assert!(bounds[i].lo() <= transformed[i] && transformed[i] <= bounds[i].hi());
                    assert!(
                        (inverse[i] - point[i]).abs()
                            <= 8.0
                                * f64::EPSILON
                                * point.iter().copied().map(f64::abs).fold(0.0_f64, f64::max)
                    );
                }
            }
        }
        assert!(Rotation::new([0.0; 4]).is_err());
        assert!(Rotation::new([0.0, 0.0, f64::NAN, 1.0]).is_err());
        Ok(())
    }
}
