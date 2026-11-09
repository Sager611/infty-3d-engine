#[cfg(test)]
use super::pureweight;

pub type V = [f64; 3];

/// Validate JavaScript numbers before a WASM integer binding can coerce them.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Finite integral u32 range is checked before conversion."
)]
pub fn checked_u32(value: f64) -> Option<u32> {
    if !value.is_finite() || !(0.0..=f64::from(u32::MAX)).contains(&value) || value.fract() != 0.0 {
        return None;
    }
    Some(value as u32)
}

#[cfg(test)]
mod tests {
    use super::checked_u32;
    #[test]
    fn integer_packets_do_not_truncate_saturate_or_wrap() {
        for (number, expected) in [
            (-0.0, 0),
            (0.0, 0),
            (1.0, 1),
            (f64::from(u32::MAX), u32::MAX),
        ] {
            assert_eq!(checked_u32(number), Some(expected));
        }
        for number in [
            -1.0,
            1.5,
            f64::from(u32::MAX) + 1.0,
            f64::NAN,
            f64::INFINITY,
        ] {
            assert_eq!(checked_u32(number), None);
        }
    }
}

#[derive(Clone, Copy)]
pub struct Ray {
    pub o: V,
    pub d: V,
}

#[derive(Clone, Copy)]
pub struct Sample {
    pub hit: bool,
    pub t: f64,
    pub gap: f64,
    pub p: V,
    pub n: V,
    pub uv: [f64; 2],
}

/// Canonical ray result with bounded gaps and explicitly optional work.
#[derive(Clone, Copy)]
pub struct GeometrySample {
    pub sample: Sample,
    pub gap_lower: f64,
    pub error: f64,
    pub uncertain: bool,
    pub gap_computed: bool,
}

pub const fn finite3(p: V) -> bool {
    p[0].is_finite() && p[1].is_finite() && p[2].is_finite()
}

impl Sample {
    #[cfg(test)]
    #[must_use]
    pub fn packed(self, ray: Ray, rho: f64) -> [f64; 16] {
        [
            if self.hit { 1.0 } else { 0.0 },
            self.t,
            self.gap,
            self.uv[0],
            self.uv[1],
            self.p[0],
            self.p[1],
            self.p[2],
            self.n[0],
            self.n[1],
            self.n[2],
            pureweight(self.uv, rho),
            self.t.mul_add(ray.d[0], ray.o[0]),
            self.t.mul_add(ray.d[1], ray.o[1]),
            self.t.mul_add(ray.d[2], ray.o[2]),
            rho,
        ]
    }
}
