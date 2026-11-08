#[must_use]
pub fn pureweight(uv: [f64; 2], rho: f64) -> f64 {
    let rho = rho.clamp(0.0, 1.0);
    let lod = (-(rho.max(1.0e-12) * 32.0).log2()).clamp(0.0, 18.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let level = lod.floor() as u32;
    let t = smoothstep(0.0, 1.0, lod.fract());
    let a = tile(uv, level, rho);
    let b = tile(uv, level + 1, rho);
    let c = tile(uv, level + 2, rho);
    mix(mix(a, b, 0.4), mix(b, c, 0.4), t)
}

fn tile(uv: [f64; 2], level: u32, rho: f64) -> f64 {
    let f = [
        uv[0].abs() - uv[0].abs().floor(),
        uv[1].abs() - uv[1].abs().floor(),
    ];
    let scale_u = 1u32 << level;
    let scale = f64::from(scale_u);
    let s = [f[0] * scale, f[1] * scale];
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let mut cell = [s[0].floor() as u32, s[1].floor() as u32];
    let p = [s[0] - s[0].floor(), s[1] - s[1].floor()];
    if uv[0] < 0.0 {
        cell[0] = (0u32
            .wrapping_sub(cell[0])
            .wrapping_sub(u32::from(p[0] > 0.0)))
            & (scale_u - 1);
    }
    if uv[1] < 0.0 {
        cell[1] = (0u32
            .wrapping_sub(cell[1])
            .wrapping_sub(u32::from(p[1] > 0.0)))
            & (scale_u - 1);
    }
    let edge = p[0].min(1.0 - p[0]).min(p[1].min(1.0 - p[1]));
    let mut hash = cell[0] ^ cell[1].wrapping_mul(0x9e37_79b9) ^ level.wrapping_mul(0x85eb_ca6b);
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7feb_352d);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x846c_a68b);
    hash ^= hash >> 16;
    let fill = 0.6f64.mul_add(f64::from(hash >> 24) / 255.0, 0.2);
    let inside = smoothstep(0.0, rho.mul_add(scale, 0.04), edge);
    let signal = mix(0.15, fill, inside);
    mix(signal, 0.5, smoothstep(0.25, 1.0, rho * scale))
}

fn mix(a: f64, b: f64, t: f64) -> f64 {
    b.mul_add(t, a * (1.0 - t))
}

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * 2.0f64.mul_add(-t, 3.0)
}
