//! GPU ABI: threaded nodes, instances and interned geometry in one storage buffer.
use super::{
    model::Object,
    spatial::{Node, Spatial},
};
use bytemuck::{Pod, Zeroable};
use std::collections::{BTreeMap, BTreeSet};

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

#[expect(
    clippy::cast_possible_truncation,
    reason = "Validated scene coordinates fit f32; branch bounds are inflated in shader"
)]
pub fn pack(objects: &[Object], selected: &[u32]) -> Result<Packet, &'static str> {
    let selected: BTreeSet<_> = selected.iter().copied().collect();
    let ids: BTreeSet<_> = objects.iter().map(|o| o.id()).collect();
    if !selected.is_subset(&ids) {
        return Err("selected object id is unknown");
    }
    let tree = Spatial::new(objects);
    let count = u32::try_from(tree.nodes.len()).map_err(|_| "too many nodes")?;
    let mut geometries = BTreeMap::new();
    for object in objects {
        let key = (object.kind().tag(), object.parameter().to_bits());
        let next = count + u32::try_from(geometries.len()).map_err(|_| "too many geometries")?;
        geometries.entry(key).or_insert(next);
    }
    let mut records = Vec::with_capacity(tree.nodes.len() + geometries.len());
    for node in tree.nodes {
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
                let object = objects[slot];
                let p = object.position();
                Record {
                    identity: [
                        object.id(),
                        object.kind().tag(),
                        u32::from(selected.contains(&object.id())),
                        geometries[&(object.kind().tag(), object.parameter().to_bits())],
                    ],
                    position_size: [p[0] as f32, p[1] as f32, p[2] as f32, object.size() as f32],
                    params: [0.0; 4],
                }
            }
        });
    }
    let mut geometry_records: Vec<_> = geometries.into_iter().collect();
    geometry_records.sort_unstable_by_key(|(_, slot)| *slot);
    for ((kind, parameter), _) in geometry_records {
        records.push(Record {
            identity: [0, kind, 0, 0],
            position_size: [0.0; 4],
            params: [f64::from_bits(parameter) as f32, 0.0, 0.0, 0.0],
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
