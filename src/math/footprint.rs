use super::types::{Ray, Sample};

pub fn footprint(sample: Sample, ray: Ray, height: f64, radius: f64, sphere: bool) -> f64 {
    if !sample.hit
        || !sample.t.is_finite()
        || sample.t <= 0.0
        || !height.is_finite()
        || height <= 0.0
    {
        return 1.0;
    }
    let nd = ray.d[2].mul_add(
        sample.n[2],
        ray.d[1].mul_add(sample.n[1], ray.d[0] * sample.n[0]),
    );
    if !nd.is_finite() || nd.abs() == 0.0 {
        return 1.0;
    }
    let density = if sphere {
        let radial = sample.n[0].hypot(sample.n[2]);
        if radial == 0.0 || !radius.is_finite() || radius <= 0.0 {
            return 1.0;
        }
        (1.0 / std::f64::consts::PI).max(1.0 / (std::f64::consts::TAU * radial)) / radius
    } else {
        1.0
    };
    (2.0 * sample.t * density / (1.8 * height * nd.abs())).min(1.0)
}
