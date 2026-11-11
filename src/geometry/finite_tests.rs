#![expect(
    clippy::float_cmp,
    reason = "Exact carrier frames, packet and witness identities"
)]
use super::{
    Error, Geometry, Kind,
    point::{Multiplicity, Request, Resolution},
};
use crate::{
    material::Material,
    math::Ray,
    scene::{
        model::{Object, parse_objects},
        prepared::Prepared,
        query::{QueryMode, closest_prepared, query_objects, query_prepared},
    },
};

fn finite(axis: f64) -> Result<Geometry, Error> {
    Geometry::new(Kind::from_tag(5).ok_or(Error::InvalidParameter)?, axis)
}
fn unframe(axis: u32, p: [f64; 3]) -> [f64; 3] {
    match axis {
        1 => [p[2], p[0], p[1]],
        2 => [p[1], p[2], p[0]],
        _ => p,
    }
}
fn leaf_scene(material: f64) -> Result<Prepared, Box<dyn std::error::Error>> {
    Ok(Prepared::new(&parse_objects(&[
        1.0, 5.0, 0.0, 0.0, 0.0, 0.07, 0.0, material,
    ])?)?)
}

#[test]
fn finite_plane_axes_bounds_and_edges_are_explicit() -> Result<(), Box<dyn std::error::Error>> {
    assert!(Kind::from_tag(5).is_some(), "finite plane tag 5 is missing");
    for (axis, parameter) in [(0, 0.0), (1, 1.0), (2, 2.0)] {
        let geometry = finite(parameter)?;
        assert!(
            geometry
                .radius()
                .is_some_and(|r| r >= 2.0_f64.sqrt() && r < 2.0)
        );
        let ray = Ray {
            o: unframe(axis, [0.25, 0.5, 2.0]),
            d: unframe(axis, [0.0, 0.0, -1.0]),
        };
        let q = geometry.query(ray, true)?;
        assert!(q.sample.hit && !q.uncertain);
        assert!((q.sample.t - 2.0).abs() < 1e-13);
        assert_eq!(q.sample.p, unframe(axis, [0.25, 0.5, 0.0]));
        assert_eq!(q.sample.n, unframe(axis, [0.0, 0.0, 1.0]));
        assert_eq!(q.sample.uv, [0.625, 0.75]);
        let outside = geometry.query(
            Ray {
                o: unframe(axis, [1.000_001, 0.5, 2.0]),
                d: ray.d,
            },
            true,
        )?;
        assert!(!outside.sample.hit && outside.gap_computed);
    }
    let kind = Kind::from_tag(5).ok_or("missing finite plane")?;
    for parameter in [-1.0, 0.5, 3.0, f64::NAN] {
        assert!(Geometry::new(kind, parameter).is_err());
    }
    Ok(())
}

#[test]
fn finite_plane_is_double_sided_and_handles_coplanar_contacts() -> Result<(), Error> {
    let geometry = finite(0.0)?;
    for z in [-2.0_f64, 2.0] {
        let q = geometry.query(
            Ray {
                o: [1.0, 1.0, z],
                d: [0.0, 0.0, -z.signum()],
            },
            true,
        )?;
        assert!(q.sample.hit && !q.uncertain);
        assert!((q.sample.t - 2.0).abs() < 1e-13);
    }
    let q = geometry.query(
        Ray {
            o: [-2.0, 0.0, 0.0],
            d: [1.0, 0.0, 0.0],
        },
        true,
    )?;
    assert!(q.sample.hit && (q.sample.t - 1.0).abs() < 1e-13);
    assert_eq!(q.sample.p, [-1.0, 0.0, 0.0]);
    let q = geometry.query(
        Ray {
            o: [0.0; 3],
            d: [0.0, 1.0, 0.0],
        },
        true,
    )?;
    assert!(q.sample.hit && q.sample.t == 0.0);
    Ok(())
}

#[test]
fn finite_precision_boundaries_never_claim_a_resolved_false_hit() -> Result<(), Error> {
    let geometry = finite(0.0)?;
    let diagonal = std::f64::consts::FRAC_1_SQRT_2;
    for ray in [
        Ray {
            o: [1.0, 0.0, 1.0],
            d: [2.0_f64.powi(-54), 0.0, -1.0],
        },
        Ray {
            o: [-2.0, f64::EPSILON, 0.0],
            d: [diagonal, diagonal, 0.0],
        },
    ] {
        let q = geometry.query(ray, true)?;
        assert!(
            !q.sample.hit || q.uncertain,
            "rounding manufactured a certain finite-plane contact"
        );
    }
    Ok(())
}

#[test]
fn finite_plane_miss_gaps_use_real_rectangle_witnesses() -> Result<(), Error> {
    let geometry = finite(0.0)?;
    for (ray, gap) in [
        (
            Ray {
                o: [2.0, 0.0, 1.0],
                d: [0.0, 0.0, -1.0],
            },
            1.0,
        ),
        (
            Ray {
                o: [2.0, 0.0, 1.0],
                d: [0.0, 1.0, 0.0],
            },
            2.0_f64.sqrt(),
        ),
        (
            Ray {
                o: [0.0, 0.0, 1.0],
                d: [0.0, 0.0, 1.0],
            },
            1.0,
        ),
        (
            Ray {
                o: [-2.0, 0.0, 1.0],
                d: [1.0, 0.0, 0.0],
            },
            1.0,
        ),
        (
            Ray {
                o: [2.0, 2.0, 1.0],
                d: [0.0, 0.0, -1.0],
            },
            2.0_f64.sqrt(),
        ),
    ] {
        let q = geometry.query(ray, true)?;
        assert!(!q.sample.hit && q.gap_computed);
        assert!(q.sample.p[0].abs() <= 1.0 && q.sample.p[1].abs() <= 1.0);
        assert_eq!(q.sample.p[2], 0.0);
        assert!(q.gap_lower <= gap + 1e-12 && q.sample.gap >= gap - 1e-12);
        let delta = [0, 1, 2].map(|i| q.sample.p[i] - ray.d[i].mul_add(q.sample.t, ray.o[i]));
        let distance = delta[0].hypot(delta[1]).hypot(delta[2]);
        assert!(distance <= q.sample.gap + q.error + 1e-12);
    }
    Ok(())
}

#[test]
fn finite_plane_closest_is_a_unique_bounded_projection() -> Result<(), Error> {
    for (axis, parameter) in [(0, 0.0), (1, 1.0), (2, 2.0)] {
        let geometry = finite(parameter)?;
        let q = geometry.closest(unframe(axis, [2.0, 3.0, 4.0]), Request::default())?;
        assert_eq!(q.multiplicity, Multiplicity::Unique);
        assert_eq!(q.resolution, Resolution::Converged);
        assert!(q.evaluations <= 64);
        assert!(q.distance.lower <= 21.0_f64.sqrt() && q.distance.upper >= 21.0_f64.sqrt());
        let witness = q.witness.ok_or(Error::Numeric)?;
        assert_eq!(witness.point, unframe(axis, [1.0, 1.0, 0.0]));
        let no_work = geometry.closest(
            [0.0; 3],
            Request {
                max_evaluations: 0,
                ..Request::default()
            },
        )?;
        assert_eq!(no_work.resolution, Resolution::Unresolved);
        assert!(no_work.witness.is_none());
    }
    Ok(())
}

#[test]
fn leaf_collision_and_cutout_belong_to_the_instance_not_the_view()
-> Result<(), Box<dyn std::error::Error>> {
    let flat = leaf_scene(0.0)?;
    let leaf = leaf_scene(2.0)?;
    assert!(!flat.has_leaf() && leaf.has_leaf());
    let ray = Ray {
        o: [0.0, 0.0, 0.05],
        d: [0.0, 0.0, -1.0],
    };
    let a = query_prepared(&flat, ray, QueryMode::Inspect)?;
    let b = query_prepared(&leaf, ray, QueryMode::Inspect)?;
    assert!(a[0].sample.hit && b[0].sample.hit);
    assert!(
        (a[0].sample.t - b[0].sample.t).abs() > 1e-8,
        "leaf is only an appearance"
    );
    let outside = Ray {
        o: [-0.067, -0.067, 0.05],
        d: ray.d,
    };
    assert!(!query_prepared(&flat, outside, QueryMode::First)?.is_empty());
    assert!(
        query_prepared(&leaf, outside, QueryMode::First)?.is_empty(),
        "leaf cutout is a solid card"
    );
    let point = closest_prepared(
        &leaf,
        1,
        [0.0, 0.0, 0.1],
        Request {
            absolute: 1e-20,
            relative: 0.0,
            max_evaluations: 1,
        },
    )?;
    assert_eq!(point.resolution, Resolution::Unresolved);
    assert!(point.evaluations <= 1 && point.witness.is_some());
    let on_surface = closest_prepared(&leaf, 1, b[0].sample.p, Request::default())?;
    assert!(on_surface.distance.lower <= 1e-10 && on_surface.distance.upper < 1e-8);
    Ok(())
}

#[test]
fn leaf_finish_does_not_displace_existing_geometry() -> Result<(), Box<dyn std::error::Error>> {
    let leaf = Material::from_tag(2).ok_or("missing leaf material")?;
    let ray = Ray {
        o: [0.2, 0.3, 4.0],
        d: [0.0, 0.0, -1.0],
    };
    for tag in [1, 2, 3, 4] {
        let kind = Kind::from_tag(tag).ok_or("missing original geometry")?;
        let object = Object::new(1, kind, [0.0; 3], 1.0, if tag == 2 { 1.5 } else { 0.0 })?;
        let painted = Prepared::new(&[object.with_material(leaf)])?;
        assert!(painted.has_leaf_finish() && !painted.has_leaf());
        let a = query_objects(&[object], ray, QueryMode::Inspect)?;
        let b = query_objects(&[object.with_material(leaf)], ray, QueryMode::Inspect)?;
        assert_eq!(
            a[0].packed(ray).map(f64::to_bits),
            b[0].packed(ray).map(f64::to_bits)
        );
    }
    let prepared = Prepared::new(&parse_objects(&[
        1.0, 5.0, 0.0, 0.0, 0.0, 0.07, 0.0, 0.0, 2.0, 5.0, 0.5, 0.0, 0.0, 0.07, 0.0, 2.0,
    ])?)?;
    assert_eq!(prepared.geometries().len(), 1);
    assert_eq!(
        prepared.instances()[0].geometry(),
        prepared.instances()[1].geometry()
    );
    assert_ne!(
        prepared.instances()[0].material(),
        prepared.instances()[1].material()
    );
    Ok(())
}
