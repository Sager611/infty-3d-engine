//! One branchless height sample; the Leaf cup gives its finite carrier a close ray seed.
use super::{Field, repeat};
use crate::{
    geometry::{Error, finite},
    math::{
        Ray, Sample,
        types::GeometrySample,
        vector::{dot, madd, sub},
    },
};

const CUP_X: f64 = 4.375;
const CUP_Y: f64 = 0.0625;
const STEM: f64 = 0.09375;
const FLOOR: f64 = 0.15;

fn gradient(point: [f64; 3], gain: f64) -> [f64; 3] {
    [
        -2.0 * CUP_X * gain * (point[0] + STEM),
        -2.0 * CUP_Y * gain * point[1],
        1.0,
    ]
}

fn roots(a: f64, b: f64, c: f64) -> [f64; 2] {
    if a == 0.0 {
        return [-c / b; 2];
    }
    let disc = b.mul_add(b, -4.0 * a * c);
    let q = -0.5 * (b + disc.sqrt().copysign(b));
    if q == 0.0 {
        return [-b / (2.0 * a); 2];
    }
    let (first, second) = (q / a, c / q);
    [first.min(second), first.max(second)]
}

fn seeds(ray: Ray, gain: f64) -> [Sample; 2] {
    let (scale_x, scale_y) = (CUP_X * gain, CUP_Y * gain);
    let shift = (-dot(ray.o, ray.d)).max(0.0);
    let origin = madd(ray.o, ray.d, shift);
    let ox = origin[0] + STEM;
    let oy = origin[1];
    let [dx, dy, dz] = ray.d;
    let a = scale_x.mul_add(dx * dx, scale_y * dy * dy);
    let b = (2.0 * scale_x * ox).mul_add(dx, (2.0 * scale_y * oy).mul_add(dy, -dz));
    let c = scale_x.mul_add(
        ox * ox,
        scale_y.mul_add(oy * oy, (-FLOOR).mul_add(gain, -origin[2])),
    );
    roots(a, b, c).map(|root| finite::sample(madd(origin, ray.d, root), shift + root, true))
}

fn in_domain(sample: Sample) -> bool {
    sample.t.is_finite() && sample.t >= 0.0 && sample.p[0].abs() <= 1.0 && sample.p[1].abs() <= 1.0
}

impl Field<'_> {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Validated UV scales to 0..=512000"
    )]
    fn bilinear(self, uv: [f64; 2], order: usize, stride: usize, edge: usize) -> f64 {
        let scale = f64::from(self.edge * repeat(order));
        let [x, y] = [uv[0] * scale, uv[1] * scale];
        let (ix, iy) = (x as usize, y as usize);
        let (fx, fy) = (x.fract(), y.fract());
        let last = edge - 1;
        let mask = last | 0_usize.wrapping_sub(usize::from(order.is_multiple_of(4)));
        let at = |i: usize, j: usize| {
            f64::from(
                self.data[order * stride + (j & mask).min(last) * edge + (i & mask).min(last)],
            )
        };
        let low = at(ix, iy);
        let high = at(ix, iy + 1);
        let lo = (at(ix + 1, iy) - low).mul_add(fx, low);
        let hi = (at(ix + 1, iy + 1) - high).mul_add(fx, high);
        (hi - lo).mul_add(fy, lo)
    }

    fn height(self, tap: impl Fn(usize) -> f64) -> Result<f64, Error> {
        let [a, b, c, d] = self.amplitudes.map(|v| v / 255.0);
        let value = d.mul_add(
            tap(3) - 128.0,
            c.mul_add(
                tap(2) - 128.0,
                b.mul_add(tap(1) - 128.0, a * (tap(0) - 128.0)),
            ),
        );
        value.is_finite().then_some(value).ok_or(Error::Numeric)
    }

    fn blade_normal(
        self,
        point: [f64; 3],
        uv: [f64; 2],
        macro_tap: impl Fn([f64; 2]) -> f64,
    ) -> [f64; 3] {
        let mut normal = gradient(point, self.amplitudes[0]);
        let step = 3.0 / f64::from(self.edge);
        for (axis, value) in normal.iter_mut().take(2).enumerate() {
            let (mut lo, mut hi) = (uv, uv);
            lo[axis] = (uv[axis] - step).max(0.0);
            hi[axis] = (uv[axis] + step).min(1.0);
            let slope = (macro_tap(lo) - macro_tap(hi)) * self.amplitudes[0] / (1020.0 * step);
            *value = 0.7_f64.mul_add(slope, 0.3 * *value);
        }
        normal
    }

    fn project(
        self,
        direction: [f64; 3],
        sample: Sample,
        axis: f64,
        height: f64,
        covered: impl Fn([f64; 2]) -> bool,
        macro_tap: impl Fn([f64; 2]) -> f64,
    ) -> Option<Sample> {
        let normal = if self.cup {
            gradient(sample.p, self.amplitudes[0])
        } else {
            [0.0, 0.0, 1.0]
        };
        let slope = dot(direction, normal);
        let difference = height - sample.p[2];
        // Bound the tangent-plane correction; the analytic roots provide candidates.
        let step = if difference == 0.0 {
            0.0
        } else if self.cup {
            let magnitude = slope.abs().max(1.0 / 1_048_576.0);
            let divisor = if slope < 0.0 { -magnitude } else { magnitude };
            (difference / divisor).clamp(-0.05, 0.05)
        } else {
            difference / slope
        };
        let local = madd(sample.p, direction, step);
        let mut projected = finite::sample(local, sample.t + step, true);
        if !in_domain(projected) || !covered(projected.uv) {
            return None;
        }
        let normal = if self.cup {
            self.blade_normal(local, projected.uv, macro_tap)
        } else {
            normal
        };
        let length = dot(normal, normal).sqrt();
        projected.p = finite::unframe(local, axis);
        projected.n = finite::unframe(normal.map(|v| v / length), axis);
        Some(projected)
    }

    fn finite_texh(
        self,
        ray: Ray,
        mut base: GeometrySample,
        axis: f64,
        tap: impl Fn([f64; 2], usize) -> f64,
    ) -> Result<Option<GeometrySample>, Error> {
        let local_ray = Ray {
            o: finite::frame(ray.o, axis),
            d: finite::frame(ray.d, axis),
        };
        let covered = |uv| tap(uv, 4) >= 127.5;
        let candidates = if self.cup {
            seeds(local_ray, self.amplitudes[0]).map(Some)
        } else {
            [
                base.sample.hit.then(|| {
                    finite::sample(finite::frame(base.sample.p, axis), base.sample.t, true)
                }),
                None,
            ]
        };
        let mut seeded = base.sample.hit;
        for candidate in candidates.into_iter().flatten() {
            if self.cup && !(in_domain(candidate) && covered(candidate.uv)) {
                continue;
            }
            seeded = true;
            let height = self.height(|order| tap(candidate.uv, order))?;
            if let Some(hit) = self.project(local_ray.d, candidate, axis, height, covered, |uv| {
                tap(uv, 0)
            }) {
                return Ok(Some(GeometrySample {
                    sample: hit,
                    gap_lower: 0.0,
                    error: finite::rounding(ray, hit.t),
                    uncertain: !self.cup && base.uncertain,
                }));
            }
        }
        base.sample.hit = false;
        Ok(seeded.then_some(base))
    }

    pub fn texh(
        self,
        ray: Ray,
        mut base: GeometrySample,
        finite_axis: Option<f64>,
    ) -> Result<GeometrySample, Error> {
        if base.sample.hit && finite_axis.is_none() {
            return Ok(base);
        }
        let stride = self.stride()?;
        let edge = usize::try_from(self.edge).map_err(|_| Error::Numeric)?;
        let tap = |uv, order| self.bilinear(uv, order, stride, edge);
        let covered = |uv| tap(uv, 4) >= 127.5;
        if let Some(axis) = finite_axis
            && let Some(result) = self.finite_texh(ray, base, axis, tap)?
        {
            return Ok(result);
        }
        if base.uncertain || base.gap_lower > self.reach() {
            return Ok(base);
        }
        // Preserve the last vertex at UV 1; wrap coordinates outside the unit tile.
        let unit = |u: f64| u - u.floor() + f64::from(u8::from(u.to_bits() == 1.0_f64.to_bits()));
        let [u, v] = base.sample.uv;
        let uv = [unit(u), unit(v)];
        let point = madd(ray.o, ray.d, base.sample.t);
        let side = dot(sub(point, base.sample.p), base.sample.n);
        if side != 0.0
            && (finite_axis.is_none() || covered(uv))
            && side.signum() * self.height(|order| tap(uv, order))? >= base.sample.gap
        {
            base.sample.hit = true;
            base.sample.p = point;
            base.sample.gap = 0.0;
            base.gap_lower = 0.0;
        }
        Ok(base)
    }
}
