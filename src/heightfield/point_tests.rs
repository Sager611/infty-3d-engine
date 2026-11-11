use super::*;
use crate::geometry::point::{Request, Resolution};

#[test]
fn leaf_closest_budget_and_valid_witness_bounds_are_explicit() -> Result<(), Error> {
    let p = [0.0, 0.0, 1.0];
    let none = point::query(
        LEAF,
        p,
        Request {
            max_evaluations: 0,
            ..Request::default()
        },
    )?;
    assert!(none.witness.is_none() && none.evaluations == 0);
    let q = point::query(
        LEAF,
        p,
        Request {
            absolute: 1e-20,
            relative: 0.0,
            max_evaluations: 1,
        },
    )?;
    assert_eq!(q.resolution, Resolution::Unresolved);
    assert!(q.evaluations <= 1 && q.distance.lower > 0.9 && q.distance.lower <= q.distance.upper);
    let w = q.witness.ok_or(Error::Unresolved)?;
    let v = LEAF.sample([w.point[0], w.point[1]])?;
    assert!(v.coverage >= 127.5 && (v.height - w.point[2]).abs() <= w.error);
    let distance = (p[0] - w.point[0])
        .hypot(p[1] - w.point[1])
        .hypot(p[2] - w.point[2]);
    assert!(distance <= q.distance.upper + w.error);
    Ok(())
}

#[test]
fn surface_proximity_uses_continuous_witnesses_not_grid_vertex_snapping() -> Result<(), Error> {
    let xy = [0.012, -0.217];
    let height = LEAF.sample(xy)?.height;
    let p = [xy[0], xy[1], height];
    let q = point::query(LEAF, p, Request::default())?;
    assert_eq!(q.resolution, Resolution::Converged);
    assert!(q.distance.lower == 0.0 && q.distance.upper < 1e-12 && q.evaluations <= 64);
    assert_eq!(q.witness.ok_or(Error::Unresolved)?.point, p);
    Ok(())
}

#[test]
fn cutout_closest_bounds_never_use_the_removed_rectangle() -> Result<(), Error> {
    let pixels = data([[128; 4], [128; 4], [128; 4], [128; 4], [0, 255, 0, 255]]);
    let field = Field {
        amplitudes: [0.0; 4],
        ..fixture(&pixels)
    };
    let p = [-0.9, -0.3, 0.1];
    let expected = 0.4_f64.hypot(0.1);
    let q = point::query(field, p, Request::default())?;
    assert!(q.distance.lower <= expected && q.distance.upper >= expected);
    assert!(q.evaluations <= 64);
    let w = q.witness.ok_or(Error::Unresolved)?;
    assert!(w.point[0] >= -0.5 && field.sample([w.point[0], w.point[1]])?.coverage >= 127.5);
    Ok(())
}
