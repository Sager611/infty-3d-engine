//! Linear RGB/height tiles; leaf relief additionally defines finite-plane geometry.
use std::collections::BTreeSet;
pub mod gpu;
mod texture;
pub use texture::texture;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    Steel,
    Brick,
    Leaf,
}
/// Material-wide shading/relief data; shaders read it from a generated table.
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub roughness: f32,
    pub metallic: f32,
    pub occlusion: f32,
    /// Relief is cut into the carrier: the base order is measured from its top.
    pub carve: bool,
}
impl Material {
    pub const ALL: [Self; 3] = [Self::Steel, Self::Brick, Self::Leaf];
    pub const fn spec(self) -> Spec {
        let (roughness, metallic, occlusion, carve) = match self {
            Self::Steel => (0.26, 1.0, 0.0, false),
            Self::Brick => (0.86, 0.0, 1.0, true),
            Self::Leaf => (0.45, 0.0, 0.0, false),
        };
        Spec {
            roughness,
            metallic,
            occlusion,
            carve,
        }
    }
    pub const fn tag(self) -> u8 {
        match self {
            Self::Steel => 0,
            Self::Brick => 1,
            Self::Leaf => 2,
        }
    }
    pub const fn from_tag(tag: u32) -> Option<Self> {
        match tag {
            0 => Some(Self::Steel),
            1 => Some(Self::Brick),
            2 => Some(Self::Leaf),
            _ => None,
        }
    }
}
pub const EDGE: u32 = 512;
#[derive(Clone, Copy, Debug)]
pub struct Atlas {
    pub cells: [u16; 2],
    pub border: [f32; 4],
}
#[derive(Clone, Copy, Debug)]
pub struct Level {
    pub atlas: Option<Atlas>,
    pub source: &'static [u8],
    pub material: Material,
    pub order: i32,
    pub extent: f64,
    pub relief: f32,
    pub color: f32,
    pub roughness: f32,
}
pub fn presets() -> Vec<Level> {
    presets_with_leaf(false)
}
pub fn presets_with_leaf(leaf: bool) -> Vec<Level> {
    Material::ALL
        .into_iter()
        .take(if leaf { 3 } else { 2 })
        .flat_map(|material| {
            let relief = match material {
                Material::Steel => [0.000_05, 0.000_01, 0.000_002, 0.000_000_4],
                Material::Brick => [0.006, 0.006_4, 0.000_64, 0.000_048],
                Material::Leaf => [
                    0.002_187_5,
                    0.000_068_359_375,
                    0.000_008_544_922,
                    0.000_002_136_230_5,
                ],
            };
            relief
                .into_iter()
                .zip(texture::assets(material))
                .zip(0..)
                .map(move |((relief, source), order)| Level {
                    atlas: (material == Material::Brick && order == 0).then_some(Atlas {
                        cells: [2, 6],
                        border: [0.22, 0.214, 0.195, 0.12],
                    }),
                    source,
                    material,
                    order,
                    extent: (if material == Material::Leaf {
                        0.14
                    } else {
                        0.42
                    }) / 10.0_f64.powi(order),
                    relief,
                    color: if order == 0 { 1.0 } else { 0.65 },
                    roughness: 0.025,
                })
        })
        .collect()
}
pub fn validate(levels: &[Level], limit: u32) -> Result<(), &'static str> {
    if levels.is_empty()
        || levels.len() > usize::try_from(limit).map_err(|_| "material layer limit")?
    {
        return Err("material layer limit exceeded");
    }
    let mut keys = BTreeSet::new();
    for level in levels {
        if !keys.insert((level.material.tag(), level.order)) {
            return Err("duplicate material order");
        }
        if (level.material == Material::Leaf && !(0..4).contains(&level.order))
            || level.source.is_empty()
            || level.atlas.is_some_and(|a| {
                a.cells.into_iter().any(|v| v == 0 || u32::from(v) > EDGE)
                    || a.border
                        .into_iter()
                        .any(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
            })
            || !level.extent.is_finite()
            || level.extent < f64::from(f32::MIN_POSITIVE)
            || level.extent > f64::from(f32::MAX)
            || !level.relief.is_finite()
            || level.relief < 0.0
            || f64::from(level.relief) > level.extent
            || !level.color.is_finite()
            || !(0.0..=1.0).contains(&level.color)
            || !level.roughness.is_finite()
            || !(0.0..=1.0).contains(&level.roughness)
        {
            return Err("invalid material level");
        }
    }
    if levels
        .iter()
        .any(|l| !keys.contains(&(l.material.tag(), 0)))
    {
        return Err("material requires one base order");
    }
    Ok(())
}
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "Finite caller origins yield unit fractions and 8-bit cells; packed 24 bits fit f32 exactly"
)]
pub fn phase(level: Level, origin: [f64; 3]) -> [f32; 4] {
    let mut result = [0.0; 4];
    let mut cells = 0_u32;
    for (axis, value) in origin.into_iter().enumerate() {
        let fraction = (value.rem_euclid(level.extent) / level.extent) as f32;
        let cell = ((value.rem_euclid(level.extent * 256.0) - value.rem_euclid(level.extent))
            / level.extent)
            .round()
            .rem_euclid(256.0) as u32;
        let carry = u32::from(fraction >= 1.0);
        result[axis] = fraction.fract();
        cells |= ((cell + carry) & 255) << (axis * 8);
    }
    result[3] = cells as f32;
    result
}
pub fn mip(pixels: &[u8], edge: u32) -> Result<Vec<u8>, &'static str> {
    let side = usize::try_from(edge).map_err(|_| "invalid mip size")?;
    if edge < 2
        || !edge.is_power_of_two()
        || side.checked_mul(side).and_then(|v| v.checked_mul(4)) != Some(pixels.len())
    {
        return Err("invalid RGBA mip input");
    }
    let half = side / 2;
    let mut result = Vec::with_capacity(half * half * 4);
    for y in 0..half {
        for x in 0..half {
            for channel in 0..4 {
                let a = (2 * y * side + 2 * x) * 4 + channel;
                let b = a + side * 4;
                let sum = u16::from(pixels[a])
                    + u16::from(pixels[a + 4])
                    + u16::from(pixels[b])
                    + u16::from(pixels[b + 4]);
                result.push(u8::try_from((sum + 2) / 4).map_err(|_| "invalid mip average")?);
            }
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests;
