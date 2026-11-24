//! Lazy independent geometry/colour owners; retain dirty bindings across failed updates.
use super::Tiles;
use crate::heightfield::gpu::Heights;
#[derive(Clone, Copy)]
pub struct Demand {
    pub leaf: bool,
    pub materials: bool,
}
pub struct Resources {
    pub tiles: Tiles,
    pub heights: Heights,
    dirty: bool,
}
impl Resources {
    pub fn empty(device: &wgpu::Device) -> Self {
        Self {
            tiles: Tiles::empty(device),
            heights: Heights::empty(device),
            dirty: false,
        }
    }
    pub fn ensure(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        need: Demand,
    ) -> Result<bool, String> {
        self.dirty |= self
            .heights
            .ensure(device, need.leaf)
            .map_err(|error| error.to_string())?;
        if need.materials {
            self.dirty |= self
                .tiles
                .ensure(device, queue, need.leaf)
                .map_err(str::to_owned)?;
        }
        Ok(std::mem::take(&mut self.dirty))
    }
}
