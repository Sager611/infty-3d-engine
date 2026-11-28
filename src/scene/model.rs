use std::fmt;

pub const MAX_OBJECTS: usize = 65_536;
pub const OBJECT_STRIDE: usize = 8;
const POSE_STRIDE: usize = 12;
const POSE_TAG: f64 = -1.0;

#[cfg(test)]
use super::rotation::Rotation;
use super::transform::Transform;
use crate::geometry::Geometry;
pub use crate::geometry::Kind;
use crate::{material::Material, math::checked_u32};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Object {
    id: u32,
    geometry: Geometry,
    transform: Transform,
    material: Material,
}

impl Object {
    pub fn new(
        id: u32,
        kind: Kind,
        position: [f64; 3],
        size: f64,
        parameter: f64,
    ) -> Result<Self, SceneError> {
        if id == 0 {
            return Err(SceneError::InvalidId);
        }
        let transform = Transform::new(position, size)?;
        if !parameter.is_finite() {
            return Err(SceneError::NonFinite);
        }
        let geometry = Geometry::new(kind, parameter).map_err(|_| SceneError::InvalidParameter)?;
        Ok(Self {
            id,
            geometry,
            transform,
            material: Material::Steel,
        })
    }

    pub const fn with_material(mut self, material: Material) -> Self {
        self.material = material;
        self
    }
    pub fn with_rotation(mut self, rotation: [f64; 4]) -> Result<Self, SceneError> {
        self.transform = self.transform.with_rotation(rotation)?;
        Ok(self)
    }
    pub const fn material(self) -> Material {
        self.material
    }
    pub const fn geometry(self) -> Geometry {
        self.geometry
    }
    pub const fn transform(self) -> Transform {
        self.transform
    }

    #[must_use]
    pub const fn id(self) -> u32 {
        self.id
    }

    #[must_use]
    pub const fn kind(self) -> Kind {
        self.geometry.kind()
    }

    #[must_use]
    pub const fn position(self) -> [f64; 3] {
        self.transform.position()
    }

    #[cfg(test)]
    #[must_use]
    pub const fn size(self) -> f64 {
        self.transform.scale()
    }

    #[must_use]
    pub const fn parameter(self) -> f64 {
        self.geometry.parameter()
    }
}

impl Default for Object {
    fn default() -> Self {
        Self {
            id: 1,
            geometry: Geometry::SPHERE,
            transform: Transform::IDENTITY,
            material: Material::Steel,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneError {
    InvalidLength,
    TooManyObjects,
    NonFinite,
    InvalidId,
    DuplicateId,
    InvalidKind,
    InvalidPosition,
    InvalidSize,
    InvalidRotation,
    InvalidParameter,
    InvalidMaterial,
}

impl fmt::Display for SceneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidLength => "invalid object packet length",
            Self::TooManyObjects => "object packet has too many objects",
            Self::NonFinite => "object packet contains a non-finite value",
            Self::InvalidId => "object packet contains an invalid id",
            Self::DuplicateId => "object packet contains a duplicate id",
            Self::InvalidKind => "object packet contains an invalid kind",
            Self::InvalidPosition => "object position is out of bounds",
            Self::InvalidSize => "object size is out of bounds",
            Self::InvalidRotation => "object rotation must be a finite nonzero quaternion",
            Self::InvalidParameter => "object parameter is invalid",
            Self::InvalidMaterial => "object material must be 0 (steel) or 1 (brick)",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for SceneError {}

pub fn parse_objects(packet: &[f64]) -> Result<Vec<Object>, SceneError> {
    let posed = packet.first() == Some(&POSE_TAG);
    let stride = if posed { POSE_STRIDE } else { OBJECT_STRIDE };
    let values = &packet[usize::from(posed)..];
    if !values.len().is_multiple_of(stride) {
        return Err(SceneError::InvalidLength);
    }
    let count = values.len() / stride;
    if count > MAX_OBJECTS {
        return Err(SceneError::TooManyObjects);
    }

    let mut objects = Vec::with_capacity(count);
    let mut ids = std::collections::BTreeSet::new();
    for values in values.chunks_exact(stride) {
        let id = values[0];
        let kind_tag = values[1];
        let position = [values[2], values[3], values[4]];
        let size = values[5];
        let parameter = values[6];
        let material = values[7];

        if !id.is_finite()
            || !kind_tag.is_finite()
            || position.iter().any(|value| !value.is_finite())
            || !size.is_finite()
            || !parameter.is_finite()
            || !material.is_finite()
        {
            return Err(SceneError::NonFinite);
        }
        let material = checked_u32(material)
            .and_then(Material::from_tag)
            .ok_or(SceneError::InvalidMaterial)?;
        let id = checked_u32(id)
            .filter(|&id| id > 0)
            .ok_or(SceneError::InvalidId)?;
        if !ids.insert(id) {
            return Err(SceneError::DuplicateId);
        }
        let kind = checked_u32(kind_tag)
            .and_then(Kind::from_tag)
            .ok_or(SceneError::InvalidKind)?;
        let mut object = Object::new(id, kind, position, size, parameter)?.with_material(material);
        if posed {
            object = object.with_rotation([values[8], values[9], values[10], values[11]])?;
        }
        objects.push(object);
    }
    Ok(objects)
}

#[cfg(test)]
#[must_use]
pub fn pack_objects(objects: &[Object]) -> Vec<f64> {
    let posed = objects
        .iter()
        .any(|o| o.transform().rotation() != Rotation::IDENTITY);
    let mut packet = Vec::with_capacity(
        usize::from(posed) + objects.len() * if posed { POSE_STRIDE } else { OBJECT_STRIDE },
    );
    if posed {
        packet.push(POSE_TAG);
    }
    for object in objects {
        packet.extend_from_slice(&[
            f64::from(object.id),
            f64::from(object.kind().tag()),
            object.position()[0],
            object.position()[1],
            object.position()[2],
            object.size(),
            object.parameter(),
            f64::from(object.material().tag()),
        ]);
        if posed {
            packet.extend_from_slice(&object.transform().rotation().components());
        }
    }
    packet
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotated_packets_preserve_pose_queries_and_legacy_records()
    -> Result<(), Box<dyn std::error::Error>> {
        let half = std::f64::consts::FRAC_1_SQRT_2;
        let packet = [
            -1.0, 7.0, 5.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, half, 0.0, half,
        ];
        let objects = parse_objects(&packet)?;
        assert_eq!(pack_objects(&objects), packet);
        let transform = objects[0].transform();
        let point = transform.world_point([0.0, 0.0, 1.0]);
        assert!((point[0] - 1.0).abs() < 1e-12 && point[1].abs() < 1e-12 && point[2].abs() < 1e-12);
        let hit = crate::scene::query::query_object(
            &objects[0],
            crate::math::Ray {
                o: [3.0, 0.3, 0.4],
                d: [-1.0, 0.0, 0.0],
            },
        )?;
        assert!(hit.sample.hit);
        assert!((hit.sample.t - 3.0).abs() < 1e-12);
        assert!(hit.sample.n[0] > 0.999_999);
        let closest = transform.closest(
            objects[0].geometry(),
            [2.0, 0.3, 0.4],
            crate::geometry::point::Request::default(),
        )?;
        assert!(closest.distance.lower <= 2.0 && closest.distance.upper >= 2.0);
        assert!(closest.distance.upper - closest.distance.lower < 1e-10);
        let other = Object::new(8, Kind::Sphere, [5.0, 0.0, 0.0], 0.1, 0.0)?;
        let tree = crate::scene::spatial::Spatial::new(&[objects[0], other]);
        assert!(
            tree.candidates([3.0, 0.3, 0.4], [-1.0, 0.0, 0.0])
                .contains(&0)
        );
        let legacy = valid_packet(1.0, 0.0);
        assert_eq!(pack_objects(&parse_objects(&legacy)?), legacy);
        let mut invalid = packet;
        invalid[9..].fill(0.0);
        assert!(parse_objects(&invalid).is_err());
        Ok(())
    }

    fn valid_packet(kind: f64, parameter: f64) -> Vec<f64> {
        vec![7.0, kind, -0.0, 2.0, -3.0, 1.0, parameter, 0.0]
    }

    #[test]
    fn thousands_of_mixed_instances_round_trip() -> Result<(), SceneError> {
        let packet: Vec<_> = (1..=4096)
            .flat_map(|id| {
                let kind = if id % 2 == 0 { 2.0 } else { 1.0 };
                [
                    f64::from(id),
                    kind,
                    0.0,
                    0.0,
                    0.0,
                    0.1,
                    if id % 2 == 0 { 1.5 } else { 0.0 },
                    0.0,
                ]
            })
            .collect();
        assert_eq!(pack_objects(&parse_objects(&packet)?), packet);
        Ok(())
    }

    #[test]
    fn defaults_are_a_valid_sphere() {
        let object = Object::default();
        assert_eq!(object.id(), 1);
        assert_eq!(object.kind(), Kind::Sphere);
        assert_eq!(
            object.position().map(f64::to_bits),
            [0.0; 3].map(f64::to_bits)
        );
        assert_eq!(object.size().to_bits(), 1.0f64.to_bits());
        assert_eq!(object.parameter().to_bits(), 0.0f64.to_bits());
    }

    #[test]
    fn empty_is_valid_and_round_trips() -> Result<(), SceneError> {
        let objects = parse_objects(&[])?;
        assert!(objects.is_empty());
        assert_eq!(pack_objects(&objects), Vec::<f64>::new());
        Ok(())
    }

    #[test]
    fn all_kinds_round_trip_and_normalize_signed_zero() -> Result<(), SceneError> {
        let packet = [
            1.0, 1.0, -0.0, 1.0, 2.0, 1.0, 0.0, 0.0, 2.0, 2.0, 3.0, 4.0, 5.0, 0.5, 1.5, 0.0, 3.0,
            3.0, 6.0, 7.0, 8.0, 2.0, -0.0, 0.0, 4.0, 5.0, 0.0, 0.0, 0.0, 1.0, -0.0, 0.0,
        ];
        let objects = parse_objects(&packet)?;
        assert_eq!(objects.len(), 4);
        assert_eq!(objects[3].kind(), Kind::FinitePlane);
        assert_eq!(objects[3].parameter().to_bits(), 0.0f64.to_bits());
        assert_eq!(objects[0].position()[0].to_bits(), 0.0f64.to_bits());
        assert_eq!(objects[2].parameter().to_bits(), 0.0f64.to_bits());
        assert_eq!(
            pack_objects(&objects),
            packet
                .into_iter()
                .enumerate()
                .map(|(i, x)| if matches!(i, 2 | 22 | 30) { 0.0 } else { x })
                .collect::<Vec<_>>()
        );
        Ok(())
    }

    #[test]
    fn constructor_normalization_is_idempotent() -> Result<(), SceneError> {
        let first = Object::new(1, Kind::Sphere, [-0.0, 1.0, 0.0], 1.0, -0.0)?;
        let second = Object::new(
            first.id(),
            first.kind(),
            first.position(),
            first.size(),
            first.parameter(),
        )?;
        assert_eq!(first, second);
        assert_eq!(pack_objects(&[first]), pack_objects(&[second]));
        Ok(())
    }

    #[test]
    fn invalid_classes_are_rejected() {
        assert!(matches!(
            parse_objects(&[1.0; 7]),
            Err(SceneError::InvalidLength)
        ));
        assert!(matches!(
            parse_objects(&[f64::NAN, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
            Err(SceneError::NonFinite)
        ));
        assert!(matches!(
            parse_objects(&[0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
            Err(SceneError::InvalidId)
        ));
        assert!(matches!(
            parse_objects(&[1.5, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
            Err(SceneError::InvalidId)
        ));
        assert!(matches!(
            parse_objects(&[
                1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0
            ]),
            Err(SceneError::DuplicateId)
        ));
        assert!(matches!(
            parse_objects(&[1.0, 6.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
            Err(SceneError::InvalidKind)
        ));
        assert!(matches!(
            parse_objects(&[1.0, 1.0, 20.1, 0.0, 0.0, 1.0, 0.0, 0.0]),
            Err(SceneError::InvalidPosition)
        ));
        assert!(matches!(
            parse_objects(&[1.0, 1.0, 0.0, 0.0, 0.0, 0.009, 0.0, 0.0]),
            Err(SceneError::InvalidSize)
        ));
        assert!(matches!(
            parse_objects(&[1.0, 2.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0]),
            Err(SceneError::InvalidParameter)
        ));
        assert!(matches!(
            parse_objects(&[1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0]),
            Err(SceneError::InvalidParameter)
        ));
        assert!(matches!(
            parse_objects(&[1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 3.0]),
            Err(SceneError::InvalidMaterial)
        ));
    }

    #[test]
    fn bounds_and_count_are_rejected_without_clamping() {
        assert!(matches!(
            Object::new(1, Kind::Sphere, [20.1, 0.0, 0.0], 1.0, 0.0),
            Err(SceneError::InvalidPosition)
        ));
        assert!(matches!(
            Object::new(1, Kind::Sphere, [0.0; 3], 4.1, 0.0),
            Err(SceneError::InvalidSize)
        ));
        let packet = (1..=65_537)
            .flat_map(|id| [f64::from(id), 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0])
            .collect::<Vec<_>>();
        assert!(matches!(
            parse_objects(&packet),
            Err(SceneError::TooManyObjects)
        ));
    }

    #[test]
    fn parsing_does_not_mutate_source() -> Result<(), SceneError> {
        let mut packet = valid_packet(1.0, 0.0);
        let original = packet.clone();
        parse_objects(&packet)?;
        assert_eq!(packet, original);
        packet[2] = 21.0;
        assert!(parse_objects(&packet).is_err());
        assert_eq!(packet[2].to_bits(), 21.0f64.to_bits());
        Ok(())
    }

    #[test]
    fn checked_id_cast_keeps_u32_boundary_exact() -> Result<(), SceneError> {
        let mut packet = valid_packet(1.0, 0.0);
        packet[0] = f64::from(u32::MAX);
        assert_eq!(parse_objects(&packet)?[0].id(), u32::MAX);
        packet[0] += 1.0;
        assert_eq!(parse_objects(&packet), Err(SceneError::InvalidId));
        Ok(())
    }

    #[test]
    fn display_is_human_and_error_has_no_any_payload() {
        assert_eq!(parse_objects(&[1.0; 7]), Err(SceneError::InvalidLength));
        assert_eq!(
            SceneError::InvalidLength.to_string(),
            "invalid object packet length"
        );
    }
}
