//! Base-hit coverage and the unchanged witness-based gap rule; rendered hits resolve in material texh.
use super::{Field, Value};
use crate::{
    geometry::Error,
    math::{
        Ray,
        types::GeometrySample,
        vector::{dot, madd, sub},
    },
};

impl Field<'_> {
    pub(crate) fn uv(self, uv: [f64; 2]) -> Result<Value, Error> {
        let unit = |u: f64| u - u.floor() + f64::from(u8::from(u.to_bits() == 1.0_f64.to_bits()));
        self.sample(uv.map(|u| unit(u).mul_add(2.0, -1.0)))
    }

    pub fn texh(
        self,
        ray: Ray,
        mut base: GeometrySample,
        finite_axis: Option<f64>,
    ) -> Result<GeometrySample, Error> {
        if base.sample.hit {
            if finite_axis.is_some() {
                base.sample.hit = self.uv(base.sample.uv)?.coverage >= 127.5;
            }
            return Ok(base);
        }
        if base.uncertain || base.gap_lower > self.reach() {
            return Ok(base);
        }
        let value = self.uv(base.sample.uv)?;
        let point = madd(ray.o, ray.d, base.sample.t);
        let side = dot(sub(point, base.sample.p), base.sample.n);
        if side != 0.0
            && (finite_axis.is_none() || value.coverage >= 127.5)
            && side.signum() * value.height >= base.sample.gap
        {
            base.sample.hit = true;
            base.sample.p = point;
            base.sample.gap = 0.0;
            base.gap_lower = 0.0;
        }
        Ok(base)
    }
}
