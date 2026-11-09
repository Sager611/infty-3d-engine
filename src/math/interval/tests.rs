use super::{ArithmeticError, Interval as I, norm};

fn contains(interval: I, value: f64) {
    assert!(
        interval.lo() <= value && value <= interval.hi(),
        "{interval:?} excludes {value}"
    );
}

#[test]
fn directed_operations_enclose_independent_dyadic_arithmetic() -> Result<(), ArithmeticError> {
    for numerator in -32..=32 {
        for other in -32..=32 {
            let a = I::point(f64::from(numerator) / 16.0)?;
            let b = I::point(f64::from(other) / 16.0)?;
            contains(a.add(b)?, f64::from(numerator + other) / 16.0);
            contains(a.sub(b)?, f64::from(numerator - other) / 16.0);
            contains(a.mul(b)?, f64::from(numerator * other) / 256.0);
            contains(a.square()?, f64::from(numerator * numerator) / 256.0);
        }
    }
    let a = I::new(-2.0, 3.0)?;
    let b = I::new(-4.0, 5.0)?;
    let product = a.mul(b)?;
    for endpoint in [-12.0, -10.0, 8.0, 15.0] {
        contains(product, endpoint);
    }
    Ok(())
}

#[test]
fn sqrt_two_enclosure_is_verified_with_exact_integer_squares() -> Result<(), ArithmeticError> {
    let root = I::point(2.0)?.sqrt()?;
    let numerator = |value: f64| u128::from(value.to_bits() & ((1_u64 << 52) - 1)) + (1_u128 << 52);
    let target = 2_u128 << 104;
    assert!(numerator(root.lo()).pow(2) <= target);
    assert!(numerator(root.hi()).pow(2) >= target);
    Ok(())
}

#[test]
fn midpoint_stays_inside_extreme_and_subnormal_intervals() -> Result<(), ArithmeticError> {
    let extremes = I::new(-f64::MAX, f64::MAX)?;
    assert_eq!(extremes.middle().to_bits(), 0.0_f64.to_bits());
    let tiny = f64::from_bits(1);
    assert_eq!(I::point(tiny)?.middle().to_bits(), tiny.to_bits());
    Ok(())
}

#[test]
fn scaling_subnormals_and_unrepresentable_arithmetic_are_explicit() -> Result<(), ArithmeticError> {
    contains(
        norm([I::point(1e200)?, I::point(-2e200)?, I::ZERO])?,
        5.0_f64.sqrt() * 1e200,
    );
    let tiny = f64::from_bits(1);
    contains(norm([I::point(tiny)?, I::ZERO, I::ZERO])?, tiny);
    let half = I::point(tiny)?.scale(0.5)?;
    assert!(half.lo() <= 0.0 && half.hi() >= tiny);
    assert!(I::point(f64::NAN).is_err());
    assert!(I::new(2.0, 1.0).is_err());
    assert!(I::point(-1.0)?.sqrt().is_err());
    assert!(I::point(f64::MAX)?.add(I::point(f64::MAX)?).is_err());
    assert!(norm([I::point(f64::MAX)?; 3]).is_err());
    Ok(())
}
