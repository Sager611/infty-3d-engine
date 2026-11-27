//! Lazy immutable texture array and material metadata.
use super::{EDGE, Level, mip, phase, presets_with_leaf, texture, validate};
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
    means: Vec<([f32; 4], u32, f32)>,
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
        let levels = presets_with_leaf(leaf);
        if self.ready() && self.levels.len() >= levels.len() {
            return Ok(false);
        }
        *self = Self::new(device, queue, &levels)?;
        Ok(true)
    }
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        levels: &[Level],
    ) -> Result<Self, &'static str> {
        let limits = device.limits();
        validate(levels, limits.max_texture_array_layers / 3)?;
        if EDGE > limits.max_texture_dimension_2d {
            return Err("512px material textures are unsupported");
        }
        let layers = u32::try_from(levels.len()).map_err(|_| "material layer count")?;
        // Layers: colour/height, then runs to the next top (for groove texels), then to the next groove (for tops).
        let texture_array = image(device, EDGE, 3 * layers, EDGE.ilog2() + 1);
        let mut stats = Vec::with_capacity(levels.len());
        for (layer, level) in (0..layers).zip(levels) {
            let pixels = texture(*level)?;
            let (threshold, contrast, low, high) = split(&pixels);
            let (to_top, to_groove) =
                runs(&pixels, threshold, low, high, level.atlas.map(|a| a.cells));
            upload(queue, &texture_array, layer + layers, to_top)?;
            upload(queue, &texture_array, layer + 2 * layers, to_groove)?;
            stats.push((
                mean(&pixels),
                low,
                bevel(&pixels, threshold, low, high) << 2
                    | median_height(&pixels) << 16
                    | threshold << 24,
                contrast * level.relief,
            ));
            upload(queue, &texture_array, layer, pixels)?;
        }
        // Each material's largest relief step carries direct wall placement.
        let means: Vec<_> = stats
            .iter()
            .zip(levels)
            .map(|(&(mean, low, packed, step), level)| {
                let largest = stats
                    .iter()
                    .zip(levels)
                    .filter(|(_, other)| other.material.tag() == level.material.tag())
                    .fold(0.0_f32, |top, (other, _)| top.max(other.3));
                (
                    mean,
                    packed | u32::from(step > 0.0 && step >= largest) << 1,
                    unit(low),
                )
            })
            .collect();
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
    means: &[([f32; 4], u32, f32)],
    origin: [f64; 3],
    leaf: bool,
) -> Result<Vec<Row>, &'static str> {
    levels
        .iter()
        .zip(means)
        .enumerate()
        .filter(|(_, (level, _))| leaf || !level.material.spec().field)
        .map(|(index, (level, (mean, packed, groove)))| {
            Ok(Row {
                identity: [
                    u32::from(level.material.tag()),
                    u32::try_from(index).map_err(|_| "material layer")?,
                    u32::from(level.order == 0)
                        | u32::try_from(level.order).map_err(|_| "material order")? << 8
                        | packed,
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
                // Atlas mortar, or the groove-class mean height for ordinary tiles.
                border: level.atlas.map_or([0.0, 0.0, 0.0, *groove], |a| a.border),
            })
        })
        .collect()
}
fn upload(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    layer: u32,
    mut pixels: Vec<u8>,
) -> Result<(), &'static str> {
    let mut edge = EDGE;
    let mut index = 0;
    loop {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: index,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
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
            return Ok(());
        }
        pixels = mip(&pixels, edge)?;
        edge /= 2;
        index += 1;
    }
}
// Otsu's alpha threshold (the byte that best splits grooves from tops), unit contrast and both class means.
#[expect(clippy::cast_possible_truncation, reason = "Unit contrast fits f32")]
fn split(pixels: &[u8]) -> (u32, f32, f64, f64) {
    let mut count = [0.0_f64; 256];
    for texel in pixels.chunks_exact(4) {
        count[usize::from(texel[3])] += 1.0;
    }
    let total: f64 = count.iter().sum();
    let sum: f64 = (0_u32..).zip(count).map(|(b, n)| n * f64::from(b)).sum();
    let (mut weight, mut below, mut best, mut result) = (0.0, 0.0, 0.0, (0, 0.0, 0.0, 0.0));
    for (byte, n) in (0_u32..).zip(count) {
        weight += n;
        below = n.mul_add(f64::from(byte), below);
        let above = total - weight;
        if weight > 0.0 && above > 0.0 {
            let gap = (sum - below) / above - below / weight;
            if gap * gap * weight * above > best {
                best = gap * gap * weight * above;
                result = (byte + 1, gap / 255.0, below / weight, (sum - below) / above);
            }
        }
    }
    (result.0, result.1 as f32, result.2, result.3)
}
// Mean wall ramp width between the 20 % and 80 % levels of the class gap, per crossing, in quarter texels.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Clamped to six bits"
)]
fn bevel(pixels: &[u8], threshold: u32, low: f64, high: f64) -> u32 {
    let edge = EDGE as usize;
    let (texels, top) = (edge * edge, |i: usize| {
        u32::from(pixels[4 * i + 3]) >= threshold
    });
    let (mut band, mut crossings) = (0.0_f64, 0.0_f64);
    for i in 0..texels {
        let alpha = f64::from(pixels[4 * i + 3]);
        band += f64::from(u8::from(
            alpha >= 0.2_f64.mul_add(high - low, low) && alpha < 0.2_f64.mul_add(low - high, high),
        ));
        crossings += f64::from(u8::from(i % edge + 1 < edge && top(i) != top(i + 1)));
        crossings += f64::from(u8::from(i + edge < texels && top(i) != top(i + edge)));
    }
    (band / crossings.max(1.0) * 4.0).round().clamp(1.0, 63.0) as u32
}
// Quarter texels (up to 63.75) along +u, -u, +v, -v (RGBA) from a groove texel to the foot (20 % of the class gap)
// of the next rising wall, and from a top texel to the crest (80 %) of the next falling one; zero on that ramp itself.
// Interpolated crossings, mirror-extended like the sampler at tile and atlas-cell edges (neighbouring cells are
// unrelated faces), past atlas rows into a typical face half a cell over (the bond), split by Otsu class so
// filtering never mixes classes.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Runs are clamped to one byte"
)]
fn runs(
    pixels: &[u8],
    threshold: u32,
    low: f64,
    high: f64,
    atlas: Option<[u16; 2]>,
) -> (Vec<u8>, Vec<u8>) {
    let cells = atlas.map_or([1, 1], |a| a.map(usize::from));
    let edge = EDGE as usize;
    let middle = f64::from(threshold) - 0.5;
    let (foot, crest) = (
        0.2_f64.mul_add(high - low, low),
        0.2_f64.mul_add(low - high, high),
    );
    let (mut to_top, mut to_groove) = (vec![0_u8; pixels.len()], vec![0_u8; pixels.len()]);
    // A typical face: alpha averaged over atlas faces and their flips.
    let (width, rows) = (edge / cells[0], edge / cells[1]);
    let typical = atlas.map(|faces| {
        let mut face = vec![0.0_f64; width * rows];
        for (cell, x, y) in (0..cells[0] * cells[1] * width * rows)
            .map(|k| (k / (width * rows), k / rows % width, k % rows))
        {
            let at = (cell / cells[0] * edge / cells[1] + y) * edge + cell % cells[0] * width + x;
            for (x, y) in [
                (x, y),
                (width - 1 - x, y),
                (x, rows - 1 - y),
                (width - 1 - x, rows - 1 - y),
            ] {
                face[x * rows + y] += f64::from(pixels[4 * at + 3]);
            }
        }
        let share = 0.25 / f64::from(faces[0]) / f64::from(faces[1]);
        face.iter().map(|v| v * share).collect::<Vec<_>>()
    });
    for line in 0..edge {
        for (channel, start, stride, count) in
            [(0, line * edge, 1, cells[0]), (2, line, edge, cells[1])]
        {
            for (cell, size) in
                (0..count).map(|k| (k * edge / count, (k + 1) * edge / count - k * edge / count))
            {
                for (offset, reverse) in [(0, false), (1, true)] {
                    let texel = |i: usize| {
                        let i = if i < size { i } else { 2 * size - 1 - i };
                        start + stride * (cell + if reverse { size - 1 - i } else { i })
                    };
                    let alpha = |i: usize| match &typical {
                        Some(face) if channel == 2 && i >= size => {
                            face[(line % width + width / 2) % width * rows
                                + (i - size).min(rows - 1)]
                        }
                        _ => f64::from(pixels[4 * texel(i) + 3]),
                    };
                    // (distance, on that ramp): toward the next rising wall's foot and the next falling wall's crest.
                    let (mut rise, mut fall) = ((64.0_f64, false), (64.0_f64, false));
                    for i in (0..2 * size).rev() {
                        let (here, next) = (
                            alpha(i),
                            if i + 1 < 2 * size {
                                alpha(i + 1)
                            } else {
                                alpha(i)
                            },
                        );
                        let cross = |level: f64| (level - here) / (next - here);
                        rise = if here > middle {
                            (64.0, false)
                        } else if next > middle || rise.1 {
                            if here >= foot {
                                (0.0, true)
                            } else if next >= foot {
                                (cross(foot), false)
                            } else {
                                (rise.0 + 1.0, false)
                            }
                        } else {
                            (rise.0 + 1.0, false)
                        };
                        fall = if here <= middle {
                            (64.0, false)
                        } else if next <= middle || fall.1 {
                            if here < crest {
                                (0.0, true)
                            } else if next < crest {
                                (cross(crest), false)
                            } else {
                                (fall.0 + 1.0, false)
                            }
                        } else {
                            (fall.0 + 1.0, false)
                        };
                        if i < size {
                            let (out, run) = if here > middle {
                                (&mut to_groove, fall.0)
                            } else {
                                (&mut to_top, rise.0)
                            };
                            out[4 * texel(i) + channel + offset] =
                                (run * 4.0).round().min(255.0) as u8;
                        }
                    }
                }
            }
        }
    }
    (to_top, to_groove)
}
#[expect(clippy::cast_possible_truncation, reason = "Unit heights fit f32")]
fn unit(byte: f64) -> f32 {
    (byte / 255.0) as f32
}
// Median height byte: the typical brick face, unlike the mean, ignores deep mortar.
fn median_height(pixels: &[u8]) -> u32 {
    let mut count = [0_usize; 256];
    for p in pixels.chunks_exact(4) {
        count[usize::from(p[3])] += 1;
    }
    let mut below = 0;
    let byte = count.iter().position(|&n| {
        below += n;
        2 * below > pixels.len() / 4
    });
    byte.and_then(|b| u32::try_from(b).ok()).unwrap_or(255)
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
