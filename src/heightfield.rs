//! Immutable bilinear maps; four height bands, coarse coverage and bounded fine UV variation.
use crate::{geometry::Error, math::interval::Interval as I};
pub mod gpu;
mod phase;
mod ray;

pub const STEPS: usize = 64;
pub const AMPLITUDES: [f64; 4] = [
    0.125,
    0.000_976_562_5,
    0.000_122_070_312_5,
    0.000_030_517_578_125,
];
pub const DATA: &[u8; 2_184_540] = include_bytes!("heightfield/leaf.bin");
#[derive(Clone, Copy)]
pub struct Field<'a> {
    data: &'a [u8],
    edge: u32,
    amplitudes: [f64; 4],
    anchor: [f64; 2],
}
pub static LEAF: Field<'static> = Field {
    data: DATA,
    edge: 512,
    amplitudes: AMPLITUDES,
    anchor: [0.0; 2],
};
#[derive(Clone, Copy, Default)]
pub struct Node {
    pub ij: [u32; 2],
    pub depth: u32,
}
#[derive(Clone, Copy)]
struct Work {
    node: Node,
    lower: f64,
}
fn minimum(old: Option<f64>, value: f64) -> f64 {
    old.map_or(value, |v| v.min(value))
}
fn pop(work: &mut [Work; STEPS], len: &mut usize) -> Result<Work, Error> {
    if *len > STEPS {
        return Err(Error::Numeric);
    }
    let index = work
        .iter()
        .take(*len)
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            a.lower
                .total_cmp(&b.lower)
                .then(b.node.depth.cmp(&a.node.depth))
        })
        .map(|(index, _)| index)
        .ok_or(Error::Numeric)?;
    let item = work[index];
    *len -= 1;
    work[index] = work[*len];
    Ok(item)
}
#[derive(Clone, Copy)]
pub struct Bounds {
    pub lo: [f64; 3],
    pub hi: [f64; 3],
}
pub mod point;
#[derive(Clone, Copy)]
pub struct Value {
    pub height: f64,
    pub gradient: [f64; 2],
    pub coverage: f64,
}
#[cfg(test)]
impl Value {
    pub fn normal(self) -> [f64; 3] {
        let scale = self.gradient[0].abs().max(self.gradient[1].abs()).max(1.0);
        let vector = [
            -self.gradient[0] / scale,
            -self.gradient[1] / scale,
            1.0 / scale,
        ];
        let length = vector[0].hypot(vector[1]).hypot(vector[2]);
        vector.map(|value| value / length)
    }
}
impl Node {
    pub fn domain(self) -> Result<[[f64; 2]; 2], Error> {
        if self.depth > 24 {
            return Err(Error::InvalidRequest);
        }
        let side = 1_u32 << self.depth;
        if self.ij[0] >= side || self.ij[1] >= side {
            return Err(Error::InvalidRequest);
        }
        let step = 2.0 / f64::from(side);
        Ok([
            self.ij.map(|v| f64::from(v).mul_add(step, -1.0)),
            self.ij.map(|v| f64::from(v + 1).mul_add(step, -1.0)),
        ])
    }
}
const fn repeat(order: usize) -> u32 {
    [1, 10, 100, 1000, 1][order]
}
fn polynomial(c: [f64; 4], x: f64, y: f64) -> f64 {
    c[3].mul_add(x * y, c[1].mul_add(x, c[2].mul_add(y, c[0])))
}
impl<'a> Field<'a> {
    /// Trusted immutable pyramids; validate layout, finite values and a covered anchor.
    pub fn new(
        data: &'a [u8],
        edge: u32,
        amplitudes: [f64; 4],
        anchor: [f64; 2],
    ) -> Result<Self, Error> {
        let field = Self {
            data,
            edge,
            amplitudes,
            anchor,
        };
        let stride = field.stride()?;
        if data.len() != 5 * stride || amplitudes.iter().any(|value| !value.is_finite()) {
            return Err(Error::InvalidParameter);
        }
        if field.sample(anchor)?.coverage < (64.0 * f64::EPSILON).mul_add(1024.0, 127.5) {
            return Err(Error::InvalidParameter);
        }
        Ok(field)
    }
    /// Pack immutable planes/pyramids; GPU gains must be exact normal f32s or zero.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "Reject every non-exact conversion"
    )]
    pub fn gpu_bytes(self) -> Result<Vec<u8>, Error> {
        let field = Self::new(self.data, self.edge, self.amplitudes, self.anchor)?;
        let stride = field.stride()?;
        let edge = usize::try_from(field.edge).map_err(|_| Error::Numeric)?;
        let coverage = &field.data[4 * stride..];
        let clear_border = (0..edge).all(|i| {
            [i, (edge - 1) * edge + i, i * edge, i * edge + edge - 1]
                .into_iter()
                .all(|at| coverage[at] == 0)
        });
        let mut bytes = Vec::with_capacity(32 + field.data.len());
        for value in [
            field.edge,
            u32::try_from(stride).map_err(|_| Error::Numeric)?,
            field.resolution()?,
            u32::from(clear_border),
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for amplitude in field.amplitudes {
            let value = amplitude as f32;
            if value.abs() > 1.0
                || value.is_subnormal()
                || f64::from(value).to_bits() != amplitude.to_bits()
            {
                return Err(Error::Numeric);
            }
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(field.data);
        Ok(bytes)
    }

    fn stride(self) -> Result<usize, Error> {
        if !(2..=512).contains(&self.edge) || !self.edge.is_power_of_two() {
            return Err(Error::InvalidParameter);
        }
        let count = self.edge * self.edge;
        usize::try_from((count + 2 * (count - 1) / 3 + 3) & !3).map_err(|_| Error::Numeric)
    }
    fn bytes(self, order: usize, level: u32, ij: [u32; 2]) -> Result<[f64; 2], Error> {
        let stride = self.stride()?;
        if order > 4 || level > self.edge.ilog2() {
            return Err(Error::InvalidParameter);
        }
        let side = self.edge >> level;
        let ij = ij.map(|v| {
            if order == 0 || order == 4 {
                v.min(side - 1)
            } else {
                v % side
            }
        });
        let pixel = usize::try_from(ij[1] * side + ij[0]).map_err(|_| Error::Numeric)?;
        let base = order * stride;
        if level == 0 {
            let value = f64::from(*self.data.get(base + pixel).ok_or(Error::Numeric)?);
            return Ok([value; 2]);
        }
        let count = self.edge * self.edge;
        let previous = self.edge >> (level - 1);
        let offset = usize::try_from(count + 2 * (count - previous * previous) / 3)
            .map_err(|_| Error::Numeric)?;
        let index = base + offset + 2 * pixel;
        Ok([
            f64::from(*self.data.get(index).ok_or(Error::Numeric)?),
            f64::from(*self.data.get(index + 1).ok_or(Error::Numeric)?),
        ])
    }
    fn coefficients(self, order: usize, ij: [u32; 2]) -> Result<[f64; 4], Error> {
        let v00 = self.bytes(order, 0, ij)?[0];
        let v10 = self.bytes(order, 0, [ij[0] + 1, ij[1]])?[0];
        let v01 = self.bytes(order, 0, [ij[0], ij[1] + 1])?[0];
        let v11 = self.bytes(order, 0, [ij[0] + 1, ij[1] + 1])?[0];
        Ok([v00, v10 - v00, v01 - v00, v11 - v10 - v01 + v00])
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Validated canonical or mapped fine coordinates fit the u32 texel grid"
    )]
    fn coordinate(self, order: usize, p: f64) -> Result<(u32, f64), Error> {
        let scale = f64::from(self.edge * repeat(order)) * 0.5;
        let mut cell = scale.mul_add(p, scale).floor() as u32;
        let mut fraction = scale.mul_add(p, scale - f64::from(cell));
        if fraction < 0.0 {
            cell = cell.checked_sub(1).ok_or(Error::Numeric)?;
            fraction += 1.0;
        } else if fraction >= 1.0 {
            cell += 1;
            fraction -= 1.0;
        }
        Ok((cell, fraction))
    }
    pub fn sample(self, p: [f64; 2]) -> Result<Value, Error> {
        if !p[0].is_finite() || !p[1].is_finite() || p[0].abs() > 1.0 || p[1].abs() > 1.0 {
            return Err(Error::InvalidPoint);
        }
        self.stride()?;
        let mut value = Value {
            height: 0.0,
            gradient: [0.0; 2],
            coverage: 0.0,
        };
        for order in 0..4 {
            let mapped = phase::map(p, order);
            let (ix, fx) = self.coordinate(order, mapped.point[0])?;
            let (iy, fy) = self.coordinate(order, mapped.point[1])?;
            let coeff = self.coefficients(order, [ix, iy])?;
            let amplitude = self.amplitudes[order] / 255.0;
            value.height = amplitude.mul_add(
                polynomial([coeff[0] - 128.0, coeff[1], coeff[2], coeff[3]], fx, fy),
                value.height,
            );
            let scale = f64::from(self.edge * repeat(order)) * 0.5;
            let gradient = [
                amplitude * scale * coeff[3].mul_add(fy, coeff[1]),
                amplitude * scale * coeff[3].mul_add(fx, coeff[2]),
            ];
            for axis in 0..2 {
                value.gradient[axis] = gradient[0].mul_add(
                    mapped.jacobian[0][axis],
                    gradient[1].mul_add(mapped.jacobian[1][axis], value.gradient[axis]),
                );
            }
        }
        let (ix, fx) = self.coordinate(4, p[0])?;
        let (iy, fy) = self.coordinate(4, p[1])?;
        value.coverage = polynomial(self.coefficients(4, [ix, iy])?, fx, fy);
        if !value.height.is_finite() || value.gradient.iter().any(|value| !value.is_finite()) {
            return Err(Error::Numeric);
        }
        Ok(value)
    }
    pub fn reach(self) -> f64 {
        let [a, b, c, d] = self.amplitudes;
        ((a.abs() + b.abs() + c.abs() + d.abs()) * (128.0 / 255.0)).next_up()
    }

    fn alpha_range(self, order: usize, node: Node) -> Result<I, Error> {
        let side = 1_u32 << node.depth;
        let scale = u64::from(self.edge * repeat(order));
        let mut first = [0_u32; 2];
        let mut last = [0_u32; 2];
        let mut low = [0.0; 2];
        let mut high = [0.0; 2];
        for axis in 0..2 {
            let a = u64::from(node.ij[axis]) * scale;
            let b = u64::from(node.ij[axis] + 1) * scale;
            first[axis] = u32::try_from(a >> node.depth).map_err(|_| Error::Numeric)?;
            last[axis] = u32::try_from(b >> node.depth).map_err(|_| Error::Numeric)?;
            low[axis] = f64::from(u32::try_from(a % u64::from(side)).map_err(|_| Error::Numeric)?)
                / f64::from(side);
            high[axis] = f64::from(u32::try_from(b % u64::from(side)).map_err(|_| Error::Numeric)?)
                / f64::from(side);
        }
        if (2..4).contains(&order) {
            let [lo, hi] = phase::region(node, order)?;
            for axis in 0..2 {
                (first[axis], low[axis]) = self.coordinate(order, lo[axis])?;
                (last[axis], high[axis]) = self.coordinate(order, hi[axis])?;
            }
        }
        let span = (last[0] - first[0] + 1).max(last[1] - first[1] + 1);
        let mut result: Option<[f64; 2]> = None;
        if last[0] - first[0] <= 1 && last[1] - first[1] <= 1 {
            // At most four bilinear subrectangles; each has exactly four extrema.
            for dy in 0..2 {
                for dx in 0..2 {
                    let ij = [first[0] + dx, first[1] + dy];
                    if ij[0] > last[0] || ij[1] > last[1] {
                        continue;
                    }
                    let c = self.coefficients(order, ij)?;
                    let a = [
                        if ij[0] == first[0] { low[0] } else { 0.0 },
                        if ij[1] == first[1] { low[1] } else { 0.0 },
                    ];
                    let b = [
                        if ij[0] == last[0] { high[0] } else { 1.0 },
                        if ij[1] == last[1] { high[1] } else { 1.0 },
                    ];
                    for corner in 0..4 {
                        let v = polynomial(
                            c,
                            if corner & 1 == 0 { a[0] } else { b[0] },
                            if corner & 2 == 0 { a[1] } else { b[1] },
                        );
                        result = Some(result.map_or([v, v], |r| [r[0].min(v), r[1].max(v)]));
                    }
                }
            }
            let [lo, hi] = result.ok_or(Error::Numeric)?;
            // Four bounded products/sums with integer coefficients of total magnitude <=1020.
            let allowance = 64.0 * f64::EPSILON * 1024.0;
            return I::new(
                (lo - allowance).max(0.0).next_down().max(0.0),
                (hi + allowance).min(255.0).next_up().min(255.0),
            )
            .map_err(Error::from);
        }
        let level = (32 - (span - 1).leading_zeros()).min(self.edge.ilog2());
        let a = first.map(|v| v >> level);
        let b = last.map(|v| v >> level);
        for dy in 0..2 {
            for dx in 0..2 {
                let ij = [
                    if dx == 0 { a[0] } else { b[0] },
                    if dy == 0 { a[1] } else { b[1] },
                ];
                let v = self.bytes(order, level, ij)?;
                result = Some(result.map_or(v, |r| [r[0].min(v[0]), r[1].max(v[1])]));
            }
        }
        let [lo, hi] = result.ok_or(Error::Numeric)?;
        Ok(I::new(lo, hi)?)
    }
    pub fn resolution(self) -> Result<u32, Error> {
        self.stride()?;
        let mut highest = 1;
        for order in 0..4 {
            if !self.amplitudes[order].is_finite() {
                return Err(Error::InvalidParameter);
            }
            if self.amplitudes[order] != 0.0 {
                highest = highest.max(repeat(order));
            }
        }
        Ok(self.edge * highest)
    }
    pub fn roundoff(self) -> f64 {
        let base = (64.0 * f64::EPSILON).mul_add(
            self.amplitudes[0].abs()
                + self.amplitudes[1].abs()
                + self.amplitudes[2].abs()
                + self.amplitudes[3].abs(),
            f64::from_bits(64),
        );
        let rounding = phase::ERROR * f64::from(self.edge);
        (1000.0 * rounding).mul_add(
            self.amplitudes[3].abs(),
            (100.0 * rounding).mul_add(self.amplitudes[2].abs(), base),
        )
    }
    pub const fn anchor(self) -> [f64; 2] {
        self.anchor
    }
    pub fn bounds(self, node: Node) -> Result<Option<Bounds>, Error> {
        let [lo, hi] = node.domain()?;
        self.stride()?;
        if self.alpha_range(4, node)?.hi() < 127.5 {
            return Ok(None);
        }
        let mut height = I::ZERO;
        for order in 0..4 {
            height = height.add(
                self.alpha_range(order, node)?
                    .sub(I::point(128.0)?)?
                    .scale(self.amplitudes[order])?
                    .divide(255.0)?,
            )?;
        }
        Ok(Some(Bounds {
            lo: [lo[0], lo[1], height.lo()],
            hi: [hi[0], hi[1], height.hi()],
        }))
    }
}
#[cfg(test)]
mod tests;
