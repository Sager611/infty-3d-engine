//! Bounded asset decoding; linear RGB + authored relief. See REFERENCES.md.
use super::{EDGE, Level, Material};

pub const fn assets(material: Material) -> [&'static [u8]; 4] {
    match material {
        Material::Steel => [
            include_bytes!("tiles/steel-0.rgba.z"),
            include_bytes!("tiles/steel-1.rgba.z"),
            include_bytes!("tiles/steel-2.rgba.z"),
            include_bytes!("tiles/steel-3.rgba.z"),
        ],
        Material::Brick => [
            include_bytes!("tiles/brick-0.rgba.z"),
            include_bytes!("tiles/brick-1.rgba.z"),
            include_bytes!("tiles/brick-2.rgba.z"),
            include_bytes!("tiles/brick-3.rgba.z"),
        ],
    }
}
pub(super) fn decode(source: &[u8]) -> Result<Vec<u8>, &'static str> {
    let size = usize::try_from(EDGE * EDGE * 4).map_err(|_| "material texture size")?;
    let pixels = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(source, size)
        .map_err(|_| "invalid compressed material texture")?;
    if pixels.len() != size {
        return Err("material texture must be 512x512 RGBA8");
    }
    Ok(pixels)
}
pub fn texture(level: Level) -> Result<Vec<u8>, &'static str> {
    decode(level.source)
}
