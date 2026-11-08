//! Validated CPU scene model, camera, and analytic query facade.

/// Validated orbit camera parsing and ray construction.
pub mod camera;
/// Canonical analytic geometry contract, independent of instances.
pub mod geometry;
/// Validated scene object model and packet conversion.
pub mod model;
/// Shared geometry and instance storage ABI.
pub mod packed;
/// Deterministic analytic scene queries.
pub mod query;
/// Conservative finite-object hierarchy with explicit unbounded leaves.
pub mod spatial;

#[cfg(target_arch = "wasm32")]
pub use camera::{CameraError, parse_camera};
#[cfg(target_arch = "wasm32")]
pub use model::{Object, SceneError, parse_objects};
#[cfg(target_arch = "wasm32")]
pub use query::{QueryError, QueryMode, query_objects};
