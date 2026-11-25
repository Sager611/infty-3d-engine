//! Validated CPU scene model, camera, and analytic query facade.

/// Validated orbit camera parsing and ray construction.
pub mod camera;
/// Validated scene object model and packet conversion.
pub mod model;
/// Shared geometry and instance storage ABI.
pub mod packed;
/// Immutable shared geometry, instances and hierarchy.
pub mod prepared;
/// Deterministic analytic scene queries.
pub mod query;
/// Canonical quaternion rotations and bounded vector maps.
pub mod rotation;
/// Conservative finite-object hierarchy with explicit unbounded leaves.
pub mod spatial;
/// Validated rigid/uniform transforms and coordinate mapping.
pub mod transform;

#[cfg(target_arch = "wasm32")]
pub use camera::{CameraError, parse_camera};
#[cfg(target_arch = "wasm32")]
pub use model::{SceneError, parse_objects};
#[cfg(target_arch = "wasm32")]
pub use query::{QueryError, QueryMode, query_objects};
