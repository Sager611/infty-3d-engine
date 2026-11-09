#![doc = "Infty 3D engine."]

#[cfg(any(test, target_arch = "wasm32"))]
mod device_state;
#[cfg(any(test, target_arch = "wasm32"))]
mod diagnostic;
#[cfg(any(test, target_arch = "wasm32"))]
mod geometry;
#[cfg(any(test, target_arch = "wasm32"))]
mod material;
#[cfg(any(test, target_arch = "wasm32"))]
mod math;
#[cfg(any(test, target_arch = "wasm32"))]
mod scene;
#[cfg(any(test, target_arch = "wasm32"))]
mod shader;
#[cfg(any(test, target_arch = "wasm32"))]
mod upload;

#[cfg(target_arch = "wasm32")]
mod api;

#[cfg(target_arch = "wasm32")]
mod gpu;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod gpu_test;

#[cfg(target_arch = "wasm32")]
pub use api::{PreparedScene, closest_point, query_ray, query_scene, trace_closest_point};
#[cfg(target_arch = "wasm32")]
pub use gpu::Renderer;
