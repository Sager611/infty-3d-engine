use std::fmt;

use crate::math::{CameraBasis, Ray, basis, camera_at};

const MIN_PITCH: f64 = -1.45;
const MAX_PITCH: f64 = 1.45;
const MIN_DISTANCE: f64 = 0.0001;
const MAX_DISTANCE: f64 = 100.0;
const TARGET_LIMIT: f64 = 20.0;
const CAMERA_PACKET_LEN: usize = 6;

/// A validated orbit camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub yaw: f64,
    pub pitch: f64,
    pub distance: f64,
    pub target: [f64; 3],
}

impl Camera {
    /// Return the camera's orthonormal orbit basis.
    #[must_use]
    pub fn basis(self) -> CameraBasis {
        basis(self.yaw, self.pitch, self.distance, self.target)
    }

    /// Build a ray through the center of the selected pixel.
    pub fn ray(self, width: u32, height: u32, x: f64, y: f64) -> Result<Ray, CameraError> {
        let width = f64::from(width);
        let height = f64::from(height);
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err(CameraError::InvalidDimensions);
        }
        if !x.is_finite() || !y.is_finite() {
            return Err(CameraError::NonFinite);
        }
        if !(0.0..width).contains(&x) || !(0.0..height).contains(&y) {
            return Err(CameraError::InvalidPixel);
        }

        Ok(camera_at(
            self.yaw,
            self.pitch,
            self.distance,
            self.target,
            [width, height],
            [x.floor() + 0.5, y.floor() + 0.5],
        ))
    }
}

/// Errors raised while parsing or sampling a camera.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraError {
    InvalidLength,
    NonFinite,
    InvalidPitch,
    InvalidDistance,
    InvalidTarget,
    InvalidDimensions,
    InvalidPixel,
}

impl fmt::Display for CameraError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidLength => "camera packet must contain exactly six values",
            Self::NonFinite => "camera contains a non-finite value",
            Self::InvalidPitch => "camera pitch is out of bounds",
            Self::InvalidDistance => "camera distance is out of bounds",
            Self::InvalidTarget => "camera target is out of bounds",
            Self::InvalidDimensions => "camera dimensions must be finite and positive",
            Self::InvalidPixel => "camera pixel is out of bounds",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for CameraError {}

/// Parse the canonical [yaw, pitch, distance, target-x, target-y, target-z] packet.
pub fn parse_camera(packet: &[f64]) -> Result<Camera, CameraError> {
    if packet.len() != CAMERA_PACKET_LEN {
        return Err(CameraError::InvalidLength);
    }
    if packet.iter().any(|value| !value.is_finite()) {
        return Err(CameraError::NonFinite);
    }

    let pitch = packet[1];
    if !(MIN_PITCH..=MAX_PITCH).contains(&pitch) {
        return Err(CameraError::InvalidPitch);
    }
    let distance = packet[2];
    if !(MIN_DISTANCE..=MAX_DISTANCE).contains(&distance) {
        return Err(CameraError::InvalidDistance);
    }
    let target = [packet[3], packet[4], packet[5]];
    if target
        .iter()
        .any(|value| !(-TARGET_LIMIT..=TARGET_LIMIT).contains(value))
    {
        return Err(CameraError::InvalidTarget);
    }

    Ok(Camera {
        yaw: canonical_yaw(packet[0]),
        pitch,
        distance,
        target,
    })
}

fn canonical_yaw(yaw: f64) -> f64 {
    let yaw = yaw.rem_euclid(std::f64::consts::TAU);
    if yaw == 0.0 { 0.0 } else { yaw }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(yaw: f64) -> [f64; 6] {
        [yaw, 0.2, 5.0, 1.0, 2.0, -3.0]
    }

    #[test]
    fn packet_wraps_yaw_and_is_idempotent() -> Result<(), CameraError> {
        let camera = parse_camera(&packet(-std::f64::consts::TAU - 0.25))?;
        let reparsed = parse_camera(&[
            camera.yaw,
            camera.pitch,
            camera.distance,
            camera.target[0],
            camera.target[1],
            camera.target[2],
        ])?;
        assert_eq!(camera, reparsed);
        assert!((0.0..std::f64::consts::TAU).contains(&camera.yaw));
        Ok(())
    }

    #[test]
    fn packet_reports_each_error_class() {
        assert!(matches!(
            parse_camera(&[0.0; 5]),
            Err(CameraError::InvalidLength)
        ));
        assert!(matches!(
            parse_camera(&[f64::NAN, 0.2, 5.0, 1.0, 2.0, -3.0]),
            Err(CameraError::NonFinite)
        ));
        assert!(matches!(
            parse_camera(&[0.0, 1.46, 5.0, 1.0, 2.0, -3.0]),
            Err(CameraError::InvalidPitch)
        ));
        assert!(matches!(
            parse_camera(&[0.0, 0.2, 0.00009, 1.0, 2.0, -3.0]),
            Err(CameraError::InvalidDistance)
        ));
        assert!(matches!(
            parse_camera(&[0.0, 0.2, 5.0, 20.1, 2.0, -3.0]),
            Err(CameraError::InvalidTarget)
        ));
    }

    #[test]
    fn basis_matches_center_ray_and_target_translation() -> Result<(), CameraError> {
        let camera = parse_camera(&[0.3, 0.2, 5.0, 3.0, 2.0, -1.0])?;
        let basis = camera.basis();
        let ray = camera.ray(200, 100, 99.0, 49.0)?;
        let expected = camera_at(
            camera.yaw,
            camera.pitch,
            camera.distance,
            camera.target,
            [200.0, 100.0],
            [99.5, 49.5],
        );
        assert_eq!(ray.o.map(f64::to_bits), basis.origin.map(f64::to_bits));
        assert_eq!(ray.o.map(f64::to_bits), expected.o.map(f64::to_bits));
        assert_eq!(ray.d.map(f64::to_bits), expected.d.map(f64::to_bits));
        let offset = [
            5.0 * 0.2f64.cos() * 0.3f64.sin(),
            5.0 * 0.2f64.sin(),
            5.0 * 0.2f64.cos() * 0.3f64.cos(),
        ];
        let translated: [f64; 3] = std::array::from_fn(|i| camera.target[i] + offset[i]);
        assert_eq!(basis.origin.map(f64::to_bits), translated.map(f64::to_bits));
        Ok(())
    }

    #[test]
    fn ray_rejects_invalid_dimensions_and_pixels() -> Result<(), CameraError> {
        let camera = parse_camera(&packet(0.0))?;
        assert!(matches!(
            camera.ray(0, 100, 0.0, 0.0),
            Err(CameraError::InvalidDimensions)
        ));
        assert!(matches!(
            camera.ray(100, 100, f64::NAN, 0.0),
            Err(CameraError::NonFinite)
        ));
        assert!(matches!(
            camera.ray(100, 100, 100.0, 0.0),
            Err(CameraError::InvalidPixel)
        ));
        Ok(())
    }
}
