//! Validated CPU scene model, camera, and analytic query facade.

/// Validated orbit camera parsing and ray construction.
pub mod camera;
/// Validated scene object model and packet conversion.
pub mod model;
/// Deterministic analytic scene queries.
pub mod query;

#[cfg(target_arch = "wasm32")]
pub use camera::{CameraError, parse_camera};
#[cfg(target_arch = "wasm32")]
pub use model::{MAX_OBJECTS, Object, SceneError, parse_objects};
#[cfg(target_arch = "wasm32")]
pub use query::{QueryError, QueryMode, query_objects};
