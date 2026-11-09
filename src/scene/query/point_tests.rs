use super::*;
use crate::geometry::point::{Request, Resolution};

#[test]
fn shared_instance_closest_queries_use_stable_ids_and_world_units()
-> Result<(), Box<dyn std::error::Error>> {
    let scene = Prepared::new(&[
        Object::new(9, Kind::Sphere, [0.0; 3], 1.0, 0.0)?,
        Object::new(1, Kind::Sphere, [4.0, 0.0, 0.0], 2.0, 0.0)?,
        Object::new(7, Kind::Plane, [0.0, -3.0, 0.0], 0.5, 0.0)?,
    ])?;
    for (id, point, expected) in [
        (1, [10.0, 0.0, 0.0], 4.0),
        (9, [10.0, 0.0, 0.0], 9.0),
        (7, [10.0, -7.0, 3.0], 4.0),
    ] {
        let result = closest_prepared(&scene, id, point, Request::default())?;
        assert!(result.distance.lower <= expected && expected <= result.distance.upper);
        assert_eq!(result.resolution, Resolution::Converged);
        assert_eq!(result.evaluations, 1);
    }
    assert_eq!(scene.geometries().len(), 2);
    assert_eq!(scene.instances().len(), 3);
    assert!(matches!(
        closest_prepared(&scene, 99, [0.0; 3], Request::default()),
        Err(QueryError::UnknownObject)
    ));
    assert!(matches!(
        closest_prepared(&scene, 1, [f64::NAN; 3], Request::default()),
        Err(QueryError::Geometry(GeometryError::InvalidPoint))
    ));
    Ok(())
}
