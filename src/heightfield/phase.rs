//! Fixed C1 fine-coordinate variation. Inputs are validated canonical points; no state.
use super::{Error, Node, repeat};

pub(super) const RATE: f64 = 31.0 / 16.0;
pub(super) const ERROR: f64 = 128.0 * f64::EPSILON;
pub(super) struct Mapping {
    pub point: [f64; 2],
    pub jacobian: [[f64; 2]; 2],
}
fn hash(ij: [u32; 2]) -> [f64; 2] {
    let word = ij[0].wrapping_mul(0x9e37_79b9) ^ ij[1].wrapping_mul(0x85eb_ca6b);
    [word, word ^ 0x68bc_21eb].map(|mut value| {
        value = (value ^ (value >> 16)).wrapping_mul(0x7feb_352d);
        value = (value ^ (value >> 15)).wrapping_mul(0x846c_a68b);
        f64::from(value ^ (value >> 16)) / 4_294_967_296.0
    })
}
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Canonical points and positive seeds give small non-negative noise cells"
)]
fn noise(point: [f64; 2], period: u32) -> ([f64; 2], [[f64; 2]; 2]) {
    let ij = point.map(|v| v.floor() as u32 % period);
    let next = ij.map(|v| (v + 1) % period);
    let fraction = point.map(|v| v - v.floor());
    let weight = fraction.map(|v| v * v * 2.0_f64.mul_add(-v, 3.0));
    let slope = fraction.map(|v| 6.0 * v * (1.0 - v));
    let a = hash(ij);
    let b = hash([next[0], ij[1]]);
    let c = hash([ij[0], next[1]]);
    let d = hash(next);
    let mut value = [0.0; 2];
    let mut jacobian = [[0.0; 2]; 2];
    for axis in 0..2 {
        let lo = (b[axis] - a[axis]).mul_add(weight[0], a[axis]);
        let hi = (d[axis] - c[axis]).mul_add(weight[0], c[axis]);
        value[axis] = (hi - lo).mul_add(weight[1], lo);
        jacobian[axis] = [
            ((d[axis] - c[axis] - b[axis] + a[axis]).mul_add(weight[1], b[axis] - a[axis]))
                * slope[0],
            (hi - lo) * slope[1],
        ];
    }
    (value, jacobian)
}
pub(super) fn map(p: [f64; 2], order: usize) -> Mapping {
    let mut mapping = Mapping {
        point: p,
        jacobian: [[1.0, 0.0], [0.0, 1.0]],
    };
    if order < 2 {
        return mapping;
    }
    let repeats = f64::from(repeat(order));
    // Orders 2 and 3 keep seeds (13,71) and (47,113); finer orders continue the line.
    let step = f64::from(u32::try_from(order - 2).unwrap_or(0));
    let seed = [step.mul_add(34.0, 13.0), step.mul_add(42.0, 71.0)];
    let q = [0, 1].map(|axis| {
        let scaled = (repeats * 0.5).mul_add(p[axis], repeats * 0.5);
        scaled.mul_add(0.25, seed[axis])
    });
    let (value, gradient) = noise(q, repeat(order) / 4);
    for (axis, row) in mapping.jacobian.iter_mut().enumerate() {
        mapping.point[axis] = (2.5 / repeats).mul_add(value[axis], p[axis]);
        for (entry, slope) in row.iter_mut().zip(gradient[axis]) {
            *entry = slope.mul_add(0.3125, *entry);
        }
    }
    // Each noise partial is <=1.5; each mapping row sums to <=1+2*1.5*0.3125.
    mapping
}
pub(super) fn region(node: Node, order: usize) -> Result<[[f64; 2]; 2], Error> {
    let [lo, hi] = node.domain()?;
    let center = [0, 1].map(|axis| lo[axis].midpoint(hi[axis]));
    let mapped = map(center, order);
    let radius = RATE / f64::from(1_u32 << node.depth) + ERROR;
    let maximum = (1.0 + 2.5 / f64::from(repeat(order))).next_up();
    Ok([
        mapped.point.map(|p| (p - radius).max(-1.0)),
        mapped.point.map(|p| (p + radius).min(maximum)),
    ])
}
