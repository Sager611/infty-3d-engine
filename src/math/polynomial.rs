use crate::diagnostic::{Recorder, Tag};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PolyError {
    NonFinite,
    ZeroLeading,
    BadInterval,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Roots {
    pub(super) values: [f64; 4],
    pub(super) len: usize,
    pub(super) error: f64,
    pub(super) uncertain: bool,
}

const EPS: f64 = f64::EPSILON;

pub(super) fn eval<C: AsRef<[f64]>>(coeff: C, x: f64) -> f64 {
    let coeff = coeff.as_ref();
    let mut value: f64 = 0.0;
    for &c in coeff {
        value = value.mul_add(x, c);
    }
    value
}

fn roundoff_bound(coeff: &[f64], x: f64) -> f64 {
    let mut scale: f64 = 0.0;
    let mut n = 0.0;
    let ax = x.abs();
    for &c in coeff {
        scale = scale.mul_add(ax, c.abs());
        n += 1.0;
        if !scale.is_finite() {
            return f64::INFINITY;
        }
    }
    let gamma = n * EPS / n.mul_add(-EPS, 1.0);
    gamma * scale
}

fn root_tolerance(a: f64, b: f64) -> f64 {
    32.0 * EPS * a.abs().max(b.abs()).max(1.0)
}

const fn empty_roots() -> Roots {
    Roots {
        values: [0.0; 4],
        len: 0,
        error: 0.0,
        uncertain: false,
    }
}

fn add_root(roots: &mut Roots, x: f64) {
    if !x.is_finite() {
        roots.uncertain = true;
        return;
    }
    for i in 0..roots.len {
        if (roots.values[i] - x).abs() <= root_tolerance(roots.values[i], x) {
            return;
        }
    }
    if roots.len == roots.values.len() {
        roots.uncertain = true;
        return;
    }
    let mut at = roots.len;
    while at != 0 && x < roots.values[at - 1] {
        roots.values[at] = roots.values[at - 1];
        at -= 1;
    }
    roots.values[at] = x;
    roots.len += 1;
}

fn sign_with_bound(value: f64, bound: f64) -> i8 {
    if value > bound {
        1
    } else if value < -bound {
        -1
    } else {
        0
    }
}

fn valid_coeff(coeff: &[f64]) -> Result<(), PolyError> {
    if coeff.iter().any(|x| !x.is_finite()) {
        Err(PolyError::NonFinite)
    } else if coeff[0] == 0.0 {
        Err(PolyError::ZeroLeading)
    } else {
        Ok(())
    }
}

pub(super) fn cubic(coeff: [f64; 4]) -> Result<Roots, PolyError> {
    valid_coeff(&coeff)?;
    let [a, b, c, d] = coeff;
    let aa = b / a;
    let bb = c / a;
    let cc = d / a;
    let p = aa.mul_add(-aa / 3.0, bb);
    let q = (2.0 * aa * aa * aa).mul_add(1.0 / 27.0, (-aa * bb).mul_add(1.0 / 3.0, cc));
    let half_q = q * 0.5;
    let third_p = p / 3.0;
    let discriminant = half_q.mul_add(half_q, third_p * third_p * third_p);
    let disc_scale = half_q.abs().mul_add(half_q.abs(), third_p.abs().powi(3));
    let disc_bound = 16.0 * EPS * disc_scale.max(f64::MIN_POSITIVE);
    if [aa, bb, cc, p, q, discriminant, disc_scale, disc_bound]
        .into_iter()
        .any(|x| !x.is_finite())
        || (disc_scale == 0.0 && (p != 0.0 || q != 0.0))
    {
        return Err(PolyError::NonFinite);
    }
    let mut roots = empty_roots();

    if discriminant > disc_bound {
        let radical = discriminant.sqrt();
        let u = (-half_q + radical).cbrt();
        let v = (-half_q - radical).cbrt();
        add_root(&mut roots, u + v - aa / 3.0);
    } else if discriminant < -disc_bound {
        let radius = 2.0 * (-third_p).sqrt();
        let denominator = (-third_p * third_p * third_p).sqrt();
        let mut argument = -half_q / denominator;
        if !(-1.0..=1.0).contains(&argument) {
            roots.uncertain = true;
            argument = argument.clamp(-1.0, 1.0);
        }
        let angle = argument.acos() / 3.0;
        let two_pi_thirds = 2.0 * std::f64::consts::PI / 3.0;
        for phase in [angle, angle - two_pi_thirds, angle + two_pi_thirds] {
            add_root(&mut roots, radius * phase.cos() - aa / 3.0);
        }
    } else if p == 0.0 && q == 0.0 {
        add_root(&mut roots, -aa / 3.0);
        roots.uncertain = discriminant != 0.0;
    } else {
        let u = (-half_q).cbrt();
        add_root(&mut roots, 2.0f64.mul_add(u, -(aa / 3.0)));
        add_root(&mut roots, -u - aa / 3.0);
        roots.uncertain = discriminant != 0.0;
    }
    Ok(roots)
}

fn bisect(coeff: &[f64], mut lo: f64, mut hi: f64, mut flo: f64, mut fhi: f64) -> (f64, f64) {
    for _ in 0..56 {
        let mid = (hi - lo).mul_add(0.5, lo);
        if mid <= lo || mid >= hi {
            break;
        }
        let fmid = eval(coeff, mid);
        if (flo.is_sign_negative() && fmid.is_sign_negative())
            || (flo.is_sign_positive() && fmid.is_sign_positive())
        {
            lo = mid;
            flo = fmid;
        } else {
            hi = mid;
            fhi = fmid;
        }
    }
    let root = (hi - lo).mul_add(0.5, lo);
    let _ = fhi;
    (root, (hi - lo) * 0.5)
}

#[cfg(test)]
pub(super) fn quartic(coeff: [f64; 5], lo: f64, hi: f64) -> Result<Roots, PolyError> {
    quartic_traced(coeff, lo, hi, &mut Recorder::off())
}

pub(super) fn quartic_traced(
    coeff: [f64; 5],
    lo: f64,
    hi: f64,
    trace: &mut Recorder<'_>,
) -> Result<Roots, PolyError> {
    valid_coeff(&coeff)?;
    if !lo.is_finite() || !hi.is_finite() {
        return Err(PolyError::NonFinite);
    }
    if lo > hi {
        return Err(PolyError::BadInterval);
    }

    let derivative = [4.0 * coeff[0], 3.0 * coeff[1], 2.0 * coeff[2], coeff[3]];
    let stationary = cubic(derivative)?;
    let mut points = [0.0; 6];
    let mut point_len = 2;
    points[0] = lo;
    points[1] = hi;
    let mut roots = empty_roots();
    roots.uncertain = stationary.uncertain;

    for &x in &stationary.values[..stationary.len] {
        if x > lo && x < hi {
            points[point_len] = x;
            point_len += 1;
        }
    }
    points[..point_len].sort_by(f64::total_cmp);

    for &x in &points[..point_len] {
        let value = eval(coeff, x);
        let bound = roundoff_bound(&coeff, x);
        if !value.is_finite() || !bound.is_finite() {
            return Err(PolyError::NonFinite);
        }
        if sign_with_bound(value, bound) == 0 {
            trace.emit(Tag::Root, [x, value, bound, 0.0, 0.0, 0.0]);
            add_root(&mut roots, x);
            roots.uncertain |= value.abs() > 0.5 * bound;
        }
    }

    for pair in points[..point_len].windows(2) {
        let a = pair[0];
        let b = pair[1];
        let fa = eval(coeff, a);
        let fb = eval(coeff, b);
        let sa = sign_with_bound(fa, roundoff_bound(&coeff, a));
        let sb = sign_with_bound(fb, roundoff_bound(&coeff, b));
        if sa != 0 && sb != 0 && sa != sb {
            let (root, bracket_error) = bisect(&coeff, a, b, fa, fb);
            trace.emit(Tag::Bracket, [a, b, root, bracket_error, fa, fb]);
            add_root(&mut roots, root);
            roots.error = roots.error.max(bracket_error);
            roots.uncertain |= bracket_error > root_tolerance(root, root);
        } else {
            trace.emit(
                Tag::Reject,
                [1.0, a, b, fa, fb, f64::from(sa) * f64::from(sb)],
            );
        }
    }
    Ok(roots)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_roots(actual: Roots, expected: &[f64]) {
        assert_eq!(actual.len, expected.len());
        for (actual, expected) in actual.values[..actual.len].iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= 2.0e-12,
                "{actual} != {expected}"
            );
        }
        assert!(actual.error.is_finite());
    }

    #[test]
    fn evaluates_horner_form() {
        assert_eq!(eval([1.0, -3.0, 2.0], 2.0).to_bits(), 0.0f64.to_bits());
    }

    #[test]
    fn four_simple_roots() -> Result<(), PolyError> {
        let roots = quartic([1.0, -10.0, 35.0, -50.0, 24.0], 0.0, 5.0)?;
        assert_roots(roots, &[1.0, 2.0, 3.0, 4.0]);
        assert!(roots.error <= 1.0e-15, "{}", roots.error);
        Ok(())
    }

    #[test]
    fn biquadratic_double_tangencies() -> Result<(), PolyError> {
        let roots = quartic([1.0, 0.0, -2.0, 0.0, 1.0], -2.0, 2.0)?;
        assert_roots(roots, &[-1.0, 1.0]);
        assert!(!roots.uncertain);
        Ok(())
    }

    #[test]
    fn positive_offset_is_a_near_miss() -> Result<(), PolyError> {
        let roots = quartic([1.0, 0.0, 2.0e-6, 0.0, 1.0e-12], -1.0, 1.0)?;
        assert_eq!(roots.len, 0);
        assert!(!roots.uncertain);
        Ok(())
    }

    #[test]
    fn no_real_roots() -> Result<(), PolyError> {
        assert_eq!(quartic([1.0, 0.0, 0.0, 0.0, 1.0], -2.0, 2.0)?.len, 0);
        Ok(())
    }

    #[test]
    fn interval_keeps_only_inside_roots() -> Result<(), PolyError> {
        assert_roots(quartic([1.0, -3.0, 2.0, 0.0, 0.0], 0.5, 1.5)?, &[1.0]);
        Ok(())
    }

    #[test]
    fn cubic_triple_root() -> Result<(), PolyError> {
        assert_roots(cubic([1.0, 0.0, 0.0, 0.0])?, &[0.0]);
        Ok(())
    }

    #[test]
    fn unrepresentable_finite_cubic_arithmetic_is_an_error() {
        assert_eq!(
            cubic([f64::MIN_POSITIVE, f64::MAX, 1.0, 1.0]),
            Err(PolyError::NonFinite)
        );
    }

    #[test]
    fn unrepresentable_finite_quartic_arithmetic_is_an_error() {
        assert_eq!(
            quartic([1.0, 0.0, 0.0, 0.0, 1.0], 0.0, f64::MAX),
            Err(PolyError::NonFinite)
        );
    }

    #[test]
    fn underflowed_nonzero_discriminants_are_not_exact_multiple_roots() {
        for coeff in [[1.0, 0.0, 0.0, 1e-300], [1.0, 0.0, -1e-300, 0.0]] {
            assert_eq!(cubic(coeff), Err(PolyError::NonFinite));
        }
    }

    #[test]
    fn invalid_input_is_explicit() {
        assert_eq!(cubic([0.0, 1.0, 2.0, 3.0]), Err(PolyError::ZeroLeading));
        assert_eq!(
            quartic([1.0, 0.0, 0.0, 0.0, 0.0], 2.0, 1.0),
            Err(PolyError::BadInterval)
        );
        assert_eq!(
            quartic([f64::NAN, 0.0, 0.0, 0.0, 0.0], 0.0, 1.0),
            Err(PolyError::NonFinite)
        );
    }
}
