#[cfg(test)]
use super::pureweight;

pub type V = [f64; 3];

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
