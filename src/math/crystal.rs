use super::{
    interval::{ArithmeticError, Interval as I, distance, norm},
    types::{GeometrySample, Ray, Sample, V},
};
#[path = "crystal/point.rs"]
mod point;
#[path = "crystal/ray.rs"]
mod ray;
#[cfg(test)]
#[path = "crystal/reference.rs"]
pub mod reference;
pub use point::closest;
pub use ray::query;
pub const MIN_SCALE: f64 = 0.000_000_953_674_316_406_25;
pub const RADIUS: f64 = 1.75;
pub const EVALUATIONS: u32 = 27;

#[derive(Clone, Copy)]
pub struct Projection {
    pub p: V,
    pub n: V,
    pub uv: [f64; 2],
    pub lower: f64,
    pub upper: f64,
    pub error: f64,
}
#[derive(Clone, Copy)]
struct Patch {
    r: f64,
    z: f64,
    dr: f64,
    dz: f64,
}
impl Patch {
    const fn rising(scale: f64) -> Self {
        Self {
            r: scale,
            z: 0.5 * scale,
            dr: 0.5 * scale,
            dz: 2.5 * scale,
        }
    }
    const fn falling(scale: f64) -> Self {
        Self {
            r: 1.5 * scale,
            z: 3.0 * scale,
            dr: 0.5 * scale,
            dz: -2.0 * scale,
        }
    }
    const fn core() -> Self {
        Self {
            r: 0.0,
            z: 0.0,
            dr: MIN_SCALE,
            dz: 0.5 * MIN_SCALE,
        }
    }
    fn slope(self) -> f64 {
        self.dz / self.dr
    }
    fn intercept(self) -> f64 {
        (-self.slope()).mul_add(self.r, self.z)
    }
}
const fn cycle(p: V) -> V {
    [p[1], p[2], p[0]]
}
const fn uncycle(p: V) -> V {
    [p[2], p[0], p[1]]
}
const fn turn(mut p: Projection) -> Projection {
    p.p = uncycle(p.p);
    p.n = uncycle(p.n);
    p
}
fn better(a: Projection, b: Projection) -> Projection {
    let mut result = if b.upper < a.upper { b } else { a };
    result.lower = a.lower.min(b.lower);
    result
}
fn power_floor(value: f64) -> f64 {
    f64::from_bits(value.clamp(MIN_SCALE * 0.5, 1.0).to_bits() & 0x7ff0_0000_0000_0000)
}
fn quotient(n: I, d: I) -> Result<I, ArithmeticError> {
    if d.lo() <= 0.0 {
        return Err(ArithmeticError);
    }
    n.mul(I::new(
        (1.0 / d.hi()).next_down(),
        (1.0 / d.lo()).next_up(),
    )?)
}
fn normal(direction: [f64; 2], slope: f64) -> V {
    let length = slope.hypot(1.0);
    [
        -slope * direction[0] / length,
        -slope * direction[1] / length,
        1.0 / length,
    ]
}
fn uv(p: V, n: V) -> [f64; 2] {
    if n[0].abs() >= n[1].abs() && n[0].abs() >= n[2].abs() {
        [p[1], p[2]]
    } else if n[1].abs() >= n[2].abs() {
        [p[0], p[2]]
    } else {
        [p[0], p[1]]
    }
}
#[cfg(test)]
#[path = "crystal/tests.rs"]
mod tests;
