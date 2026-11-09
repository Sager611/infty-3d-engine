use super::{Closest, Multiplicity, Request, Resolution};
use crate::geometry::{Error, Geometry, Kind};

fn solve(point: [f64; 3], beta: f64, request: Request) -> Result<Closest, Error> {
    let result = Geometry::new(Kind::Goursat, beta)?.closest(point, request)?;
    assert!(result.evaluations <= request.max_evaluations);
    assert!(result.distance.lower.is_finite() && result.distance.lower >= 0.0);
    assert!(result.distance.upper.is_finite() && result.distance.upper >= result.distance.lower);
    let Some(witness) = result.witness else {
        return Err(Error::Numeric);
    };
    assert!(witness.point.iter().all(|x| x.is_finite()));
    assert!(witness.error.is_finite() && witness.error >= 0.0);
    let residual = witness
        .point
        .iter()
        .map(|x| x.mul_add(*x, -1.0).powi(2))
        .sum::<f64>()
        - beta;
    assert!(
        residual.abs() <= 32.0_f64.mul_add(witness.error, 64.0 * f64::EPSILON * (beta + 3.0)),
        "surface witness residual {residual}, error {}",
        witness.error
    );
    let distance = (witness.point[0] - point[0])
        .hypot(witness.point[1] - point[1])
        .hypot(witness.point[2] - point[2]);
    assert!(distance <= result.distance.upper + witness.error);
    Ok(result)
}

fn oracle(result: Closest, exact: f64, request: Request) {
    assert!(
        result.distance.lower <= exact && exact <= result.distance.upper,
        "global interval {:?} excludes {exact}",
        result.distance
    );
    assert_eq!(result.resolution, Resolution::Converged);
    assert!(
        result.distance.upper - result.distance.lower <= request.tolerance(result.distance.upper)
    );
}

#[test]
fn global_symmetric_oracles_preserve_distinct_minima() -> Result<(), Error> {
    let request = Request::default();
    for beta in [1.05_f64, 1.5, 1.95] {
        let center = solve([0.0; 3], beta, request)?;
        oracle(center, (3.0 - (3.0 * beta).sqrt()).sqrt(), request);
        assert_eq!(center.multiplicity, Multiplicity::NonUnique);
        let corner = solve([-1.0, 1.0, -1.0], beta, request)?;
        oracle(corner, (1.0 + beta.sqrt()).sqrt() - 1.0, request);
        assert_eq!(corner.multiplicity, Multiplicity::NonUnique);
        let hole = solve([0.0, -1.0, 0.0], beta, request)?;
        oracle(hole, (2.0 - (2.0 * beta).sqrt()).sqrt(), request);
        assert_eq!(hole.multiplicity, Multiplicity::NonUnique);
    }
    Ok(())
}

#[test]
fn global_interior_and_exterior_axis_oracles_do_not_snap_near_symmetry() -> Result<(), Error> {
    let request = Request::default();
    for beta in [1.05_f64, 1.5, 1.95] {
        let outer = (1.0 + beta.sqrt()).sqrt();
        for coordinate in [1.000_1, 1.1, 3.0] {
            let result = solve([1.0, -coordinate, 1.0], beta, request)?;
            oracle(result, (coordinate - outer).abs(), request);
            assert_ne!(result.multiplicity, Multiplicity::NonUnique);
            if coordinate.to_bits() == 3.0_f64.to_bits() {
                assert_eq!(result.multiplicity, Multiplicity::Unique);
            }
        }
    }
    Ok(())
}

#[test]
fn bounded_work_refines_global_bounds_without_fabricating_a_miss() -> Result<(), Error> {
    let point = [0.3, 0.9, 1.3];
    let mut lower = 0.0;
    let mut upper = f64::INFINITY;
    let mut last = Resolution::Unresolved;
    for budget in [1, 7, 31, 127, 511, 4095, 65_535] {
        let request = Request {
            absolute: 1e-8,
            relative: 0.0,
            max_evaluations: budget,
        };
        let result = solve(point, 1.5, request)?;
        assert!(
            result.distance.lower >= lower && result.distance.upper <= upper,
            "more work weakened the bounds"
        );
        if budget == 1 {
            assert_eq!(result.resolution, Resolution::Unresolved);
        }
        lower = result.distance.lower;
        upper = result.distance.upper;
        last = result.resolution;
    }
    assert_eq!(last, Resolution::Converged);
    Ok(())
}

#[test]
fn nonminimum_diagonal_stationary_point_does_not_hide_three_global_minima() -> Result<(), Error> {
    let near = 1.125_f64.mul_add(1.125, -1.0);
    let far = 1.5_f64.mul_add(1.5, -1.0);
    let beta = (2.0 * near).mul_add(near, far * far);
    let coordinate = 567.0 / 538.0;
    let request = Request::default();
    let result = solve([coordinate; 3], beta, request)?;
    let feasible = (1.5 - coordinate)
        .hypot(1.125 - coordinate)
        .hypot(1.125 - coordinate);
    let diagonal = ((1.0 + (beta / 3.0).sqrt()).sqrt() - coordinate).abs() * 3.0_f64.sqrt();
    assert_eq!(result.resolution, Resolution::Converged);
    assert!(result.distance.upper <= feasible + request.tolerance(result.distance.upper));
    assert!(result.distance.upper < diagonal);
    assert_eq!(result.multiplicity, Multiplicity::NonUnique);
    Ok(())
}

#[test]
fn exact_dyadic_surface_points_cover_inner_outer_and_zero_coordinate_charts() -> Result<(), Error> {
    for point in [
        [0.5_f64, 0.5, 1.0],
        [0.0, -1.25, 1.25],
        [-1.5, 1.125, 1.125],
    ] {
        let beta = point
            .into_iter()
            .map(|x| x.mul_add(x, -1.0).powi(2))
            .sum::<f64>();
        let request = Request::default();
        let result = solve(point, beta, request)?;
        oracle(result, 0.0, request);
        assert_ne!(result.multiplicity, Multiplicity::NonUnique);
    }
    Ok(())
}

#[test]
fn goursat_precision_limits_and_large_points_are_explicit() -> Result<(), Error> {
    let request = Request {
        absolute: 1e-30,
        relative: 0.0,
        max_evaluations: 127,
    };
    let limited = solve([0.3, 0.9, 1.3], 1.5, request)?;
    assert_eq!(limited.resolution, Resolution::Unresolved);
    let large = solve([1e200, -2e200, 3e200], 1.5, Request::default())?;
    assert_eq!(large.resolution, Resolution::Converged);
    let geometry = Geometry::new(Kind::Goursat, 1.5)?;
    assert!(matches!(
        geometry.closest([f64::MAX; 3], Request::default()),
        Err(Error::Numeric)
    ));
    Ok(())
}
