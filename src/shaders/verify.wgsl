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
