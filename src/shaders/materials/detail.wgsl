// Material filters, shading, and analytic screen footprints.
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
