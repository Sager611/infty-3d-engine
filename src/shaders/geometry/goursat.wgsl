// Analytic centered-quartic intersection and bounded surface witness.
alias GoursatQuery = GeometryQuery;

struct GoursatWitness {
  p: vec3f,
  upper: f32,
  error: f32,
  valid: bool,
  uncertain: u32,
};

fn goursat_value(p: vec3f, beta: f32) -> f32 {
  let q = p * p - vec3f(1.0);
  return dot(q, q) - beta;
}

fn goursat_gradient(p: vec3f) -> vec3f {
  return 4.0 * p * (p * p - vec3f(1.0));
}

fn goursat_well(p: vec3f) -> vec3f {
  return vec3f(
    select(1.0, -1.0, p.x < 0.0),
    select(1.0, -1.0, p.y < 0.0),
    select(1.0, -1.0, p.z < 0.0)
  );
}

fn goursat_uv(p: vec3f, n: vec3f) -> vec2f {
  let a = abs(n);
  if (a.x >= a.y && a.x >= a.z) {
    return vec2f(p.y, p.z);
  }
  if (a.y >= a.z) {
    return vec2f(p.x, p.z);
  }
  return vec2f(p.x, p.y);
}

fn goursat_value_error(p: vec3f, beta: f32) -> f32 {
  let q = p * p + vec3f(1.0);
  let gamma = 16.0 * 1.1920928955078125e-7;
  return gamma / (1.0 - gamma) * (dot(q, q) + abs(beta));
}

fn goursat_ray_distance(p: vec3f, o: vec3f, d: vec3f) -> f32 {
  let dd = dot(d, d);
  let t = max(0.0, dot(p - o, d) / dd);
  return length(p - (o + t * d));
}

fn goursat_witness(o: vec3f, d: vec3f, candidate: vec3f, beta: f32) -> GoursatWitness {
  let invalid = GoursatWitness(candidate, 1e30, 1e30, false, 1u);
  let fc = goursat_value(candidate, beta);
  if (abs(fc) <= goursat_value_error(candidate, beta)) { return invalid; }
  var inside = goursat_well(candidate);
  var outside = candidate;
  if (fc < 0.0) {
    inside = candidate;
    outside = vec3f(0.0);
  }
  if (goursat_value(inside, beta) >= -goursat_value_error(inside, beta) ||
      goursat_value(outside, beta) <= goursat_value_error(outside, beta)) { return invalid; }
  var uncertain = 0u;
  for (var iteration = 0u; iteration < 32u; iteration++) {
    let mid = 0.5 * (inside + outside);
    let fm = goursat_value(mid, beta);
    if (abs(fm) <= goursat_value_error(mid, beta) ||
        all(mid == inside) || all(mid == outside)) {
      uncertain = 1u;
      break;
    }
    if (fm < 0.0) { inside = mid; } else { outside = mid; }
  }
  let mid = 0.5 * (inside + outside);
  let projection = max(0.0, dot(mid - o, d) / dot(d, d));
  let error = 0.5 * length(outside - inside) +
    8.0 * 1.1920928955078125e-7 * (1.0 + length(inside) + length(outside)) +
    16.0 * 1.1920928955078125e-7 * (1.0 + length(mid) + length(o) + projection * length(d));
  let upper = (goursat_ray_distance(mid, o, d) + error) * (1.0 + 4.0 * 1.1920928955078125e-7);
  return GoursatWitness(mid, upper, error, true, uncertain);
}

fn goursat(o: vec3f, d: vec3f, beta: f32, need_gap: bool) -> GoursatQuery {
  let dd = dot(d, d);
  let t0 = -dot(o, d) / dd;
  let p0 = fma(vec3f(t0), d, o);
  let radius = sqrt(3.0 + sqrt(3.0 * beta));
  // Roundoff padding belongs to the search enclosure, not the surface polynomial.
  let slack = 16.0 * 1.1920928955078125e-7 * (1.0 + length(o) + abs(t0) * length(d) + radius);
  let enclosure = radius + slack;
  let reach = sqrt(max(enclosure * enclosure - dot(p0, p0), 0.0) / dd);
  let lo = max(-reach, -t0);
  let hi = reach;

  let x2 = p0 * p0;
  let d2 = d * d;
  let coeff = array<f32, 5>(
    dot(d2 * d2, vec3f(1.0)),
    4.0 * dot(p0 * d * d * d, vec3f(1.0)),
    dot(d2 * (6.0 * x2 - vec3f(2.0)), vec3f(1.0)),
    4.0 * dot(p0 * d * (x2 - vec3f(1.0)), vec3f(1.0)),
    goursat_value(p0, beta)
  );

  var roots = PolyRoots(vec4f(0.0), 0u, 0.0, 0u);
  if (lo <= hi) {
    roots = poly_quartic(coeff, lo, hi);
  }

  var query = Query(false, max(0.0, t0), 0.0, o, vec3f(0.0, 1.0, 0.0), vec2f(0.0));
  var status = roots.uncertain;
  if (roots.count > 0u) {
    let t = t0 + roots.values[0];
    let p = o + t * d;
    let gradient = goursat_gradient(p);
    let n = select(vec3f(0.0, 1.0, 0.0), normalize(gradient), dot(gradient, gradient) > 0.0);
    query = Query(true, t, 0.0, p, n, goursat_uv(p, n));
    return GoursatQuery(query, 0.0,
      roots.error + poly_position_allowance(t), status);
  }

  let forward = o + max(0.0, t0) * d;
  let ray_gap = max(length(forward) - radius - slack, 0.0);
  if (!need_gap) {
    query.gap = ray_gap;
    return GoursatQuery(query, ray_gap, 1e30, 2u);
  }
  var best = GoursatWitness(o, 1e30, 1e30, false, 1u);
  let origin_witness = goursat_witness(o, d, o, beta);
  if (origin_witness.valid && origin_witness.upper < best.upper) { best = origin_witness; }

  let derivative = poly_cubic(vec4f(4.0 * coeff[0], 3.0 * coeff[1],
    2.0 * coeff[2], coeff[3]));
  if (derivative.uncertain != 0u) { status = 1u; }
  for (var i = 0u; i < 3u; i++) {
    if (i < derivative.count) {
      let s = derivative.values[i];
      let t = t0 + s;
      if (t >= 0.0) {
        let witness = goursat_witness(o, d, o + t * d, beta);
        if (witness.valid && (!best.valid ||
            witness.upper < best.upper - (witness.error + best.error))) { best = witness; }
        if (witness.uncertain != 0u) { status = 1u; }
      }
    }
  }

  if (!best.valid) {
    query.gap = ray_gap;
    return GoursatQuery(query, ray_gap, 1e30, 3u);
  }
  query.p = best.p;
  let gradient = goursat_gradient(best.p);
  query.n = select(vec3f(0.0, 1.0, 0.0), normalize(gradient), dot(gradient, gradient) > 0.0);
  query.uv = goursat_uv(best.p, query.n);
  query.t = max(0.0, dot(best.p - o, d) / dd);
  query.gap = best.upper;
  return GoursatQuery(query, ray_gap, best.error, max(status, best.uncertain));
}
