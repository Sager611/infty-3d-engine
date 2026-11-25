//! Lazy immutable texture array and material metadata.
use super::{EDGE, Level, mip, phase, presets, presets_with_leaf, texture, validate};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
pub mod resources;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Row {
    pub identity: [u32; 4],
    pub surface: [f32; 4],
    pub phase: [f32; 4],
    pub mean: [f32; 4],
    pub border: [f32; 4],
}
const _: () = assert!(std::mem::size_of::<Row>() == 80);

pub struct Tiles {
    pub texture: wgpu::Texture,
    pub sampler: wgpu::Sampler,
    pub repeat: wgpu::Sampler,
    pub clamp: wgpu::Sampler,
    pub buffer: wgpu::Buffer,
    levels: Vec<Level>,
    #[cfg(test)]
    means: Vec<[f32; 4]>,
}
fn image(device: &wgpu::Device, edge: u32, layers: u32, mips: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("material tiles"),
        size: wgpu::Extent3d {
            width: edge,
            height: edge,
            depth_or_array_layers: layers,
        },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}
fn sampler(device: &wgpu::Device, mode: wgpu::AddressMode) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("material mip filter"),
        address_mode_u: mode,
        address_mode_v: mode,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    })
}
impl Tiles {
    pub fn empty(device: &wgpu::Device) -> Self {
        Self {
            texture: image(device, 1, 1, 1),
            sampler: sampler(device, wgpu::AddressMode::MirrorRepeat),
            repeat: sampler(device, wgpu::AddressMode::Repeat),
            clamp: sampler(device, wgpu::AddressMode::ClampToEdge),
            buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("material phases"),
                contents: bytemuck::bytes_of(&Row::zeroed()),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            }),
            levels: Vec::new(),
            #[cfg(test)]
            means: Vec::new(),
        }
    }
    pub const fn ready(&self) -> bool {
        !self.levels.is_empty()
    }
    pub fn ensure(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        leaf: bool,
    ) -> Result<bool, &'static str> {
        if self.ready() && self.levels.len() >= if leaf { 12 } else { 8 } {
            return Ok(false);
        }
        let levels = if leaf {
            presets_with_leaf(true)
        } else {
            presets()
        };
        *self = Self::new(device, queue, &levels)?;
        Ok(true)
    }
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        levels: &[Level],
    ) -> Result<Self, &'static str> {
        let limits = device.limits();
        validate(levels, limits.max_texture_array_layers)?;
        if EDGE > limits.max_texture_dimension_2d {
            return Err("512px material textures are unsupported");
        }
        let layers = u32::try_from(levels.len()).map_err(|_| "material layer count")?;
        let texture_array = image(device, EDGE, layers, EDGE.ilog2() + 1);
        let mut means = Vec::with_capacity(levels.len());
        for (layer, level) in levels.iter().enumerate() {
            let mut pixels = texture(*level)?;
            means.push(mean(&pixels));
            let mut edge = EDGE;
            let mut index = 0;
            loop {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture_array,
                        mip_level: index,
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: 0,
                            z: u32::try_from(layer).map_err(|_| "material layer")?,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &pixels,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(edge * 4),
                        rows_per_image: Some(edge),
                    },
                    wgpu::Extent3d {
                        width: edge,
                        height: edge,
                        depth_or_array_layers: 1,
                    },
                );
                if edge == 1 {
                    break;
                }
                pixels = mip(&pixels, edge)?;
                edge /= 2;
                index += 1;
            }
        }
        let rows = rows(levels, &means, [0.0; 3], true)?;
        Ok(Self {
            texture: texture_array,
            sampler: sampler(device, wgpu::AddressMode::MirrorRepeat),
            repeat: sampler(device, wgpu::AddressMode::Repeat),
            clamp: sampler(device, wgpu::AddressMode::ClampToEdge),
            buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("material phases"),
                contents: bytemuck::cast_slice(&rows),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            }),
            levels: levels.to_vec(),
            #[cfg(test)]
            means,
        })
    }
    #[cfg(test)]
    pub fn update(
        &self,
        queue: &wgpu::Queue,
        origin: [f64; 3],
        leaf: bool,
    ) -> Result<usize, &'static str> {
        let records = rows(&self.levels, &self.means, origin, leaf)?;
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&records));
        Ok(records.len() * 80)
    }
    pub fn bindings<'a>(&'a self, view: &'a wgpu::TextureView) -> [wgpu::BindGroupEntry<'a>; 5] {
        [
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::Sampler(&self.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: self.buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 10,
                resource: wgpu::BindingResource::Sampler(&self.repeat),
            },
            wgpu::BindGroupEntry {
                binding: 11,
                resource: wgpu::BindingResource::Sampler(&self.clamp),
            },
        ]
    }
    pub fn view(&self) -> wgpu::TextureView {
        self.texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        })
    }
}
#[expect(
    clippy::cast_possible_truncation,
    reason = "Validated extents fit f32 and normalized channel means deliberately round to f32"
)]
fn rows(
    levels: &[Level],
    means: &[[f32; 4]],
    origin: [f64; 3],
    leaf: bool,
) -> Result<Vec<Row>, &'static str> {
    levels
        .iter()
        .zip(means)
        .enumerate()
        .filter(|(_, (level, _))| leaf || level.material != super::Material::Leaf)
        .map(|(index, (level, mean))| {
            Ok(Row {
                identity: [
                    u32::from(level.material.tag()),
                    u32::try_from(index).map_err(|_| "material layer")?,
                    u32::from(level.order == 0)
                        | if level.material == super::Material::Leaf {
                            u32::try_from(level.order).map_err(|_| "leaf order")? << 8
                        } else {
                            0
                        },
                    level
                        .atlas
                        .map_or(0, |a| u32::from(a.cells[0]) | (u32::from(a.cells[1]) << 16)),
                ],
                surface: [
                    level.extent as f32,
                    level.relief,
                    level.color,
                    level.roughness,
                ],
                phase: phase(*level, origin),
                mean: *mean,
                border: level.atlas.map_or([0.0; 4], |a| a.border),
            })
        })
        .collect()
}
fn mean(pixels: &[u8]) -> [f32; 4] {
    let sum = pixels.chunks_exact(4).fold([0_u32; 4], |mut sums, p| {
        for i in 0..4 {
            sums[i] += u32::from(p[i]);
        }
        sums
    });
    let scale = 1.0 / (512.0 * 512.0 * 255.0);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "Unit channel averages deliberately round to f32"
    )]
    let mean = sum.map(|s| (f64::from(s) * scale) as f32);
    mean
}
