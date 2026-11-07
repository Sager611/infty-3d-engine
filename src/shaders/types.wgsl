// Shared scene types, constants, and bindings.
const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;
const EPS: f32 = 0.000001;

struct Query {
  hit: bool,
  t: f32,
  gap: f32,
  p: vec3f,
  n: vec3f,
  uv: vec2f,
};

struct Params {
  view: vec4f,
  screen: vec4f,
};

@group(0) @binding(0) var pixels: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var present: texture_2d<f32>;
