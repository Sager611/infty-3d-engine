#![doc = "Infty 3D engine."]

#[cfg(any(test, target_arch = "wasm32"))]
mod geometry;
#[cfg(any(test, target_arch = "wasm32"))]
mod math;
#[cfg(any(test, target_arch = "wasm32"))]
mod scene;
#[cfg(any(test, target_arch = "wasm32"))]
mod shader;

#[cfg(target_arch = "wasm32")]
mod api;

#[cfg(target_arch = "wasm32")]
mod gpu;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod gpu_test;

#[cfg(target_arch = "wasm32")]
pub use api::{closest_point, query_ray, query_scene};
#[cfg(target_arch = "wasm32")]
pub use gpu::Renderer;
