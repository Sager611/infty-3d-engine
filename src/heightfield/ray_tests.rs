use super::*;
use crate::math::Ray;

#[test]
fn a_finest_piece_combines_all_heights_and_mask_before_intersection() -> Result<(), Error> {
    let pixels = data([
        [10, 90, 170, 250],
        [200, 20, 240, 80],
        [40, 230, 70, 190],
        [190, 10, 210, 110],
        [255; 4],
    ]);
    let field = fixture(&pixels);
    let ij = [731, 1537];
    let piece = field.piece(ij)?;
    assert_eq!(piece.ij, ij);
    assert_eq!(piece.resolution, 2000);
    for i in 0..4 {
        let x = f64::from(i % 2) * 0.6 + 0.2;
        let y = f64::from(i / 2) * 0.6 + 0.2;
        let xy = [
            (f64::from(ij[0]) + x) * 2.0 / 2000.0 - 1.0,
            (f64::from(ij[1]) + y) * 2.0 / 2000.0 - 1.0,
        ];
        let value = field.sample(xy)?;
        let h = piece.height[0]
            .add(piece.height[1].scale(x)?)?
            .add(piece.height[2].scale(y)?)?
            .add(piece.height[3].scale(x * y)?)?;
        assert!(h.lo() - 1e-15 <= value.height && h.hi() + 1e-15 >= value.height);
        let m = piece.mask[0]
            .add(piece.mask[1].scale(x)?)?
            .add(piece.mask[2].scale(y)?)?
            .add(piece.mask[3].scale(x * y)?)?;
        assert!(m.lo() <= 255.0 && m.hi() >= 255.0);
    }
    Ok(())
}

#[test]
fn perpendicular_rays_hit_one_fixed_summed_surface_from_both_sides() -> Result<(), Error> {
    let xy = [0.012, -0.217];
    let value = LEAF.sample(xy)?;
    assert!(value.coverage >= 127.5);
    for sign in [-1.0, 1.0] {
        let ray = Ray {
            o: [xy[0], xy[1], value.height + sign],
            d: [0.0, 0.0, -sign],
        };
        let q = ray::query(LEAF, ray, 64)?;
        assert!(q.complete && !q.uncertain && q.visits == 1 && q.pending_lower.is_none());
        let hit = q.contact.ok_or(Error::Unresolved)?;
        assert!(hit.time.lo() <= 1.0 && hit.time.hi() >= 1.0);
        assert!((hit.p[2] - value.height).abs() <= hit.error);
        assert_eq!(hit.normal, value.normal());
    }
    let miss = ray::query(
        LEAF,
        Ray {
            o: [-0.96, -0.96, 1.0],
            d: [0.0, 0.0, -1.0],
        },
        64,
    )?;
    assert!(miss.complete && miss.contact.is_none());
    Ok(())
}

#[test]
fn analytic_cells_choose_the_first_of_two_roots_after_independent_masking() -> Result<(), Error> {
    let diagonal = std::f64::consts::FRAC_1_SQRT_2;
    for (mask, expected) in [
        ([255; 4], 0.25 / diagonal),
        ([0, 255, 0, 255], 0.75 / diagonal),
    ] {
        let pixels = data([[128, 128, 128, 192], [128; 4], [128; 4], [128; 4], mask]);
        let field = Field {
            amplitudes: [1.0, 0.0, 0.0, 0.0],
            ..fixture(&pixels)
        };
        let q = ray::query(
            field,
            Ray {
                o: [-1.0, 0.0, 64.0 / 255.0 * 3.0 / 16.0],
                d: [diagonal, -diagonal, 0.0],
            },
            64,
        )?;
        assert!(
            q.complete && !q.uncertain,
            "first root is not certified within 64 visits"
        );
        let hit = q.contact.ok_or(Error::Unresolved)?;
        assert!(hit.time.lo() <= expected + 1e-12 && hit.time.hi() >= expected - 1e-12);
        assert!((hit.time.middle() - expected).abs() < 1e-10);
        assert!(field.sample([hit.p[0], hit.p[1]])?.coverage >= 127.5);
    }
    Ok(())
}

#[test]
fn coplanar_cutout_entry_is_an_analytic_mask_root_not_a_solid_card() -> Result<(), Error> {
    let pixels = data([[128; 4], [128; 4], [128; 4], [128; 4], [0, 255, 0, 255]]);
    let field = Field {
        amplitudes: [0.0; 4],
        ..fixture(&pixels)
    };
    let q = ray::query(
        field,
        Ray {
            o: [-1.0, -0.5, 0.0],
            d: [1.0, 0.0, 0.0],
        },
        64,
    )?;
    assert!(q.complete && !q.uncertain);
    let hit = q.contact.ok_or(Error::Unresolved)?;
    assert!((hit.time.middle() - 0.5).abs() < 1e-10);
    assert!(hit.p[2] == 0.0 && hit.p[0] >= -0.5 - 1e-12);
    Ok(())
}

#[test]
fn an_oblique_leaf_ray_resolves_without_spending_visits_on_empty_xy_children() -> Result<(), Error>
{
    let q = ray::query(
        LEAF,
        Ray {
            o: [0.0, 0.0, 0.1],
            d: [0.3, 0.4, -0.75_f64.sqrt()],
        },
        64,
    )?;
    assert!(
        q.complete && q.contact.is_some(),
        "oblique leaf query exhausted: {} {:?}",
        q.visits,
        q.pending_lower
    );
    assert!(q.visits <= 64);
    Ok(())
}

#[test]
fn coplanar_corner_contacts_never_become_a_certain_miss() -> Result<(), Error> {
    let pixels = data([[128; 4], [128; 4], [128; 4], [128; 4], [255; 4]]);
    let field = Field {
        amplitudes: [0.0; 4],
        ..fixture(&pixels)
    };
    let diagonal = std::f64::consts::FRAC_1_SQRT_2;
    let query = ray::query(
        field,
        Ray {
            o: [-2.0, 0.0, 0.0],
            d: [diagonal, diagonal, 0.0],
        },
        64,
    )?;
    assert!(
        query.contact.is_some() || !query.complete,
        "a true corner contact became a certified miss"
    );
    Ok(())
}
#[test]
fn traversal_limits_and_unknown_outcomes_are_explicit() -> Result<(), Error> {
    let ray = Ray {
        o: [0.0, 0.0, 0.1],
        d: [0.3, 0.4, -0.75_f64.sqrt()],
    };
    for limit in [0, 1, 64, u32::MAX] {
        let q = ray::query(LEAF, ray, limit)?;
        assert!(q.visits <= 64 && q.visits <= limit);
        if limit <= 1 {
            assert!(!q.complete && q.contact.is_none());
        }
        if !q.complete {
            assert!(q.pending_lower.is_some());
        }
    }
    Ok(())
}
