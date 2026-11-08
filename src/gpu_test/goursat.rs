use crate::math::{Ray, goursat};

use super::support::run;

const TOLERANCE: f64 = 2.0e-4;
const RESIDUAL_TOLERANCE: f64 = 8.0e-4;
const ERROR_BOUND: f64 = TOLERANCE;
const BETA_105_BITS: u32 = 1.05_f32.to_bits();
const BETA_150_BITS: u32 = 1.5_f32.to_bits();
const BETA_195_BITS: u32 = 1.95_f32.to_bits();

struct Case {
    name: &'static str,
    input: [f32; 8],
}

fn normalized(x: [f32; 3]) -> [f32; 3] {
    let length = x[0].mul_add(x[0], x[1].mul_add(x[1], x[2] * x[2])).sqrt();
    [x[0] / length, x[1] / length, x[2] / length]
}

fn case(name: &'static str, o: [f32; 3], d: [f32; 3], beta: f32) -> Case {
    let d = normalized(d);
    Case {
        name,
        input: [o[0], o[1], o[2], beta, d[0], d[1], d[2], 1.0],
    }
}

fn cases() -> Vec<Case> {
    let mut cases = vec![
        case("axis hole", [0.0, 0.0, -3.0], [0.0, 0.0, 1.0], 1.5),
        case("bridge", [0.6, 0.6, -3.0], [0.0, 0.0, 1.0], 1.125),
        case("inside", [1.0, 1.0, 1.0], [1.0, 0.0, 0.0], 1.5),
        case("tangent", [0.5, 0.5, -3.0], [0.0, 0.0, 1.0], 1.125),
        case("near miss", [0.4999, 0.4999, -3.0], [0.0, 0.0, 1.0], 1.125),
        case("far", [8.0, 8.0, -3.0], [0.0, 0.0, 1.0], 1.5),
        case(
            "enclosure tangent",
            [2.3125, 0.3125, 1.3125],
            [-1.0, 1.0, 0.0],
            1.566_696_2,
        ),
    ];
    let grid = [
        ([0.13, 0.27, -3.0], [0.20, 0.10, 0.97]),
        ([-0.31, 0.22, -3.2], [-0.30, 0.20, 0.93]),
        ([0.42, -0.18, -2.7], [0.15, -0.25, 0.96]),
    ];
    for beta in [1.05_f32, 1.5_f32, 1.95_f32] {
        for (index, (origin, direction)) in grid.into_iter().enumerate() {
            let name = if beta.to_bits() == BETA_105_BITS {
                ["grid beta 1.05/0", "grid beta 1.05/1", "grid beta 1.05/2"][index]
            } else if beta.to_bits() == BETA_150_BITS {
                ["grid beta 1.5/0", "grid beta 1.5/1", "grid beta 1.5/2"][index]
            } else if beta.to_bits() == BETA_195_BITS {
                ["grid beta 1.95/0", "grid beta 1.95/1", "grid beta 1.95/2"][index]
            } else {
                unreachable!("unexpected beta bits: {}", beta.to_bits());
            };
            cases.push(case(name, origin, direction, beta));
        }
    }
    cases
}

fn expected(name: &str, input: [f32; 8]) -> Result<[f64; 16], String> {
    let ray = Ray {
        o: [
            f64::from(input[0]),
            f64::from(input[1]),
            f64::from(input[2]),
        ],
        d: [
            f64::from(input[4]),
            f64::from(input[5]),
            f64::from(input[6]),
        ],
    };
    let result = goursat(ray, f64::from(input[3]), true)
        .map_err(|error| format!("{name}: CPU Goursat error: {error:?}; input={input:?}"))?;
    Ok([
        if result.sample.hit { 1.0 } else { 0.0 },
        result.sample.t,
        result.sample.gap,
        result.sample.uv[0],
        result.sample.uv[1],
        result.sample.p[0],
        result.sample.p[1],
        result.sample.p[2],
        result.sample.n[0],
        result.sample.n[1],
        result.sample.n[2],
        result.gap_lower,
        result.error,
        if result.uncertain { 1.0 } else { 0.0 },
        0.0,
        0.0,
    ])
}

fn surface_value(point: [f64; 3], beta: f64) -> f64 {
    point
        .into_iter()
        .map(|coordinate| coordinate.mul_add(coordinate, -1.0).powi(2))
        .sum::<f64>()
        - beta
}

fn assert_physical(actual: &[f32], expected: &[f64; 16], name: &str) {
    for field in 0..12 {
        assert!(
            actual[field].is_finite(),
            "{name} field {field} is not finite"
        );
        let actual = f64::from(actual[field]);
        let difference = (actual - expected[field]).abs();
        let limit = TOLERANCE * expected[field].abs().max(1.0);
        assert!(
            difference <= limit,
            "{name} field {field}: {actual} != {} (limit {limit})",
            expected[field]
        );
    }
}

#[test]
fn verify_goursat_matches_cpu_and_surface_invariants() -> Result<(), Box<dyn std::error::Error>> {
    let cases = cases();
    let inputs: Vec<[f32; 8]> = cases.iter().map(|test_case| test_case.input).collect();
    let actual = run("verify_goursat", &inputs, inputs.len() * 16)?;
    assert_eq!(actual.len(), inputs.len() * 16);

    let axis_gap = (2.0 - f64::sqrt(3.0)).sqrt();
    for (index, test_case) in cases.iter().enumerate() {
        let expected = expected(test_case.name, test_case.input)?;
        let output = &actual[index * 16..(index + 1) * 16];
        if test_case.name == "enclosure tangent" {
            assert_eq!(expected[0].to_bits(), 1.0f64.to_bits());
        }
        assert_physical(output, &expected, test_case.name);

        {
            let beta = f64::from(test_case.input[3]);
            let point = [
                f64::from(output[5]),
                f64::from(output[6]),
                f64::from(output[7]),
            ];
            let residual = surface_value(point, beta).abs();
            assert!(
                residual <= RESIDUAL_TOLERANCE,
                "{} surface residual {residual} at {:?}",
                test_case.name,
                point
            );
        }

        assert!(
            output[12].is_finite() && output[12] >= 0.0,
            "{} error is invalid: {}",
            test_case.name,
            output[12]
        );
        {
            assert!(
                f64::from(output[12]) <= ERROR_BOUND,
                "{} error is too large: {}",
                test_case.name,
                output[12]
            );
        }

        let status = output[13];
        assert!(
            [0.0f32.to_bits(), 1.0f32.to_bits()].contains(&status.to_bits()),
            "{} status is not a precision status: {status}",
            test_case.name
        );
        assert_eq!(
            output[14].to_bits(),
            0.0f32.to_bits(),
            "{} reserved field 14",
            test_case.name
        );
        assert_eq!(
            output[15].to_bits(),
            0.0f32.to_bits(),
            "{} reserved field 15",
            test_case.name
        );
    }

    let axis_index = cases
        .iter()
        .position(|test_case| test_case.name == "axis hole")
        .ok_or("axis hole case missing")?;
    let axis_output = &actual[axis_index * 16..(axis_index + 1) * 16];
    let axis_gap_lower = f64::from(axis_output[11]);
    let axis_gap_upper = f64::from(axis_output[2]);
    assert_eq!(
        axis_gap_lower.to_bits(),
        0.0f64.to_bits(),
        "axis hole bounding-sphere lower gap"
    );
    assert!(
        axis_gap_lower <= axis_gap && axis_gap <= axis_gap_upper,
        "axis hole gap {axis_gap} outside [{axis_gap_lower}, {axis_gap_upper}]"
    );
    assert!(
        (axis_gap_upper - axis_gap).abs() <= TOLERANCE,
        "axis hole gap upper {axis_gap_upper} outside [{}, {}]",
        axis_gap - TOLERANCE,
        axis_gap + TOLERANCE
    );
    Ok(())
}
