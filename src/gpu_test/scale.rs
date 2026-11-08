use super::support::run_scene;
use crate::{
    math::Ray,
    scene::{
        model::{Kind, Object},
        packed::pack,
        query::{QueryMode, query_objects},
    },
};

#[test]
fn thousands_of_mixed_gpu_instances_match_brute_force_and_cpu()
-> Result<(), Box<dyn std::error::Error>> {
    let mut objects = Vec::new();
    for id in 1..=4096_u32 {
        objects.push(Object::new(
            id,
            if id % 2 == 0 {
                Kind::Goursat
            } else {
                Kind::Sphere
            },
            [
                f64::from(id % 16) - 8.0,
                f64::from(id / 16 % 16) - 8.0,
                f64::from(id / 256) - 8.0,
            ],
            0.12,
            if id % 2 == 0 { 1.5 } else { 0.0 },
        )?);
    }
    objects.push(Object::new(5000, Kind::Plane, [0.0, -9.0, 0.0], 1.0, 0.0)?);
    let packet = pack(&objects, &[4096])?;
    let count = f32::from(u16::try_from(packet.count)?);
    let mut rays = Vec::new();
    for x in [-7.0, 0.09, 7.0, 20.0] {
        for brute in [0.0, 1.0] {
            rays.push([x, 0.09, -20.0, count, 0.0, 0.0, 1.0, brute]);
        }
    }
    let output = run_scene("verify_spatial", &rays, &packet.records)?;
    for (i, pair) in output.chunks_exact(32).enumerate() {
        assert_eq!(&pair[..2], &pair[16..18]);
        assert_eq!(&pair[3..12], &pair[19..28]);
        assert!(pair[2] < pair[18] / 8.0);
        let ray = Ray {
            o: [f64::from(rays[i * 2][0]), f64::from(rays[i * 2][1]), -20.0],
            d: [0.0, 0.0, 1.0],
        };
        let cpu = query_objects(&objects, ray, QueryMode::First)?;
        if let Some(hit) = cpu.first() {
            assert!((f64::from(pair[0]) - f64::from(hit.id)).abs() < 0.5);
            let expected = hit.packed(ray);
            for (gpu, field) in [
                (pair[1], 3),
                (pair[4], 6),
                (pair[5], 7),
                (pair[6], 8),
                (pair[7], 12),
                (pair[8], 9),
                (pair[9], 10),
                (pair[10], 11),
                (pair[11], 13),
            ] {
                assert!(
                    (f64::from(gpu) - expected[field]).abs() < 2e-4,
                    "field {field}"
                );
            }
        } else {
            assert_eq!(pair[0].to_bits(), 0);
        }
    }
    Ok(())
}

#[test]
fn gap_and_outline_hierarchy_matches_exhaustive_misses() -> Result<(), Box<dyn std::error::Error>> {
    let objects = [
        Object::new(4096, Kind::Sphere, [0.0; 3], 1.0, 0.0)?,
        Object::new(33, Kind::Goursat, [5.0, 0.0, 0.0], 1.0, 1.5)?,
        Object::new(5000, Kind::Plane, [0.0, -3.0, 0.0], 1.0, 0.0)?,
    ];
    let packet = pack(&objects, &[4096, 33])?;
    let count = f32::from(u16::try_from(packet.count)?);
    let rays = [1.001, 1.2, 2.49, 7.55]
        .into_iter()
        .flat_map(|x| [0.0, 1.0].map(|brute| [x, 0.0, -5.0, count, 0.0, 0.0, 1.0, brute]))
        .collect::<Vec<_>>();
    let output = run_scene("verify_spatial_gaps", &rays, &packet.records)?;
    for pair in output.chunks_exact(32) {
        assert_eq!(&pair[..16], &pair[16..]);
    }
    assert!(output[0] > 0.0, "outline outside root bounds must survive");
    assert!((output[1] - 0.001).abs() < 2e-5, "analytic halo oracle");
    Ok(())
}
