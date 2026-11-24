//! Immutable field upload; a placeholder until a Leaf material is present.
use super::{Field, LEAF};
use crate::geometry::Error;
use wgpu::util::DeviceExt;
pub struct Heights {
    pub buffer: wgpu::Buffer,
    ready: bool,
}
impl Heights {
    pub fn empty(device: &wgpu::Device) -> Self {
        Self {
            buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("heightfield placeholder"),
                contents: &[0; 48],
                usage: wgpu::BufferUsages::STORAGE,
            }),
            ready: false,
        }
    }
    pub fn new(device: &wgpu::Device, field: Field<'_>) -> Result<Self, Error> {
        Ok(Self {
            buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("immutable heightfield"),
                contents: &field.gpu_bytes()?,
                usage: wgpu::BufferUsages::STORAGE,
            }),
            ready: true,
        })
    }
    pub const fn ready(&self) -> bool {
        self.ready
    }
    pub fn ensure(&mut self, device: &wgpu::Device, needed: bool) -> Result<bool, Error> {
        if self.ready() || !needed {
            return Ok(false);
        }
        *self = Self::new(device, LEAF)?;
        Ok(true)
    }
}
