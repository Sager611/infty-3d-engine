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

fn sphere_uv(n: vec3f) -> vec2f {
  let horizontal = length(n.xz);
  let v = atan2(horizontal, n.y) / PI;
  if (horizontal <= 0.0) { return vec2f(0.0, v); }
  return vec2f(fract(atan2(n.z, n.x) / (2.0 * PI) + 0.5), v);
}

fn sphere(o: vec3f, d: vec3f, c: vec3f, r: f32) -> Query {
  let oc = o - c;
  let b = dot(oc, d);
  let cc = dot(oc, oc) - r * r;
  let disc = b * b - cc;
  var t = 1e30;
  if (disc >= 0.0) {
    let root = sqrt(disc);
    let q = -b - select(root, -root, b < 0.0);
    if (q != 0.0) {
      let t0 = q;
      let t1 = cc / q;
      if (t0 >= 0.0) { t = min(t, t0); }
      if (t1 >= 0.0) { t = min(t, t1); }
    } else if (-b >= 0.0) {
      t = -b;
    }
  }
  if (t < 1e29) {
    let p = o + t * d;
    let n = normalize(p - c);
    return Query(true, t, 0.0, p, n, sphere_uv(n));
  }

  let closest_t = max(0.0, -b);
  let closest = o + closest_t * d;
  let v = closest - c;
  let l = length(v);
  var n = vec3f(0.0, 1.0, 0.0);
  if (l > 0.0) { n = v / l; }
  let p = c + r * n;
  return Query(false, closest_t, max(l - r, 0.0), p, n, sphere_uv(n));
}

fn plane(o: vec3f, d: vec3f) -> Query {
  var t = 0.0;
  var hit = false;
  if (d.y != 0.0) {
    let candidate = -o.y / d.y;
    if (candidate >= 0.0) {
      t = candidate;
      hit = true;
    }
  } else {
    hit = o.y == 0.0;
  }
  let p = o + t * d;
  return Query(hit, t, select(abs(o.y), 0.0, hit), vec3f(p.x, 0.0, p.z),
    vec3f(0.0, 1.0, 0.0), vec2f(p.x, p.z));
}

fn tile(uv: vec2f, level: u32, rho: f32) -> f32 {
  let f = abs(uv) - floor(abs(uv));
  let scale_u = 1u << level;
  let scale = f32(scale_u);
  let s = f * scale;
  var cell = vec2u(floor(s));
  let p = s - floor(s);
  if (uv.x < 0.0) {
    cell.x = (0u - cell.x - select(0u, 1u, p.x > 0.0)) & (scale_u - 1u);
  }
  if (uv.y < 0.0) {
    cell.y = (0u - cell.y - select(0u, 1u, p.y > 0.0)) & (scale_u - 1u);
  }
  let edge = min(min(p.x, 1.0 - p.x), min(p.y, 1.0 - p.y));
  var hash = cell.x ^ (cell.y * 0x9e3779b9u) ^ (level * 0x85ebca6bu);
  hash ^= hash >> 16u;
  hash *= 0x7feb352du;
  hash ^= hash >> 15u;
  hash *= 0x846ca68bu;
  hash ^= hash >> 16u;
  let fill = 0.2 + 0.6 * f32(hash >> 24u) / 255.0;
  let inside = smoothstep(0.0, 0.04 + rho * scale, edge);
  let signal = mix(0.15, fill, inside);
  return mix(signal, 0.5, smoothstep(0.25, 1.0, rho * scale));
}

fn pureweight(uv: vec2f, rho: f32) -> f32 {
  let filtered_rho = clamp(rho, 0.0, 1.0);
  let lod = clamp(-log2(max(filtered_rho, 1e-12) * 32.0), 0.0, 18.0);
  let level = u32(floor(lod));
  let t = smoothstep(0.0, 1.0, fract(lod));
  let a = tile(uv, level, filtered_rho);
  let b = tile(uv, level + 1u, filtered_rho);
  let c = tile(uv, level + 2u, filtered_rho);
  return mix(0.6 * a + 0.4 * b, 0.6 * b + 0.4 * c, t);
}

fn footprint(q: Query, ray: vec3f, height: f32, radius: f32, is_sphere: bool) -> f32 {
  if (!q.hit || q.t <= 0.0 || height <= 0.0) { return 1.0; }
  let nd = abs(dot(q.n, ray));
  if (nd == 0.0) { return 1.0; }
  var density = 1.0;
  if (is_sphere) {
    let radial = length(q.n.xz);
    if (radial == 0.0) { return 1.0; }
    density = max(1.0 / PI, 1.0 / (TAU * radial)) / radius;
  }
  return min(1.0, 2.0 * q.t * density / (1.8 * height * nd));
}

fn checker(uv: vec2f, scale: f32, a: vec3f, b: vec3f) -> vec3f {
  let q = floor(uv * scale);
  return select(a, b, fract((q.x + q.y) * 0.5) >= 0.5);
}

fn shadow(p: vec3f, n: vec3f, light: vec3f) -> f32 {
  let to_light = light - p;
  let dist = length(to_light);
  let radius = params.view.w;
  let q = sphere(p + EPS * n, to_light / dist, vec3f(0.0, radius, 0.0), radius);
  return select(1.0, 0.25, q.hit && q.t < dist);
}

fn shade(
  q: Query,
  is_sphere: bool,
  light: vec3f,
  mode: f32,
  primary_ray: vec3f,
  height: f32,
  radius: f32
) -> vec3f {
  let sky = vec3f(0.96862745, 0.9647059, 0.9490196);
  let l = normalize(light - q.p);
  let diffuse = max(dot(q.n, l), 0.0);
  if (is_sphere) {
    var base = checker(q.uv, 12.0, vec3f(0.31, 0.40, 0.48), vec3f(0.48, 0.55, 0.59));
    if (mode == 2.0) {
      let rho = footprint(q, primary_ray, height, radius, true);
      base = mix(vec3f(0.31, 0.40, 0.48), vec3f(0.48, 0.55, 0.59), pureweight(q.uv, rho));
    }
    return base * (0.18 + 0.82 * diffuse);
  }
  var base = checker(q.uv, 1.25, vec3f(0.78, 0.81, 0.82), vec3f(0.93, 0.94, 0.93));
  if (mode == 2.0) {
    let rho = footprint(q, primary_ray, height, radius, false);
    base = mix(vec3f(0.78, 0.81, 0.82), vec3f(0.93, 0.94, 0.93), pureweight(q.uv, rho));
  }
  return base * (0.6 + 0.4 * diffuse * shadow(q.p, q.n, light)) + sky * 0.02;
}

fn camera_origin(focus: vec3f, yaw: f32, pitch: f32, distance: f32) -> vec3f {
  return focus + distance * vec3f(cos(pitch) * sin(yaw), sin(pitch), cos(pitch) * cos(yaw));
}

@compute @workgroup_size(8, 8)
fn draw(@builtin(global_invocation_id) id: vec3u) {
  let size = textureDimensions(pixels);
  if (id.x >= size.x || id.y >= size.y) { return; }

  let width = params.screen.x;
  let height = params.screen.y;
  let radius = params.view.w;
  let focus = vec3f(0.0, radius, 0.0);
  let o = camera_origin(focus, params.view.x, params.view.y, params.view.z);
  let f = normalize(focus - o);
  let right = normalize(cross(f, vec3f(0.0, 1.0, 0.0)));
  let up = cross(right, f);
  let pixel = vec2f(f32(id.x), f32(id.y));
  let sx = (2.0 * (pixel.x + 0.5) - width) / height;
  let sy = (height - 2.0 * (pixel.y + 0.5)) / height;
  let d = normalize(1.8 * f + sx * right + sy * up);

  let sq = sphere(o, d, vec3f(0.0, radius, 0.0), radius);
  let pq = plane(o, d);
  var q = sq;
  var is_sphere = true;
  if (!sq.hit || (pq.hit && pq.t < sq.t)) {
    q = pq;
    is_sphere = false;
  }

  let light = vec3f(3.0, 5.0, 4.0);
  var color = vec3f(0.96862745, 0.9647059, 0.9490196);
  if (q.hit) {
    color = shade(q, is_sphere, light, params.screen.z, d, params.screen.y, params.view.w);
  }
  if (params.screen.z == 1.0 && !sq.hit) {
    let halo = clamp(1.0 - sq.gap / 1.5, 0.0, 1.0);
    let uv_color = vec3f(sq.uv.x, sq.uv.y, 1.0 - sq.uv.x);
    color = mix(color, uv_color, halo * halo);
  }
  textureStore(pixels, vec2i(id.xy), vec4f(color, 1.0));
}

struct ProbeRay {
  o: vec4f,
  d: vec4f,
};

struct ProbeResult {
  a: vec4f,
  b: vec4f,
  c: vec4f,
  d: vec4f,
};

@group(0) @binding(3) var<storage, read> rays: array<ProbeRay>;
@group(0) @binding(4) var<storage, read_write> results: array<ProbeResult>;

fn pack(q: Query, rho: f32) -> ProbeResult {
  let h = select(0.0, 1.0, q.hit);
  return ProbeResult(
    vec4f(h, q.t, q.gap, q.uv.x),
    vec4f(q.uv.y, q.p),
    vec4f(q.n, pureweight(q.uv, rho)),
    vec4f(q.p, rho)
  );
}

fn result(q: Query, o: vec3f, d: vec3f, rho: f32) -> ProbeResult {
  var packed = pack(q, rho);
  packed.d = vec4f(o + q.t * d, rho);
  return packed;
}

@compute @workgroup_size(1)
fn verify(@builtin(global_invocation_id) id: vec3u) {
  if (id.x >= arrayLength(&rays)) { return; }
  let ray = rays[id.x];
  let o = ray.o.xyz;
  let d = ray.d.xyz;
  let height = ray.d.w;
  let sphere_q = sphere(o, d, vec3f(0.0, 1.0, 0.0), 1.0);
  let plane_q = plane(o, d);
  results[2u * id.x] = result(sphere_q, o, d, footprint(sphere_q, d, height, 1.0, true));
  results[2u * id.x + 1u] = result(plane_q, o, d, footprint(plane_q, d, height, 1.0, false));
}

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  var p = array<vec2f, 3>(
    vec2f(-1.0, -1.0),
    vec2f(3.0, -1.0),
    vec2f(-1.0, 3.0)
  );
  return vec4f(p[i], 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f {
  return textureLoad(present, vec2i(p.xy), 0);
}
