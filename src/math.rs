#![allow(clippy::many_single_char_names, reason = "Matches analytic equations.")]

#[path = "math/camera.rs"]
pub mod camera_impl;
#[cfg(test)]
#[path = "math/detail.rs"]
pub mod detail;
#[cfg(test)]
#[path = "math/footprint.rs"]
pub mod footprint_impl;
#[path = "math/goursat.rs"]
pub mod goursat_impl;
#[path = "math/plane.rs"]
pub mod plane_impl;
#[path = "math/polynomial.rs"]
pub mod polynomial;
#[path = "math/sphere.rs"]
pub mod sphere_impl;
#[path = "math/types.rs"]
pub mod types;
#[path = "math/vector.rs"]
pub mod vector;

#[cfg(test)]
pub use camera_impl::camera;
pub use camera_impl::{CameraBasis, basis, camera_at};
#[cfg(test)]
pub use detail::pureweight;
#[cfg(test)]
pub use footprint_impl::footprint;
#[cfg(test)]
pub use goursat_impl::GoursatError;
pub use goursat_impl::goursat;
pub use plane_impl::plane;
#[cfg(test)]
pub use sphere_impl::sphere;
pub use sphere_impl::sphere_at;
#[cfg(test)]
pub use types::V;
pub use types::{Ray, Sample};

#[cfg(test)]
#[path = "math/tests.rs"]
mod tests;
