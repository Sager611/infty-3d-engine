#![doc = "Infty 3D engine."]

#[cfg(any(test, target_arch = "wasm32"))]
mod math;

#[cfg(target_arch = "wasm32")]
mod gpu;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod gpu_test;

#[cfg(target_arch = "wasm32")]
pub use gpu::Renderer;
