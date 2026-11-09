//! Finite directed-rounding intervals for query certificates.
#[cfg(test)]
#[path = "interval/tests.rs"]
mod tests;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArithmeticError;

#[derive(Clone, Copy, Debug)]
pub struct Interval {
    lo: f64,
    hi: f64,
}

impl Interval {
    pub const ZERO: Self = Self { lo: 0.0, hi: 0.0 };
    pub const ONE: Self = Self { lo: 1.0, hi: 1.0 };
    pub fn new(lo: f64, hi: f64) -> Result<Self, ArithmeticError> {
        if !lo.is_finite() || !hi.is_finite() || lo > hi {
            return Err(ArithmeticError);
        }
        Ok(Self { lo, hi })
    }
    pub fn point(value: f64) -> Result<Self, ArithmeticError> {
        Self::new(value, value)
    }
    pub const fn lo(self) -> f64 {
        self.lo
    }
    pub const fn hi(self) -> f64 {
        self.hi
    }
    pub const fn middle(self) -> f64 {
        self.lo.midpoint(self.hi)
    }
    pub fn intersect(self, other: Self) -> Option<Self> {
        let lo = self.lo.max(other.lo);
        let hi = self.hi.min(other.hi);
        (lo <= hi).then_some(Self { lo, hi })
    }
    pub fn add(self, other: Self) -> Result<Self, ArithmeticError> {
        if other.lo == 0.0 && other.hi == 0.0 {
            return Ok(self);
        }
        if self.lo == 0.0 && self.hi == 0.0 {
            return Ok(other);
        }
        Self::new(
            (self.lo + other.lo).next_down(),
            (self.hi + other.hi).next_up(),
        )
    }
    const fn is_point(self, value: f64) -> bool {
        self.lo.to_bits() == value.to_bits() && self.hi.to_bits() == value.to_bits()
    }
    pub fn sub(self, other: Self) -> Result<Self, ArithmeticError> {
        if self.is_point(other.lo) && other.is_point(other.lo) {
            return Ok(Self::ZERO);
        }
        if other.lo == 0.0 && other.hi == 0.0 {
            return Ok(self);
        }
        Self::new(
            (self.lo - other.hi).next_down(),
            (self.hi - other.lo).next_up(),
        )
    }
    pub fn mul(self, other: Self) -> Result<Self, ArithmeticError> {
        if (self.lo == 0.0 && self.hi == 0.0) || (other.lo == 0.0 && other.hi == 0.0) {
            return Ok(Self::ZERO);
        }
        if self.is_point(1.0) {
            return Ok(other);
        }
        if other.is_point(1.0) {
            return Ok(self);
        }
        let [a, b, c, d] = [
            self.lo * other.lo,
            self.lo * other.hi,
            self.hi * other.lo,
            self.hi * other.hi,
        ];
        Self::new(
            a.min(b).min(c).min(d).next_down(),
            a.max(b).max(c).max(d).next_up(),
        )
    }
    pub fn scale(self, value: f64) -> Result<Self, ArithmeticError> {
        self.mul(Self::point(value)?)
    }
    pub fn divide(self, positive: f64) -> Result<Self, ArithmeticError> {
        if !positive.is_finite() || positive <= 0.0 {
            return Err(ArithmeticError);
        }
        if positive.to_bits() == 1.0_f64.to_bits() {
            return Ok(self);
        }
        if self.lo == 0.0 && self.hi == 0.0 {
            return Ok(self);
        }
        Self::new(
            (self.lo / positive).next_down(),
            (self.hi / positive).next_up(),
        )
    }
    pub fn abs(self) -> Self {
        if self.lo >= 0.0 {
            return self;
        }
        if self.hi <= 0.0 {
            return Self {
                lo: -self.hi,
                hi: -self.lo,
            };
        }
        Self {
            lo: 0.0,
            hi: (-self.lo).max(self.hi),
        }
    }
    pub fn square(self) -> Result<Self, ArithmeticError> {
        let absolute = self.abs();
        Self::new(
            (absolute.lo * absolute.lo).next_down().max(0.0),
            (absolute.hi * absolute.hi).next_up(),
        )
    }
    pub fn sqrt(self) -> Result<Self, ArithmeticError> {
        if self.lo < 0.0 {
            return Err(ArithmeticError);
        }
        Self::new(
            self.lo.sqrt().next_down().max(0.0),
            self.hi.sqrt().next_up(),
        )
    }
}

/// Scaled Euclidean norm; overflow is an error, not a plausible finite bound.
pub fn norm(vector: [Interval; 3]) -> Result<Interval, ArithmeticError> {
    let magnitude = vector[0]
        .abs()
        .hi()
        .max(vector[1].abs().hi())
        .max(vector[2].abs().hi());
    if magnitude == 0.0 {
        return Ok(Interval::ZERO);
    }
    let sum = vector[0]
        .divide(magnitude)?
        .square()?
        .add(vector[1].divide(magnitude)?.square()?)?
        .add(vector[2].divide(magnitude)?.square()?)?;
    Interval::new(sum.lo().max(0.0), sum.hi())?
        .sqrt()?
        .scale(magnitude)
}

pub fn distance(a: [f64; 3], b: [f64; 3]) -> Result<Interval, ArithmeticError> {
    norm([
        Interval::point(a[0])?.sub(Interval::point(b[0])?)?,
        Interval::point(a[1])?.sub(Interval::point(b[1])?)?,
        Interval::point(a[2])?.sub(Interval::point(b[2])?)?,
    ])
}
