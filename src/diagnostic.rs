//! Caller-owned bounded numerical records; disabled recorders allocate nothing.
use std::fmt;

const MAX_EVENTS: u32 = 256;
const STRIDE: usize = 8;

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum Tag {
    Enclosure = 1,
    Root,
    Bracket,
    Reject,
    Ray,
    WorldRay,
    Bounds,
    WorldBounds,
    Patch,
    Traversal,
}

struct Record {
    tag: Tag,
    object: u32,
    values: [f64; 6],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceError;
impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid trace event limit or packet size")
    }
}
impl std::error::Error for TraceError {}

pub struct TraceLog {
    limit: usize,
    records: Vec<Record>,
    dropped: u32,
}
impl TraceLog {
    pub fn new(limit: u32) -> Result<Self, TraceError> {
        if limit > MAX_EVENTS {
            return Err(TraceError);
        }
        let limit = usize::from(u16::try_from(limit).map_err(|_| TraceError)?);
        let mut records = Vec::new();
        records.try_reserve_exact(limit).map_err(|_| TraceError)?;
        Ok(Self {
            limit,
            records,
            dropped: 0,
        })
    }
    /// Header3 = [result values, retained events, omitted events], then result and rows8.
    pub fn packed(&self, result: &[f64]) -> Result<Vec<f64>, TraceError> {
        let length = f64::from(u32::try_from(result.len()).map_err(|_| TraceError)?);
        let count = f64::from(u32::try_from(self.records.len()).map_err(|_| TraceError)?);
        let capacity = result
            .len()
            .checked_add(3 + STRIDE * self.records.len())
            .ok_or(TraceError)?;
        let mut out = Vec::new();
        out.try_reserve_exact(capacity).map_err(|_| TraceError)?;
        out.extend([length, count, f64::from(self.dropped)]);
        out.extend_from_slice(result);
        for record in &self.records {
            out.extend([f64::from(record.tag as u8), f64::from(record.object)]);
            out.extend(record.values);
        }
        Ok(out)
    }
}

pub struct Recorder<'a> {
    output: Option<&'a mut TraceLog>,
    object: u32,
}
impl<'a> Recorder<'a> {
    pub const fn new(output: Option<&'a mut TraceLog>, object: u32) -> Self {
        Self { output, object }
    }
    pub const fn off() -> Self {
        Self::new(None, 0)
    }
    pub const fn enabled(&self) -> bool {
        self.output.is_some()
    }
    pub fn emit(&mut self, tag: Tag, values: [f64; 6]) {
        if let Some(output) = self.output.as_deref_mut() {
            if output.records.len() < output.limit {
                output.records.push(Record {
                    tag,
                    object: self.object,
                    values,
                });
            } else {
                output.dropped = output.dropped.saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Geometry, Kind, point::Request};

    #[test]
    fn trace_limits_keep_a_prefix_and_report_every_omission() -> Result<(), TraceError> {
        let tags = [
            Tag::Enclosure,
            Tag::Root,
            Tag::Bracket,
            Tag::Reject,
            Tag::Ray,
            Tag::WorldRay,
            Tag::Bounds,
            Tag::WorldBounds,
            Tag::Patch,
            Tag::Traversal,
        ];
        for limit in [0, 1, MAX_EVENTS] {
            let mut log = TraceLog::new(limit)?;
            let mut recorder = Recorder::new(Some(&mut log), u32::MAX);
            for i in 0..300_u16 {
                recorder.emit(tags[usize::from(i) % tags.len()], [f64::from(i); 6]);
            }
            let result = [7.0; 12];
            let packed = log.packed(&result)?;
            assert_eq!(
                log.records.len(),
                usize::from(u16::try_from(limit).map_err(|_| TraceError)?)
            );
            assert_eq!(log.dropped, 300 - limit);
            assert_eq!(packed.len(), 15 + STRIDE * log.records.len());
            assert_eq!(packed[0].to_bits(), 12.0_f64.to_bits());
            assert_eq!(&packed[3..15], &result);
            if limit > 0 {
                assert_eq!(packed[15].to_bits(), 1.0_f64.to_bits());
                assert_eq!(packed[16].to_bits(), f64::from(u32::MAX).to_bits());
            }
        }
        assert!(TraceLog::new(MAX_EVENTS + 1).is_err());
        assert!(TraceLog::new(u32::MAX).is_err());
        Recorder::off().emit(Tag::Bounds, [0.0; 6]);
        let mut saturated = TraceLog::new(0)?;
        saturated.dropped = u32::MAX;
        Recorder::new(Some(&mut saturated), 0).emit(Tag::Root, [0.0; 6]);
        assert_eq!(saturated.dropped, u32::MAX);
        Ok(())
    }

    #[test]
    fn large_quartic_hole_misses_are_uncomputed_not_numeric_errors()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::{
            math::Ray,
            scene::{
                model::Object,
                prepared::Prepared,
                query::{QueryMode, query_prepared, query_prepared_traced},
            },
        };
        let ray = Ray {
            o: [0.0, 0.0, -6.0],
            d: [0.0, 0.0, 1.0],
        };
        for scale in [2.0, 4.0, 1.0] {
            let scene = Prepared::new(&[Object::new(42, Kind::Goursat, [0.0; 3], scale, 1.5)?])?;
            assert!(query_prepared(&scene, ray, QueryMode::All)?.is_empty());
            let mut log = TraceLog::new(MAX_EVENTS)?;
            assert!(query_prepared_traced(&scene, ray, QueryMode::All, Some(&mut log))?.is_empty());
            let row = log
                .records
                .iter()
                .find(|row| matches!(row.tag, Tag::WorldRay))
                .ok_or("no evaluated instance")?;
            assert_eq!(
                row.values[5].to_bits(),
                2.0_f64.to_bits(),
                "uncomputed gap reported as valid"
            );
        }
        Ok(())
    }

    #[test]
    fn prepared_ray_trace_keeps_real_brackets_ids_and_world_units()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::{
            math::Ray,
            scene::{
                model::Object,
                prepared::Prepared,
                query::{QueryMode, query_prepared, query_prepared_traced},
            },
        };
        let object = Object::new(42, Kind::Goursat, [2.0, -3.0, 4.0], 0.5, 1.5625)?;
        let scene = Prepared::new(&[object])?;
        let ray = Ray {
            o: [0.5, -2.5, 4.5],
            d: [1.0, 0.0, 0.0],
        };
        let normal = query_prepared(&scene, ray, QueryMode::All)?;
        let mut log = TraceLog::new(MAX_EVENTS)?;
        let actual = query_prepared_traced(&scene, ray, QueryMode::All, Some(&mut log))?;
        assert_eq!(actual.len(), 1);
        assert_eq!(
            actual[0].packed(ray).map(f64::to_bits),
            normal[0].packed(ray).map(f64::to_bits)
        );
        assert!((actual[0].sample.t - 0.75).abs() < 2e-12);
        assert!(
            log.records
                .iter()
                .any(|row| row.object == 42 && matches!(row.tag, Tag::Bracket))
        );
        assert!(log.records.iter().any(|row| row.object == 42
            && matches!(row.tag, Tag::WorldRay)
            && (row.values[1] - 0.75).abs() < 2e-12));
        Ok(())
    }

    #[test]
    fn prepared_closest_trace_keeps_final_world_accuracy_separate()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::scene::{
            model::Object,
            prepared::Prepared,
            query::{closest_prepared, closest_prepared_traced},
        };
        let scene = Prepared::new(&[Object::new(42, Kind::Goursat, [2.0, -3.0, 4.0], 0.5, 1.5)?])?;
        let point = [2.0, -3.0, 4.0];
        let normal = closest_prepared(&scene, 42, point, Request::default())?;
        let mut log = TraceLog::new(MAX_EVENTS)?;
        let actual = closest_prepared_traced(
            &scene,
            42,
            point,
            Request::default(),
            &mut Recorder::new(Some(&mut log), 42),
        )?;
        assert_eq!(
            actual.packed().map(f64::to_bits),
            normal.packed().map(f64::to_bits)
        );
        assert!(log.records.iter().any(|row| matches!(row.tag, Tag::Bounds)));
        let world = log
            .records
            .iter()
            .find(|row| matches!(row.tag, Tag::WorldBounds))
            .ok_or("no world bounds")?;
        assert_eq!(world.object, 42);
        assert_eq!(world.values[0].to_bits(), actual.distance.lower.to_bits());
        assert_eq!(world.values[1].to_bits(), actual.distance.upper.to_bits());
        Ok(())
    }

    #[test]
    fn opt_in_closest_trace_preserves_results_and_explains_refinement()
    -> Result<(), Box<dyn std::error::Error>> {
        let geometry = Geometry::new(Kind::Goursat, 1.5)?;
        let point = [0.3, 0.9, 1.3];
        let request = Request::default();
        let normal = geometry.closest(point, request)?.packed().map(f64::to_bits);
        for limit in [0, 1, MAX_EVENTS] {
            let mut log = TraceLog::new(limit)?;
            let actual =
                geometry.closest_traced(point, request, &mut Recorder::new(Some(&mut log), 42))?;
            assert_eq!(actual.packed().map(f64::to_bits), normal);
            assert!(
                log.dropped > 0 || !log.records.is_empty(),
                "query produced no decisions"
            );
            if limit == MAX_EVENTS {
                assert!(log.records.iter().any(|row| matches!(row.tag, Tag::Bounds)));
                assert!(log.records.iter().any(|row| matches!(row.tag, Tag::Patch)));
            }
        }
        Ok(())
    }
}
