// Analytic plane intersection.
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
