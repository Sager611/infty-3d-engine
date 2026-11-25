// Pure material surface evaluation. Scene buffers stay in the render adapter.

fn surface_cell(uv: vec2f, scale: f32) -> f32 {
  let cell = floor(uv * scale);
  return select(0.0, 1.0, fract((cell.x + cell.y) * 0.5) >= 0.5);
}

fn surface_rho(q: Query, local: vec3f, ray: vec3f, height: f32, size: f32, sphere: bool) -> f32 {
  if (!q.hit || q.t <= 0.0 || height == 0.0 || size == 0.0) { return 1.0; }
  let nd = abs(dot(normalize(q.n), ray));
  if (nd == 0.0) { return 1.0; }
  var density = 1.0;
  if (sphere) {
    let radial = length(q.n.xz);
    if (radial == 0.0) { return 1.0; }
    density = max(1.0 / PI, 1.0 / (TAU * radial));
  }
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
  return select(n, -n, (kind == 4u || kind==5u) && dot(carrier, ray) > 0.0);
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
  let l = normalize(light - q.p);
  let diffuse = max(dot(n, l), 0.0);
  let sphere = kind == 1u;
  var base = vec3f(0.96862745, 0.9647059, 0.9490196);
  if (sphere) {
    base = surface_base(sphere_uv(n), 12.0,
      vec3f(0.31, 0.40, 0.48), vec3f(0.48, 0.55, 0.59));
  } else if (kind == 3u) {
    base = surface_base(q.uv, 1.25,
      vec3f(0.78, 0.81, 0.82), vec3f(0.93, 0.94, 0.93));
  } else if (kind == 2u) {
    base = mix(vec3f(0.37, 0.16, 0.18), vec3f(0.86, 0.28, 0.31),
      surface_cell(local.yz, 2.0));
  } else if (kind == 4u) {
    base = vec3f(0.28, 0.68, 0.67);
  }
  if (mode == 2u && kind != 4u) {
    let rho = surface_rho(q, local, ray, height, size, sphere);
    if (sphere) {
      base = mix(vec3f(0.31, 0.40, 0.48), vec3f(0.48, 0.55, 0.59),
        pureweight(sphere_uv(n), rho));
    } else if (kind == 2u) {
      base = mix(vec3f(0.37, 0.16, 0.18), vec3f(0.86, 0.28, 0.31),
        surface_triplanar(local, n, rho));
    } else {
      base = mix(vec3f(0.78, 0.81, 0.82), vec3f(0.93, 0.94, 0.93),
        pureweight(q.uv, rho));
    }
  }
  if (kind == 3u) {
    return base * (0.6 + 0.4 * diffuse * shadow)
      + vec3f(0.96862745, 0.9647059, 0.9490196) * 0.02;
  }
  return base * (0.18 + 0.82 * diffuse * shadow);
}
