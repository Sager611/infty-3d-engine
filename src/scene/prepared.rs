//! Immutable shared definitions, instances, ID lookup and conservative hierarchy.
use super::{
    model::{MAX_OBJECTS, Object, SceneError},
    spatial::Spatial,
    transform::Transform,
};
use crate::{
    geometry::{Error, Geometry, Kind},
    material::Material,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub struct Instance {
    id: u32,
    geometry: usize,
    transform: Transform,
    material: u32,
}
impl Instance {
    pub const fn id(self) -> u32 {
        self.id
    }
    pub const fn geometry(self) -> usize {
        self.geometry
    }
    pub const fn transform(self) -> Transform {
        self.transform
    }
    pub const fn material(self) -> u32 {
        self.material
    }
}
pub struct Prepared {
    geometries: Box<[Geometry]>,
    instances: Box<[Instance]>,
    spatial: Spatial,
    by_id: BTreeMap<u32, usize>,
    leaf: bool,
    leaf_finish: bool,
}
impl Prepared {
    pub fn new(objects: &[Object]) -> Result<Self, SceneError> {
        if objects.len() > MAX_OBJECTS {
            return Err(SceneError::TooManyObjects);
        }
        let mut instances = Vec::with_capacity(objects.len());
        let mut geometries = Vec::new();
        let mut interned = BTreeMap::new();
        let mut by_id = BTreeMap::new();
        let mut leaf = false;
        let mut leaf_finish = false;
        for (index, object) in objects.iter().enumerate() {
            if by_id.insert(object.id(), index).is_some() {
                return Err(SceneError::DuplicateId);
            }
            leaf_finish |= object.material() == Material::Leaf;
            leaf |= object.kind() == Kind::FinitePlane && object.material() == Material::Leaf;
            let key = (object.kind().tag(), object.parameter().to_bits());
            let geometry = *interned.entry(key).or_insert_with(|| {
                let slot = geometries.len();
                geometries.push(object.geometry());
                slot
            });
            instances.push(Instance {
                id: object.id(),
                geometry,
                transform: object.transform(),
                material: u32::from(object.material().tag()),
            });
        }
        Ok(Self {
            geometries: geometries.into_boxed_slice(),
            instances: instances.into_boxed_slice(),
            spatial: Spatial::new(objects),
            by_id,
            leaf,
            leaf_finish,
        })
    }
    pub const fn has_leaf_finish(&self) -> bool {
        self.leaf_finish
    }
    pub const fn has_leaf(&self) -> bool {
        self.leaf
    }
    pub fn surface(&self, instance: Instance) -> Result<Geometry, Error> {
        Ok(self
            .geometries
            .get(instance.geometry)
            .copied()
            .ok_or(Error::Numeric)?
            .with_leaf_surface(instance.material == u32::from(Material::Leaf.tag())))
    }
    pub fn geometries(&self) -> &[Geometry] {
        &self.geometries
    }
    pub fn instances(&self) -> &[Instance] {
        &self.instances
    }
    pub const fn spatial(&self) -> &Spatial {
        &self.spatial
    }
    pub fn find(&self, id: u32) -> Option<&Instance> {
        self.by_id.get(&id).map(|&slot| &self.instances[slot])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Kind;
    #[test]
    fn shared_definitions_and_replacements_do_not_mutate_peers() -> Result<(), SceneError> {
        let mut objects = [
            Object::new(10, Kind::Goursat, [0.0; 3], 1.0, 1.5)?,
            Object::new(20, Kind::Goursat, [4.0, 0.0, 0.0], 0.5, 1.5)?,
            Object::new(30, Kind::Sphere, [0.0, 2.0, 0.0], 1.0, 0.0)?,
        ];
        let before = Prepared::new(&objects)?;
        assert_eq!(before.geometries().len(), 2);
        assert_eq!(before.instances().len(), 3);
        let first = before.instances()[0];
        let second = before.instances()[1];
        assert_eq!(first.geometry(), second.geometry());
        assert_eq!(second.id(), 20);
        assert_eq!(second.material(), 0);
        assert_eq!(second.transform().scale().to_bits(), 0.5_f64.to_bits());
        assert_eq!(before.find(20).map(|i| i.id()), Some(20));
        assert!(before.find(99).is_none());
        assert_eq!(before.spatial().nodes.len(), 5);
        objects[1] = Object::new(20, Kind::Goursat, [4.0, 0.0, 0.0], 0.5, 1.7)?;
        let after = Prepared::new(&objects)?;
        assert_eq!(after.geometries().len(), 3);
        assert_eq!(
            before.geometries()[first.geometry()].parameter().to_bits(),
            1.5_f64.to_bits()
        );
        assert_eq!(
            after.geometries()[after.instances()[0].geometry()]
                .parameter()
                .to_bits(),
            1.5_f64.to_bits()
        );
        assert_eq!(
            after.geometries()[after.instances()[1].geometry()]
                .parameter()
                .to_bits(),
            1.7_f64.to_bits()
        );
        Ok(())
    }
    #[test]
    fn empty_duplicate_and_capacity_cases_are_explicit() -> Result<(), SceneError> {
        let empty = Prepared::new(&[])?;
        assert!(empty.instances().is_empty() && empty.geometries().is_empty());
        assert!(empty.spatial().nodes.is_empty());
        assert!(matches!(
            Prepared::new(&[Object::default(); 2]),
            Err(SceneError::DuplicateId)
        ));
        assert!(matches!(
            Prepared::new(&vec![Object::default(); MAX_OBJECTS + 1]),
            Err(SceneError::TooManyObjects)
        ));
        Ok(())
    }
}
