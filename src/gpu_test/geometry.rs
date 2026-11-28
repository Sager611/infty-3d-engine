//! Every geometry interface function, checked on every geometry against independent references.
use super::support::run_field;
use crate::geometry::{Geometry, Kind, api::FUNCTIONS};
use crate::math::{
    Ray,
    vector::{dot, madd, sub},
};
use std::error::Error;

type TestResult = Result<(), Box<dyn Error>>;

/// Interface functions checked here; adding one fails until it gets a check.
const CHECKED: [&str; 7] = [
    "query",
    "surface_uv",
    "curvature",
    "enclosing_curvature",
    "field_relevant",
    "field_chord",
    "shade",
];

/// Probed parameters, and whether the surface is closed (bends at least as much as its enclosing sphere).
const fn probed(kind: Kind) -> (&'static [f64], bool) {
    match kind {
        Kind::Sphere => (&[0.0], true),
        Kind::Goursat => (&[1.05, 1.5, 1.95], true),
        Kind::Plane => (&[0.0], false),
        Kind::FinitePlane => (&[0.0, 1.0, 2.0], false),
    }
}

fn unit(v: [f64; 3]) -> [f64; 3] {
    let length = dot(v, v).sqrt();
    v.map(|c| c / length)
}

/// Rays from a radius-4 sphere toward points within 1.2 of the origin.
fn rays() -> Vec<Ray> {
    let mut seed = 0x2545_f491_u32;
    let mut next = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(seed >> 8) / f64::from(1_u32 << 23) - 1.0
    };
    (0..192)
        .map(|_| {
            let o = unit([next(), next(), next()]).map(|c| 4.0 * c);
            let target = [next(), next(), next()].map(|c| 1.2 * c);
            Ray {
                o,
                d: unit(sub(target, o)),
            }
        })
        .collect()
}

/// `verify_geometry` input: o.xyz, tag, d.xyz, parameter.
#[expect(
    clippy::cast_possible_truncation,
    reason = "Reference rays narrow once at the GPU boundary"
)]
fn probes(geometry: Geometry, rays: &[Ray]) -> Result<Vec<[f32; 8]>, Box<dyn Error>> {
    let tag = f32::from(u8::try_from(geometry.kind().tag())?);
    Ok(rays
        .iter()
        .map(|r| {
            let [ox, oy, oz] = r.o.map(|c| c as f32);
            let [dx, dy, dz] = r.d.map(|c| c as f32);
            [ox, oy, oz, tag, dx, dy, dz, geometry.parameter() as f32]
        })
        .collect())
}

#[derive(Default)]
struct Counts {
    hits: usize,
    bends: usize,
    seams: usize,
}

/// One probe row: query vs the CPU kernel, enclosure, shading, field culling, then curvature and UV
/// against a neighbour one step along the ray's tangent.
fn check(
    geometry: Geometry,
    enclosing: f64,
    ray: Ray,
    row: &[f32],
    counts: &mut Counts,
) -> TestResult {
    let row: Vec<f64> = row.iter().copied().map(f64::from).collect();
    let at = |i: usize| [row[i], row[i + 1], row[i + 2]];
    let (hit, t, k, k_enclosing) = (row[0] > 0.5, row[1], row[2], row[3]);
    let (n, near_p, near_n, p) = (at(4), at(8), at(12), at(16));
    let label = format!(
        "{:?} {} o={:?} d={:?}",
        geometry.kind(),
        geometry.parameter(),
        ray.o,
        ray.d
    );
    let cpu = geometry.query(ray)?;
    if !cpu.uncertain && cpu.sample.hit && dot(ray.d, cpu.sample.n).abs() > 0.1 {
        assert!(hit, "missed {label}");
        assert!(
            (t - cpu.sample.t).abs() <= 1e-4 * (1.0 + t),
            "t {t} {label}"
        );
        assert!(
            sub(n, cpu.sample.n).iter().all(|c| c.abs() < 1e-3),
            "n {n:?} {label}"
        );
        counts.hits += 1;
    } else if !cpu.uncertain && !cpu.sample.hit && cpu.sample.gap > 1e-3 {
        assert!(!hit, "false hit {label}");
    }
    assert!(
        (k_enclosing - enclosing).abs() <= 1e-5 * (1.0 + enclosing),
        "enclosing {k_enclosing} {label}"
    );
    if !hit {
        return Ok(());
    }
    assert!(
        dot(p, p).sqrt() * enclosing <= 1.0 + 1e-5,
        "outside enclosure {label}"
    );
    assert!(
        row[19] >= 0.0 && row[27] <= 1.1 && row[27].is_finite(),
        "shade {label}"
    );
    // A field material's exact hit is never culled.
    assert!(
        row[24] < 0.5 || (row[25] > 0.5 && row[26] > 0.5),
        "culled field hit {label}"
    );
    let tangent = madd(ray.d, n, -dot(ray.d, n));
    let length = dot(tangent, tangent).sqrt();
    let offset = sub(
        near_p,
        madd(p, tangent, 2.0_f64.powi(-10) / length.max(1e-20)),
    );
    if length < 0.3 || row[7] < 0.5 || dot(offset, offset).sqrt() > 1e-4 {
        return Ok(());
    }
    // Curvature: the change of the surface's own normal along the step.
    let chord = sub(near_p, p);
    let reference = dot(sub(near_n, n), chord) / dot(chord, chord);
    assert!(
        (k - reference).abs() <= 0.03_f64.mul_add(k.abs(), 0.01),
        "curvature {k} vs {reference} {label}"
    );
    // UV predicted from the sample matches the neighbour's own UV within a chart; seams jump far more.
    let mut du = [row[20] - row[22], row[21] - row[23]];
    if geometry.kind().spec().wraps_u {
        du[0] -= du[0].round();
    }
    let inside = [row[22], row[23]]
        .iter()
        .all(|c| (1e-3..=1.0 - 1e-3).contains(c));
    assert!(row[11] > 0.5 || !inside, "invalid inside {label}");
    if du.iter().any(|c| c.abs() >= 0.01) {
        counts.seams += 1;
    } else {
        assert!(du.iter().all(|c| c.abs() <= 2e-4), "uv {du:?} {label}");
        counts.bends += 1;
    }
    Ok(())
}

#[test]
fn every_interface_function_has_an_accuracy_check() {
    assert_eq!(FUNCTIONS.map(|function| function.name), CHECKED);
}

#[test]
fn every_interface_function_is_accurate_on_every_geometry() -> TestResult {
    for kind in Kind::ALL {
        let (parameters, closed) = probed(kind);
        for &parameter in parameters {
            let geometry = Geometry::new(kind, parameter)?;
            let enclosing = if closed {
                1.0 / geometry.radius().ok_or("closed without radius")?
            } else {
                0.0
            };
            let rays = rays();
            let out = run_field(
                "verify_geometry",
                &probes(geometry, &rays)?,
                rays.len() * 32,
                crate::heightfield::LEAF,
            )?;
            let mut counts = Counts::default();
            for (ray, row) in rays.iter().zip(out.chunks_exact(32)) {
                check(geometry, enclosing, *ray, row, &mut counts)?;
            }
            let Counts { hits, bends, seams } = counts;
            assert!(
                hits >= 24 && bends >= 24 && seams * 8 <= bends,
                "{kind:?} {parameter}: {hits} hits, {bends} bends, {seams} seams"
            );
        }
    }
    Ok(())
}
