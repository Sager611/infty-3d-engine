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
  origin: vec4f,
  right: vec4f,
  up: vec4f,
  forward: vec4f,
  screen: vec4u,
  selection: vec4u,
};

// Kept only so the legacy verification material helpers remain source-compatible.
// The render adapter uses frame_params exclusively.
struct LegacyParams {
  view: vec4f,
  screen: vec4f,
};
const params: LegacyParams = LegacyParams(vec4f(0.0), vec4f(0.0));

@group(0) @binding(0) var pixels: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(1) var<uniform> frame_params: Params;
@group(0) @binding(2) var present: texture_2d<f32>;
