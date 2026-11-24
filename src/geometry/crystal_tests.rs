use super::*;
use crate::diagnostic::TraceLog;
use point::{Multiplicity, Request, Resolution};

#[test]
fn crystal_catalogue_and_budget() -> Result<(), Error> {
    let c = Geometry::new(Kind::CrystallineCluster, -0.0)?;
    assert_eq!(Kind::from_tag(4), Some(c.kind()));
    assert_eq!(c.kind().tag(), 4);
    assert_eq!(Kind::from_tag(6), None);
    assert_eq!(c.parameter().to_bits(), 0);
    assert_eq!(c.radius().map(f64::to_bits), Some(1.75_f64.to_bits()));
    assert!(Geometry::new(c.kind(), 1.0).is_err());
    assert!(Geometry::new(c.kind(), f64::NAN).is_err());
    let short = c.closest(
        [0.0; 3],
        Request {
            max_evaluations: 26,
            ..Request::default()
        },
    )?;
    assert_eq!(short.resolution, Resolution::Unresolved);
    assert!(short.witness.is_none());
    assert_eq!(short.evaluations, 0);
    let apex = c.closest(
        [0.0; 3],
        Request {
            max_evaluations: 27,
            ..Request::default()
        },
    )?;
    assert_eq!(apex.resolution, Resolution::Converged);
    assert_eq!(apex.multiplicity, Multiplicity::Unique);
    assert_eq!(apex.evaluations, 27);
    Ok(())
}

#[test]
fn crystal_numerical_limits_are_not_false_certificates() -> Result<(), Error> {
    let c = Geometry::new(Kind::CrystallineCluster, 0.0)?;
    let tiny = c.closest(
        [f64::from_bits(1), 0.0, 0.0],
        Request {
            absolute: f64::from_bits(1),
            relative: 0.0,
            max_evaluations: 27,
        },
    )?;
    assert_eq!(tiny.resolution, Resolution::Unresolved);
    assert_eq!(tiny.multiplicity, Multiplicity::Unknown);
    assert!(tiny.distance.upper.is_finite());
    assert_eq!(
        c.closest([f64::MAX; 3], Request::default()).err(),
        Some(Error::Numeric)
    );
    assert!(
        c.closest([f64::INFINITY, 0.0, 0.0], Request::default())
            .is_err()
    );
    Ok(())
}

#[test]
fn crystal_trace_is_bounded_optional_and_result_neutral() -> Result<(), Box<dyn std::error::Error>>
{
    let c = Geometry::new(Kind::CrystallineCluster, 0.0)?;
    let ray = Ray {
        o: [-3.0; 3],
        d: [-1.0 / 3.0_f64.sqrt(); 3],
    };
    let plain = c.query(ray)?;
    let mut log = TraceLog::new(1)?;
    let traced = c.query_traced(ray, &mut Recorder::new(Some(&mut log), 7))?;
    c.query_traced(ray, &mut Recorder::new(Some(&mut log), 7))?;
    assert_eq!(
        plain.sample.packed(ray, 0.0).map(f64::to_bits),
        traced.sample.packed(ray, 0.0).map(f64::to_bits)
    );
    assert_eq!(plain.gap_lower.to_bits(), traced.gap_lower.to_bits());
    assert_eq!(plain.error.to_bits(), traced.error.to_bits());
    assert_eq!(plain.uncertain, traced.uncertain);
    let packet = log.packed(&[])?;
    assert_eq!(packet.len(), 11);
    assert_eq!(packet[1].to_bits(), 1.0_f64.to_bits());
    assert_eq!(packet[2].to_bits(), 1.0_f64.to_bits());
    assert_eq!(
        packet[10].to_bits(),
        f64::from(u8::from(plain.uncertain)).to_bits()
    );
    let p = [1.2, -0.4, 0.9];
    let ordinary = c.closest(p, Request::default())?;
    let traced = c.closest_traced(p, Request::default(), &mut Recorder::new(Some(&mut log), 7))?;
    assert_eq!(
        ordinary.packed().map(f64::to_bits),
        traced.packed().map(f64::to_bits)
    );
    assert_eq!(log.packed(&[])?.len(), 11);
    Ok(())
}
