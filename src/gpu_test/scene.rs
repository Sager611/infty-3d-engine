use super::support::run;
use crate::{
    math::Ray,
    scene::{
        model::{Kind, Object},
        query::query_object,
    },
};

#[test]
fn transformed_scene_packets_match_cpu() -> Result<(), Box<dyn std::error::Error>> {
    let fixtures = [
        (Kind::Sphere, [0.0, 1.0, -3.0, 1.0, 0.0, 0.0, 1.0, 2.0]),
        (Kind::Sphere, [3.0, 1.0, -3.0, 1.0, 0.0, 0.0, 1.0, 0.75]),
        (Kind::Plane, [0.0, 3.0, 0.0, 3.0, 0.0, -1.0, 0.0, 2.0]),
        (Kind::Plane, [0.0, 3.0, 0.0, 3.0, 1.0, 0.0, 0.0, 2.0]),
        (Kind::Goursat, [0.75, 1.75, -3.0, 2.0, 0.0, 0.0, 1.0, 0.75]),
    ];
    let inputs: Vec<[f32; 8]> = fixtures.iter().map(|(_, ray)| *ray).collect();
    let actual = run("verify_scene", &inputs, inputs.len() * 32)?;
    for (index, (kind, input)) in fixtures.into_iter().enumerate() {
        let object = Object::new(
            7,
            kind,
            [0.0, 1.0, 0.0],
            f64::from(input[7]),
            if kind == Kind::Goursat { 1.5 } else { 0.0 },
        )?;
        let ray = Ray {
            o: std::array::from_fn(|i| f64::from(input[i])),
            d: std::array::from_fn(|i| f64::from(input[i + 4])),
        };
        let expected = query_object(&object, ray)?.packed(ray);
        let output = &actual[index * 32..(index + 1) * 32];
        for (field, expected) in expected.into_iter().enumerate() {
            let value = f64::from(output[field]);
            let tolerance = 2.0e-4 * expected.abs().max(1.0);
            assert!(
                value.is_finite() && (value - expected).abs() <= tolerance,
                "{kind:?} field {field}: {value} != {expected}"
            );
        }
        assert!(output[24..].iter().all(|v| v.to_bits() == 0));
    }
    Ok(())
}
