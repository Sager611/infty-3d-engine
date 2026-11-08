// Pure analytic polynomial roots. Coefficients are in descending order.
const POLY_EPS: f32 = 0.000001;

struct PolyRoots {
  values: vec4f,
  count: u32,
  error: f32,
  uncertain: u32,
};

fn poly_finite(x: f32) -> bool {
  return abs(x) <= 3.402823466e38;
}

fn poly_eval(coeff: vec4f, x: f32) -> f32 {
  return ((coeff.x * x + coeff.y) * x + coeff.z) * x + coeff.w;
}

fn poly_cuberoot(x: f32) -> f32 {
  let magnitude = pow(abs(x), 1.0 / 3.0);
  return select(-magnitude, magnitude, x >= 0.0);
}

fn poly_position_allowance(x: f32) -> f32 {
  return 2.0 * 1.1920928955078125e-7 * max(1.0, abs(x));
}

fn poly_sort3(coeff: vec4f, a: f32, b: f32, c: f32, count: u32) -> PolyRoots {
  var x = a;
  var y = b;
  var z = c;
  if (x > y) { let t = x; x = y; y = t; }
  if (y > z) { let t = y; y = z; z = t; }
  if (x > y) { let t = x; x = y; y = t; }
  let values = vec4f(x, y, z, 0.0);
  var error = poly_position_allowance(x);
  if (count > 1u) { error = max(error, poly_position_allowance(y)); }
  if (count > 2u) { error = max(error, poly_position_allowance(z)); }
  return PolyRoots(values, count, error, 0u);
}

fn poly_cubic(coeff: vec4f) -> PolyRoots {
  if (!(poly_finite(coeff.x) && poly_finite(coeff.y) && poly_finite(coeff.z)
      && poly_finite(coeff.w)) || coeff.x == 0.0) {
    return PolyRoots(vec4f(0.0), 0u, 0.0, 2u);
  }

  let a = coeff.x;
  let b = coeff.y;
  let c = coeff.z;
  let d = coeff.w;
  let a2 = a * a;
  let p = (3.0 * a * c - b * b) / (3.0 * a2);
  let q = (27.0 * a2 * d - 9.0 * a * b * c + 2.0 * b * b * b)
    / (27.0 * a2 * a);
  let half_q = 0.5 * q;
  let third_p = p / 3.0;
  let discriminant = half_q * half_q + third_p * third_p * third_p;
  let discr_scale = max(1.0, abs(half_q * half_q)
    + abs(third_p * third_p * third_p));
  let discr_eps = 32.0 * POLY_EPS * discr_scale;
  let shift = b / (3.0 * a);

  if (discriminant > discr_eps) {
    let root = poly_cuberoot(-half_q + sqrt(discriminant))
      + poly_cuberoot(-half_q - sqrt(discriminant)) - shift;
    return poly_sort3(coeff, root, root, root, 1u);
  }

  if (discriminant < -discr_eps) {
    let radius = 2.0 * sqrt(max(-third_p, 0.0));
    let angle = acos(clamp((3.0 * q / (2.0 * p))
      * sqrt(-3.0 / p), -1.0, 1.0));
    let root0 = radius * cos(angle / 3.0) - shift;
    let root1 = radius * cos((angle + 2.0 * PI) / 3.0) - shift;
    let root2 = radius * cos((angle + 4.0 * PI) / 3.0) - shift;
    return poly_sort3(coeff, root0, root1, root2, 3u);
  }

  let root = poly_cuberoot(-half_q);
  let root0 = 2.0 * root - shift;
  let root1 = -root - shift;
  let distinct = select(1u, 2u, root0 != root1);
  return poly_sort3(coeff, root0, root1, root1, distinct);
}

fn poly_root_bound(coeff: array<f32, 5>, x: f32) -> f32 {
  let ax = abs(x);
  let scale = ((((abs(coeff[0]) * ax + abs(coeff[1])) * ax + abs(coeff[2])) * ax
    + abs(coeff[3])) * ax + abs(coeff[4]));
  return 8.0 * POLY_EPS * max(1.0, scale);
}

fn poly_root_tolerance(coeff: array<f32, 5>, x: f32) -> f32 {
  return 8.0 * poly_root_bound(coeff, x);
}

fn poly_duplicate(values: vec4f, count: u32, x: f32) -> bool {
  for (var i = 0u; i < 4u; i++) {
    if (i < count && abs(values[i] - x)
        <= 16.0 * POLY_EPS * max(1.0, max(abs(values[i]), abs(x)))) {
      return true;
    }
  }
  return false;
}

fn poly_quartic(coeff: array<f32, 5>, lo: f32, hi: f32) -> PolyRoots {
  if (!(poly_finite(coeff[0]) && poly_finite(coeff[1]) && poly_finite(coeff[2])
      && poly_finite(coeff[3]) && poly_finite(coeff[4]) && poly_finite(lo)
      && poly_finite(hi)) || coeff[0] == 0.0 || !(lo < hi)) {
    return PolyRoots(vec4f(0.0), 0u, 0.0, 2u);
  }

  var roots = array<f32, 4>(0.0, 0.0, 0.0, 0.0);
  var root_count = 0u;
  var error = 0.0;
  var uncertain = 0u;
  let derivative = poly_cubic(vec4f(4.0 * coeff[0], 3.0 * coeff[1],
    2.0 * coeff[2], coeff[3]));
  if (derivative.uncertain != 0u) { uncertain = 1u; }

  var critical = array<f32, 6>(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
  var critical_count = 1u;
  critical[0] = lo;
  for (var i = 0u; i < 3u; i++) {
    if (i < derivative.count) {
      let x = derivative.values[i];
      if (x > lo && x < hi && poly_finite(x)) {
        critical[critical_count] = x;
        critical_count++;
      }
    }
  }
  critical[critical_count] = hi;
  critical_count++;

  for (var i = 1u; i < 6u; i++) {
    if (i < critical_count) {
      var j = i;
      for (; j > 0u && critical[j] < critical[j - 1u]; j--) {
        let t = critical[j];
        critical[j] = critical[j - 1u];
        critical[j - 1u] = t;
      }
    }
  }

  for (var i = 0u; i + 1u < critical_count; i++) {
    let left = critical[i];
    let right = critical[i + 1u];
    let fl = (((coeff[0] * left + coeff[1]) * left + coeff[2]) * left
      + coeff[3]) * left + coeff[4];
    let fr = (((coeff[0] * right + coeff[1]) * right + coeff[2]) * right
      + coeff[3]) * right + coeff[4];

    // A stationary-point hit is a tangent candidate: the broad tolerance
    // accepts it, but only the roundoff bound makes it certain.
    if (root_count < 4u && abs(fl) <= poly_root_tolerance(coeff, left)
        && !poly_duplicate(vec4f(roots[0], roots[1], roots[2], roots[3]),
          root_count, left)) {
      if (abs(fl) > poly_root_bound(coeff, left)) { uncertain = 1u; }
      roots[root_count] = left;
      root_count++;
    }

    if ((fl < 0.0 && fr > 0.0) || (fl > 0.0 && fr < 0.0)) {
      var a = left;
      var b = right;
      var fa = fl;
      var midpoint = 0.5 * (a + b);
      var bracket_error = 0.5 * (b - a);
      var exact = false;
      for (var iteration = 0u; iteration < 32u; iteration++) {
        midpoint = 0.5 * (a + b);
        bracket_error = 0.5 * (b - a);
        if (midpoint == a || midpoint == b) {
          break;
        }
        let fm = (((coeff[0] * midpoint + coeff[1]) * midpoint + coeff[2])
          * midpoint + coeff[3]) * midpoint + coeff[4];
        if (fm == 0.0) {
          exact = true;
          bracket_error = poly_position_allowance(midpoint);
          break;
        }
        if ((fa < 0.0 && fm > 0.0) || (fa > 0.0 && fm < 0.0)) {
          b = midpoint;
        } else {
          a = midpoint;
          fa = fm;
        }
      }
      if (!exact) {
        midpoint = 0.5 * (a + b);
        bracket_error = 0.5 * (b - a);
        if (midpoint != a && midpoint != b) { uncertain = 1u; }
      }
      bracket_error = max(bracket_error, poly_position_allowance(midpoint));
      if (root_count < 4u && !poly_duplicate(
          vec4f(roots[0], roots[1], roots[2], roots[3]), root_count, midpoint)) {
        roots[root_count] = midpoint;
        root_count++;
      }
      // Keep the interval error, not the normalized Horner residual.
      error = max(error, bracket_error);
    }
  }

  let last = critical[critical_count - 1u];
  let flast = (((coeff[0] * last + coeff[1]) * last + coeff[2]) * last
    + coeff[3]) * last + coeff[4];
  if (abs(flast) <= poly_root_tolerance(coeff, last)
      && !poly_duplicate(vec4f(roots[0], roots[1], roots[2], roots[3]),
        root_count, last) && root_count < 4u) {
    if (abs(flast) > poly_root_bound(coeff, last)) { uncertain = 1u; }
    roots[root_count] = last;
    root_count++;
  }

  for (var i = 1u; i < 4u; i++) {
    if (i < root_count) {
      var j = i;
      for (; j > 0u && roots[j] < roots[j - 1u]; j--) {
        let t = roots[j];
        roots[j] = roots[j - 1u];
        roots[j - 1u] = t;
      }
    }
  }
  for (var i = 0u; i < 4u; i++) {
    if (i < root_count) {
      error = max(error, poly_position_allowance(roots[i]));
    }
  }
  return PolyRoots(vec4f(roots[0], roots[1], roots[2], roots[3]),
    root_count, error, uncertain);
}
