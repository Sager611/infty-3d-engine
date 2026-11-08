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
                let p = transform.position();
                Record {
                    identity: [
                        instance.id(),
                        geometry.kind().tag(),
                        u32::from(selected.contains(&instance.id())),
                        count
                            + u32::try_from(instance.geometry())
                                .map_err(|_| "too many geometries")?,
                    ],
                    position_size: [
                        p[0] as f32,
                        p[1] as f32,
                        p[2] as f32,
                        transform.scale() as f32,
                    ],
                    params: [0.0, 0.0, 0.0, f32::from_bits(instance.material())],
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
