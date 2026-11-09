use super::*;
use crate::geometry::{
    Kind,
    point::{Multiplicity, Resolution},
};

#[test]
fn closest_points_apply_translation_uniform_distance_and_error_once()
-> Result<(), Box<dyn std::error::Error>> {
    let transform = Transform::new([2.0, -3.0, 4.0], 2.0)?;
    for (kind, parameter, point, expected, witness, multiplicity) in [
        (
            Kind::Sphere,
            0.0,
            [8.0, -3.0, 4.0],
            4.0,
            Some([4.0, -3.0, 4.0]),
            Multiplicity::Unique,
        ),
        (
            Kind::Plane,
            0.0,
            [30.0, -5.0, 8.0],
            2.0,
            Some([30.0, -3.0, 8.0]),
            Multiplicity::Unique,
        ),
        (
            Kind::Goursat,
            1.5,
            [2.0, -3.0, 4.0],
            2.0 * (3.0 - 4.5_f64.sqrt()).sqrt(),
            None,
            Multiplicity::NonUnique,
        ),
    ] {
        let result =
            transform.closest(Geometry::new(kind, parameter)?, point, Request::default())?;
        assert!(result.distance.lower <= expected && expected <= result.distance.upper);
        assert_eq!(result.resolution, Resolution::Converged);
        assert_eq!(result.multiplicity, multiplicity);
        let actual = result.witness.ok_or(Error::Numeric)?;
        if let Some(expected) = witness {
            let difference = (expected[0] - actual.point[0])
                .hypot(expected[1] - actual.point[1])
                .hypot(expected[2] - actual.point[2]);
            assert!(difference <= actual.error);
        }
    }
    Ok(())
}

#[test]
fn world_accuracy_and_surface_position_rounding_remain_separate()
-> Result<(), Box<dyn std::error::Error>> {
    let geometry = Geometry::new(Kind::Goursat, 1.5)?;
    for scale in [0.01, 1.0, 4.0] {
        let transform = Transform::new([2.0, -3.0, 4.0], scale)?;
        let point = transform.world_point([0.3, 0.9, 1.3]);
        let request = Request {
            absolute: 1e-8,
            relative: 0.0,
            ..Request::default()
        };
        let result = transform.closest(geometry, point, request)?;
        assert_eq!(result.resolution, Resolution::Converged);
        assert!(result.distance.upper - result.distance.lower <= request.absolute);
    }
    let transform = Transform::new([20.0, -20.0, 20.0], 0.01)?;
    let request = Request {
        absolute: 1e-16,
        relative: 0.0,
        ..Request::default()
    };
    let result = transform.closest(geometry, transform.position(), request)?;
    assert_eq!(result.resolution, Resolution::Converged);
    assert!(result.witness.ok_or(Error::Numeric)?.error > request.absolute);
    Ok(())
}

#[test]
fn input_rounding_does_not_manufacture_symmetric_multiplicity()
-> Result<(), Box<dyn std::error::Error>> {
    let transform = Transform::new([0.1, 0.0, 0.0], 3.0)?;
    let point = [3.1, 3.0, 3.0];
    assert_eq!(
        transform.local_point(point).map(f64::to_bits),
        [1.0_f64.to_bits(); 3]
    );
    let geometry = Geometry::new(Kind::Goursat, 1.5)?;
    let result = transform.closest(geometry, point, Request::default())?;
    assert_ne!(result.multiplicity, Multiplicity::NonUnique);
    Ok(())
}

#[test]
fn transformed_zero_work_invalid_inputs_and_numeric_limits_are_explicit()
-> Result<(), Box<dyn std::error::Error>> {
    let geometry = Geometry::SPHERE;
    let transform = Transform::new([0.0; 3], 4.0)?;
    let zero = transform.closest(
        geometry,
        [f64::MAX; 3],
        Request {
            max_evaluations: 0,
            ..Request::default()
        },
    )?;
    assert!(zero.witness.is_none());
    assert_eq!(zero.evaluations, 0);
    assert_eq!(zero.resolution, Resolution::Unresolved);
    assert!(matches!(
        transform.closest(geometry, [f64::NAN; 3], Request::default()),
        Err(Error::InvalidPoint)
    ));
    assert!(matches!(
        transform.closest(geometry, [f64::from_bits(1), 0.0, 0.0], Request::default()),
        Err(Error::Numeric)
    ));
    let small = Transform::new([0.0; 3], 0.01)?;
    assert!(matches!(
        small.closest(geometry, [f64::MAX; 3], Request::default()),
        Err(Error::Numeric)
    ));
    Ok(())
}
