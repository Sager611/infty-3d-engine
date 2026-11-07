#![allow(clippy::many_single_char_names, reason = "Matches analytic equations.")]

#[path = "math/camera.rs"]
mod camera_impl;
#[path = "math/detail.rs"]
mod detail;
#[path = "math/footprint.rs"]
mod footprint_impl;
#[path = "math/plane.rs"]
mod plane_impl;
#[path = "math/sphere.rs"]
mod sphere_impl;
#[path = "math/types.rs"]
mod types;
#[path = "math/vector.rs"]
mod vector;

pub use camera_impl::camera;
pub use detail::pureweight;
pub use footprint_impl::footprint;
pub use plane_impl::plane;
pub use sphere_impl::sphere;
pub use types::V;
#[cfg(test)]
pub use types::{Ray, Sample};

#[cfg(test)]
#[path = "math/tests.rs"]
mod tests;
