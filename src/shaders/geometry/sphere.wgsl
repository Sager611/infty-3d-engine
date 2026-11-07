// Analytic sphere intersection and UV mapping.
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
