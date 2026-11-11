//! GPU ABI: threaded nodes, instances and interned geometry in one storage buffer.
#[cfg(test)]
use super::model::Object;
use super::{prepared::Prepared, spatial::Node};
use bytemuck::{Pod, Zeroable};
use std::collections::BTreeSet;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Record {
    pub identity: [u32; 4],
    pub position_size: [f32; 4],
    pub params: [f32; 4],
}

const _: () = assert!(std::mem::size_of::<Record>() == 48);

pub struct Packet {
    pub records: Vec<Record>,
    pub count: u32,
}

#[cfg(test)]
pub fn pack(objects: &[Object], selected: &[u32]) -> Result<Packet, String> {
    let scene = Prepared::new(objects).map_err(|error| error.to_string())?;
    pack_prepared(&scene, selected).map_err(str::to_owned)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "Validated scene coordinates fit f32; branch bounds are inflated in shader"
)]
pub fn pack_prepared(scene: &Prepared, selected: &[u32]) -> Result<Packet, &'static str> {
    let selected: BTreeSet<_> = selected.iter().copied().collect();
    if selected.iter().any(|&id| scene.find(id).is_none()) {
        return Err("selected object id is unknown");
    }
    let tree = scene.spatial();
    let count = u32::try_from(tree.nodes.len()).map_err(|_| "too many nodes")?;
    let mut records = Vec::with_capacity(tree.nodes.len() + scene.geometries().len());
    for &node in &tree.nodes {
        records.push(match node {
            Node::Branch { bounds, escape } => Record {
                identity: [
                    0,
                    u32::try_from(escape).map_err(|_| "too many nodes")?,
                    0,
                    0,
                ],
                position_size: [
                    bounds.min[0] as f32,
                    bounds.min[1] as f32,
                    bounds.min[2] as f32,
                    0.0,
                ],
                params: [
                    bounds.max[0] as f32,
                    bounds.max[1] as f32,
                    bounds.max[2] as f32,
                    0.0,
                ],
            },
            Node::Leaf(slot) => {
                let instance = scene.instances()[slot];
                let geometry = scene.geometries()[instance.geometry()];
                let transform = instance.transform();
                let (high, low) = crate::math::types::split32(transform.position())
                    .ok_or("object exceeds GPU coordinates")?;
                let finite = geometry.kind() == crate::geometry::Kind::FinitePlane;
                let low = if finite { low } else { [0.0; 3] };
                let axis = if finite {
                    match geometry.parameter() {
                        0.0 => 0,
                        1.0 => 256,
                        _ => 512,
                    }
                } else {
                    0
                };
                Record {
                    identity: [
                        instance.id(),
                        geometry.kind().tag(),
                        u32::from(selected.contains(&instance.id())) | axis,
                        count
                            + u32::try_from(instance.geometry())
                                .map_err(|_| "too many geometries")?,
                    ],
                    position_size: [high[0], high[1], high[2], transform.scale() as f32],
                    params: [
                        low[0],
                        low[1],
                        low[2],
                        f32::from(
                            u8::try_from(instance.material()).map_err(|_| "invalid material")?,
                        ),
                    ],
                }
            }
        });
    }
    for geometry in scene.geometries() {
        records.push(Record {
            identity: [0, geometry.kind().tag(), 0, 0],
            position_size: [0.0; 4],
            params: [geometry.parameter() as f32, 0.0, 0.0, 0.0],
        });
    }
    // WebGPU storage bindings cannot be empty.
    if records.is_empty() {
        records.push(Record::zeroed());
    }
    Ok(Packet { records, count })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::model::Kind;

    #[test]
    fn finite_instance_low_parts_preserve_shared_carriers_and_axis_bits()
    -> Result<(), Box<dyn std::error::Error>> {
        let position = [1.000_000_031_25, 0.031_250_000_01, -1.000_000_012_5];
        let a = Object::new(7, Kind::FinitePlane, position, 0.07, 2.0)?
            .with_material(crate::material::Material::Leaf);
        let b = Object::new(8, Kind::FinitePlane, [2.0, 0.0, 0.0], 0.07, 2.0)?;
        let packet = pack(&[a, b], &[7])?;
        assert_eq!(packet.records.len(), 4);
        let leaf = packet
            .records
            .iter()
            .find(|row| row.identity[0] == 7)
            .ok_or("instance")?;
        assert_eq!(leaf.identity[2] & 1, 1);
        assert_eq!(leaf.identity[2] >> 8, 2);
        assert_eq!(leaf.params[3].to_bits(), 2_f32.to_bits());
        for (axis, value) in position.into_iter().enumerate() {
            let restored = f64::from(leaf.position_size[axis]) + f64::from(leaf.params[axis]);
            assert!((restored - value).abs() < 2e-14, "lost instance low part");
        }
        Ok(())
    }
    #[test]
    fn shared_geometry_and_selection_are_not_slot_masks() -> Result<(), Box<dyn std::error::Error>>
    {
        let objects = (1..=4096)
            .map(|id| Object::new(id, Kind::Sphere, [0.0; 3], 0.1, 0.0))
            .collect::<Result<Vec<_>, _>>()?;
        let packet = pack(&objects, &[33, 4096, 4096])?;
        assert_eq!(packet.records.len(), 8192);
        assert_eq!(packet.count, 8191);
        let leaves: Vec<_> = packet
            .records
            .iter()
            .filter(|r| r.identity[0] != 0)
            .collect();
        assert_eq!(leaves.len(), 4096);
        assert_eq!(leaves.iter().filter(|r| r.identity[2] == 1).count(), 2);
        assert!(leaves.iter().all(|r| r.identity[3] == 8191));
        assert!(pack(&objects, &[4097]).is_err());
        Ok(())
    }
}
