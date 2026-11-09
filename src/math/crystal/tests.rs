use super::*;
fn norm(p: V) -> f64 {
    p[0].hypot(p[1]).hypot(p[2])
}
fn distance_case(p: V, expected: f64) -> Result<(), ArithmeticError> {
    let result = closest(p)?;
    let slack = 1e-10 * (1.0 + expected);
    assert!(
        result.lower <= expected + slack && result.upper + slack >= expected,
        "{} <= {} <= {}",
        result.lower,
        expected,
        result.upper
    );
    assert!(result.upper - result.lower <= 1e-8 * (1.0 + expected));
    let actual = norm([p[0] - result.p[0], p[1] - result.p[1], p[2] - result.p[2]]);
    assert!(actual <= result.upper + result.error + slack);
    assert!(result.error.is_finite() && result.error >= 0.0);
    Ok(())
}
fn ray_case(o: V, d: V, expected: f64) -> Result<(), ArithmeticError> {
    let result = query(Ray { o, d }, true)?;
    assert!(result.sample.hit, "expected hit at {expected}, got miss");
    assert!((result.sample.t - expected).abs() <= 1e-8_f64.mul_add(1.0 + expected, result.error));
    assert!(result.sample.gap.abs() < 1e-14);
    assert!((norm(result.sample.n) - 1.0).abs() < 1e-10);
    assert!(result.sample.uv[0].is_finite() && result.sample.uv[1].is_finite());
    Ok(())
}
#[test]
fn known_surface_and_core() -> Result<(), ArithmeticError> {
    distance_case([0.75, 0.0, 1.5], 0.0)?;
    distance_case([0.5, 0.0, 0.25], 0.0)?;
    distance_case([0.125, 0.0, 0.25], 0.0)?;
    distance_case([0.0; 3], 0.0)?;
    distance_case([MIN_SCALE * 0.25, 0.0, MIN_SCALE * 0.125], 0.0)
}
#[test]
fn independent_global_closest_fixtures() -> Result<(), ArithmeticError> {
    distance_case([1.2, -0.4, 0.9], 0.155_111_990_521_134_25)?;
    distance_case([0.1, 0.2, -0.3], 0.050_232_957_414_927_31)?;
    distance_case([0.0, 0.0, 1.0], 0.450_693_909_432_998_64)?;
    distance_case([1.0, 0.0, 0.0], 0.450_693_909_432_998_64)?;
    distance_case([-3.0, -4.0, -5.0], 6.437_682_154_714_871)?;
    distance_case([0.3, 0.4, 0.5], 0.049_029_033_784_546_046)
}
#[test]
fn first_hit_and_ridge_tangency() -> Result<(), ArithmeticError> {
    ray_case([0.0, 0.0, 3.0], [0.0, 0.0, -1.0], 3.0)?;
    ray_case([0.75, -3.0, 1.5], [0.0, 1.0, 0.0], 3.0)?;
    ray_case([2.0, 0.1, 0.3], [-1.0, 0.0, 0.0], 1.499_900_009_998_000_7)?;
    ray_case([0.1, 0.2, 3.0], [0.0, 0.0, -1.0], 2.754_592_176_163_839_7)?;
    ray_case([0.4, -2.0, 0.6], [0.0, 1.0, 0.0], 1.110_829_037_811_063)
}
#[test]
fn surface_origins_and_apex() -> Result<(), ArithmeticError> {
    ray_case([0.75, 0.0, 1.5], [0.0, 0.0, -1.0], 0.0)?;
    ray_case([0.0; 3], [1.0, 0.0, 0.0], 0.0)
}
#[test]
fn miss_gap_and_shadow_status() -> Result<(), ArithmeticError> {
    let d = -1.0 / 3.0_f64.sqrt();
    let ray = Ray {
        o: [-3.0; 3],
        d: [d; 3],
    };
    let result = query(ray, true)?;
    let expected = closest(ray.o)?;
    assert!(!result.sample.hit && result.gap_computed);
    assert!(result.gap_lower <= expected.upper && result.sample.gap >= expected.lower);
    assert!(!query(ray, false)?.gap_computed);
    Ok(())
}
#[test]
fn extreme_closest_inputs_stay_explicit() -> Result<(), ArithmeticError> {
    let tiny = closest([1e-300, -2e-300, 3e-300])?;
    let huge = closest([1e200, -2e200, 1e200])?;
    assert!(tiny.upper.is_finite() && tiny.upper >= tiny.lower);
    assert!(huge.upper.is_finite() && huge.upper >= huge.lower);
    assert!(closest([f64::NAN, 0.0, 0.0]).is_err());
    Ok(())
}

#[test]
fn finite_apex_cap_and_lowest_bands_are_global() -> Result<(), ArithmeticError> {
    distance_case(
        [0.0, 0.0, MIN_SCALE / 8.0],
        MIN_SCALE / (8.0 * 5.0_f64.sqrt()),
    )?;
    distance_case([0.0, 0.0, MIN_SCALE], MIN_SCALE / 5.0_f64.sqrt())?;
    distance_case([0.0, 0.0, 2.0 * MIN_SCALE], MIN_SCALE)?;
    distance_case(
        [0.0, 0.0, 4.0 * MIN_SCALE],
        13.0_f64.sqrt() * MIN_SCALE / 2.0,
    )
}

#[test]
fn negative_roots_never_become_origin_hits() -> Result<(), ArithmeticError> {
    for scale in [MIN_SCALE, 1e-24, 1e-300] {
        let result = query(
            Ray {
                o: [0.0, 0.0, -scale],
                d: [0.0, 0.0, -1.0],
            },
            true,
        )?;
        assert!(
            !result.sample.hit,
            "negative root became a hit at scale {scale}"
        );
        assert!(result.gap_computed && result.sample.gap.is_finite());
    }
    Ok(())
}

#[test]
fn direct_kernels_have_no_iteration() {
    let check = |source: &str| {
        assert!(!source.contains("for "));
        assert!(!source.contains("while "));
        assert!(!source.contains("loop {"));
        assert!(!source.contains(".iter("));
        assert!(!source.contains(".into_iter("));
        assert!(!source.contains("from_fn("));
    };
    check(include_str!("../crystal.rs"));
    check(include_str!("point.rs"));
    check(include_str!("ray.rs"));
    check(include_str!("../../shaders/geometry/crystal.wgsl"));
}
