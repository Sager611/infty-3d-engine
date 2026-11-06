const PI: f32 = 3.14159265359;
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
  let y = clamp(n.y, -1.0, 1.0);
  if (length(vec2f(n.x, n.z)) <= 0.0) { return vec2f(0.0, acos(y) / PI); }
  return vec2f(fract(atan2(n.z, n.x) / (2.0 * PI) + 0.5), acos(y) / PI);
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

fn shade(q: Query, is_sphere: bool, light: vec3f) -> vec3f {
  let sky = vec3f(0.93, 0.89, 0.82);
  let l = normalize(light - q.p);
  let diffuse = max(dot(q.n, l), 0.0);
  if (is_sphere) {
    let base = checker(q.uv, 12.0, vec3f(0.31, 0.40, 0.48), vec3f(0.48, 0.55, 0.59));
    return base * (0.18 + 0.82 * diffuse);
  }
  let base = checker(q.p.xz, 1.25, vec3f(0.91, 0.84, 0.72), vec3f(0.65, 0.55, 0.43));
  return base * (0.28 + 0.72 * diffuse * shadow(q.p, q.n, light)) + sky * 0.02;
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
  var color = vec3f(0.93, 0.89, 0.82);
  if (q.hit) {
    color = shade(q, is_sphere, light);
  }
  if (params.screen.z > 0.5 && !sq.hit) {
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

fn result(q: Query, o: vec3f, d: vec3f) -> ProbeResult {
  let h = select(0.0, 1.0, q.hit);
  return ProbeResult(
    vec4f(h, q.t, q.gap, q.uv.x),
    vec4f(q.uv.y, q.p),
    vec4f(q.n, 0.0),
    vec4f(o + q.t * d, 0.0)
  );
}

@compute @workgroup_size(1)
fn verify(@builtin(global_invocation_id) id: vec3u) {
  if (id.x >= arrayLength(&rays)) { return; }
  let ray = rays[id.x];
  let o = ray.o.xyz;
  let d = ray.d.xyz;
  results[2u * id.x] = result(sphere(o, d, vec3f(0.0, 1.0, 0.0), 1.0), o, d);
  results[2u * id.x + 1u] = result(plane(o, d), o, d);
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
