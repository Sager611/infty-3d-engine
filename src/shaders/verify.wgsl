// GPU verification entry point and present pass.
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
fn verify_goursat(@builtin(global_invocation_id) id: vec3u) {
  if (id.x >= arrayLength(&rays)) { return; }
  let ray = rays[id.x];
  let query = goursat(ray.o.xyz, ray.d.xyz, ray.o.w, ray.d.w != 0.0);
  let hit = select(0.0, 1.0, query.query.hit);
  results[id.x] = ProbeResult(
    vec4f(hit, query.query.t, query.query.gap, query.query.uv.x),
    vec4f(query.query.uv.y, query.query.p),
    vec4f(query.query.n, query.gap_lower),
    vec4f(query.error, f32(query.status), 0.0, 0.0)
  );
}

// Query24 packet, followed by eight zero padding floats for the shared harness.
@compute @workgroup_size(1)
fn verify_scene(@builtin(global_invocation_id) id: vec3u) {
  if (id.x >= arrayLength(&rays)) { return; }
  let ray = rays[id.x];
  let kind = u32(ray.o.w);
  let object = SceneObject(vec4u(7u, kind, 0u, 0u),
    vec4f(0.0, 1.0, 0.0, ray.d.w),
    vec4f(select(0.0, 1.5, kind == 2u), 0.0, 0.0, 0.0));
  let direction = normalize(ray.d.xyz);
  let sample = scene_object(ray.o.xyz, direction, object, true);
  let q = sample.query;
  let ray_point = ray.o.xyz + q.t * direction;
  results[2u * id.x] = ProbeResult(
    vec4f(f32(sample.id), f32(kind), select(0.0, 1.0, q.hit), q.t),
    vec4f(sample.gap_lower, q.gap, q.p.xy),
    vec4f(q.p.z, q.n),
    vec4f(q.uv, ray_point.xy)
  );
  results[2u * id.x + 1u] = ProbeResult(
    vec4f(ray_point.z, sample.local),
    vec4f(sample.error, select(0.0, 1.0, kind == 2u && !q.hit), f32(sample.status), 0.0),
    vec4f(0.0), vec4f(0.0)
  );
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

@compute @workgroup_size(1)
fn verify_outline(@builtin(global_invocation_id) id: vec3u) {
  if (id.x >= arrayLength(&rays)) { return; }
  let ray = rays[id.x];
  let q = Query(ray.d.x != 0.0, 2.0, ray.o.x, vec3f(0.0), vec3f(0.0, 1.0, 0.0), vec2f(0.0));
  let sample = SceneSample(q, vec3f(0.0), 7u, u32(ray.o.w), ray.o.y, ray.o.z, u32(ray.d.w));
  let eligible = outline_eligible(sample, ray.d.y != 0.0, ray.d.z);
  results[id.x] = ProbeResult(vec4f(select(0.0, 1.0, eligible), 0.0, 0.0, 0.0),
    vec4f(0.0), vec4f(0.0), vec4f(0.0));
}

@compute @workgroup_size(1)
fn verify_surface(@builtin(global_invocation_id) id: vec3u) {
  if (id.x >= arrayLength(&rays)) { return; }
  let ray = rays[id.x];
  let q = Query(true, 4.0, 0.0, vec3f(1.1, 0.0, 0.2),
    vec3f(0.0, 1.0, 0.0), vec2f(0.6, 0.2));
  let direction = vec3f(0.0, -1.0, 0.0);
  let rho = surface_rho(q, vec3f(0.6, 0.0, 0.2), direction, 100.0, ray.d.w, false);
  let color = surface_shade(q, vec3f(0.6, 0.0, 0.2), 3u, direction, 100.0,
    ray.d.w, u32(ray.o.w), q.p + vec3f(0.0, 1.0, 0.0), 1.0);
  results[id.x] = ProbeResult(vec4f(rho, color), vec4f(0.0), vec4f(0.0), vec4f(0.0));
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
