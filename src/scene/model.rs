use std::fmt;

pub const MAX_OBJECTS: usize = 65_536;
pub const OBJECT_STRIDE: usize = 8;

use crate::geometry::Geometry;
pub use crate::geometry::Kind;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Object {
    id: u32,
    geometry: Geometry,
    position: [f64; 3],
    size: f64,
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
        validate_position(position)?;
        validate_size(size)?;
        validate_parameter(kind, parameter)?;
        let geometry = Geometry::new(kind, parameter).map_err(|_| SceneError::InvalidParameter)?;
        Ok(Self {
            id,
            geometry,
            position: position.map(normalize_zero),
            size: normalize_zero(size),
        })
    }

    pub const fn geometry(self) -> Geometry {
        self.geometry
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
        self.position
    }

    #[must_use]
    pub const fn size(self) -> f64 {
        self.size
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
            position: [0.0; 3],
            size: 1.0,
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
    InvalidParameter,
    InvalidPadding,
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
            Self::InvalidParameter => "object parameter is invalid",
            Self::InvalidPadding => "object packet padding must be zero",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for SceneError {}

pub fn parse_objects(packet: &[f64]) -> Result<Vec<Object>, SceneError> {
    if !packet.len().is_multiple_of(OBJECT_STRIDE) {
        return Err(SceneError::InvalidLength);
    }
    let count = packet.len() / OBJECT_STRIDE;
    if count > MAX_OBJECTS {
        return Err(SceneError::TooManyObjects);
    }

    let mut objects = Vec::with_capacity(count);
    let mut ids = std::collections::BTreeSet::new();
    for values in packet.chunks_exact(OBJECT_STRIDE) {
        let id = values[0];
        let kind_tag = values[1];
        let position = [values[2], values[3], values[4]];
        let size = values[5];
        let parameter = values[6];
        let padding = values[7];

        if !id.is_finite()
            || !kind_tag.is_finite()
            || position.iter().any(|value| !value.is_finite())
            || !size.is_finite()
            || !parameter.is_finite()
            || !padding.is_finite()
        {
            return Err(SceneError::NonFinite);
        }
        if padding != 0.0 {
            return Err(SceneError::InvalidPadding);
        }
        if id < 1.0 || id > f64::from(u32::MAX) || id.fract() != 0.0 {
            return Err(SceneError::InvalidId);
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "Finite, integral u32 range validated above."
        )]
        let id = id as u32;
        if !ids.insert(id) {
            return Err(SceneError::DuplicateId);
        }
        if kind_tag.fract() != 0.0 || !(1.0..=3.0).contains(&kind_tag) {
            return Err(SceneError::InvalidKind);
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "Integral tag in 1..=3 validated above."
        )]
        let kind = Kind::from_tag(kind_tag as u32).ok_or(SceneError::InvalidKind)?;
        let object = Object::new(id, kind, position, size, parameter)?;
        objects.push(object);
    }
    Ok(objects)
}

#[cfg(test)]
#[must_use]
pub fn pack_objects(objects: &[Object]) -> Vec<f64> {
    let mut packet = Vec::with_capacity(objects.len() * OBJECT_STRIDE);
    for object in objects {
        packet.extend_from_slice(&[
            f64::from(object.id),
            f64::from(object.kind().tag()),
            object.position[0],
            object.position[1],
            object.position[2],
            object.size,
            object.parameter(),
            0.0,
        ]);
    }
    packet
}

fn validate_position(position: [f64; 3]) -> Result<(), SceneError> {
    if position.iter().any(|value| !value.is_finite()) {
        return Err(SceneError::NonFinite);
    }
    if position.iter().any(|value| !(-20.0..=20.0).contains(value)) {
        return Err(SceneError::InvalidPosition);
    }
    Ok(())
}

fn validate_size(size: f64) -> Result<(), SceneError> {
    if !size.is_finite() {
        return Err(SceneError::NonFinite);
    }
    if !(0.01..=4.0).contains(&size) {
        return Err(SceneError::InvalidSize);
    }
    Ok(())
}

fn validate_parameter(kind: Kind, parameter: f64) -> Result<(), SceneError> {
    if !parameter.is_finite() {
        return Err(SceneError::NonFinite);
    }
    match kind {
        Kind::Goursat => {
            if (1.05..=1.95).contains(&parameter) {
                Ok(())
            } else {
                Err(SceneError::InvalidParameter)
            }
        }
        Kind::Sphere | Kind::Plane => {
            if parameter == 0.0 {
                Ok(())
            } else {
                Err(SceneError::InvalidParameter)
            }
        }
    }
}

fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            3.0, 6.0, 7.0, 8.0, 2.0, -0.0, 0.0,
        ];
        let objects = parse_objects(&packet)?;
        assert_eq!(objects.len(), 3);
        assert_eq!(objects[0].position()[0].to_bits(), 0.0f64.to_bits());
        assert_eq!(objects[2].parameter().to_bits(), 0.0f64.to_bits());
        assert_eq!(
            pack_objects(&objects),
            packet
                .into_iter()
                .enumerate()
                .map(|(i, x)| if i == 2 || i == 22 { 0.0 } else { x })
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
            parse_objects(&[1.0, 4.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
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
            parse_objects(&[1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0]),
            Err(SceneError::InvalidPadding)
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
