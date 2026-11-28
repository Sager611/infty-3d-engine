// Pure material surface evaluation. Scene buffers stay in the render adapter.

fn surface_cell(uv: vec2f, scale: f32) -> f32 {
  let cell = floor(uv * scale);
  return select(0.0, 1.0, fract((cell.x + cell.y) * 0.5) >= 0.5);
}

// Display palette: paper background and the grey grid of flat surfaces.
const SURFACE_PAPER=vec3f(0.96862745,0.9647059,0.9490196);
const SURFACE_LOW=vec3f(0.78,0.81,0.82);
const SURFACE_HIGH=vec3f(0.93,0.94,0.93);

// Pixel footprint over a UV cell; density is UV cells per unit area along the surface (0: undefined).
fn surface_rho(q: Query, ray: vec3f, height: f32, size: f32, density: f32) -> f32 {
  if (!q.hit || q.t <= 0.0 || height == 0.0 || size == 0.0) { return 1.0; }
  let nd = abs(dot(normalize(q.n), ray));
  if (nd == 0.0 || density == 0.0) { return 1.0; }
  return min(1.0, 2.0 * q.t * density / (1.8 * height * size * nd));
}

fn surface_triplanar(local: vec3f, n: vec3f, rho: f32) -> f32 {
  let a = abs(normalize(n));
  let total = max(a.x + a.y + a.z, EPS);
  let weights = a / total;
  let yz = pureweight(local.yz, rho);
  let xz = pureweight(local.xz, rho);
  let xy = pureweight(local.xy, rho);
  return weights.x * yz + weights.y * xz + weights.z * xy;
}

fn surface_base(uv: vec2f, scale: f32, a: vec3f, b: vec3f) -> vec3f {
  return mix(a, b, surface_cell(uv, scale));
}

// A byte of height uncertainty must not become a large shading-slope jump.
fn surface_slope_step(step:f32,quantum:f32)->f32 {return max(step,16.0*abs(quantum));}

// Face the carrier, not individual bumps crossing the view tangent.
fn surface_normal(normal: vec3f, carrier: vec3f, ray: vec3f, kind: u32) -> vec3f {
  let n = normalize(normal);
  return select(n, -n, geometry_card(kind) && dot(carrier, ray) > 0.0);
}

fn surface_shade(
  q: Query,
  carrier: vec3f,
  local: vec3f,
  kind: u32,
  ray: vec3f,
  height: f32,
  size: f32,
  mode: u32,
  light: vec3f,
  shadow: f32
) -> vec3f {
  let n = surface_normal(q.n, carrier, ray, kind);
  let diffuse = max(dot(n, normalize(light - q.p)), 0.0);
  return geometry_shade(kind, q, n, local, ray, height, size, mode, diffuse, shadow);
}

fn surface_lit(base: vec3f, diffuse: f32, shadow: f32) -> vec3f {
  return base * (0.18 + 0.82 * diffuse * shadow);
}
