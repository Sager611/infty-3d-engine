use super::{Closest, DistanceBounds, Error, Multiplicity, Request, Resolution, Witness};
use crate::math::crystal;

pub(super) fn query(point: [f64; 3], request: Request) -> Result<Closest, Error> {
    if request.max_evaluations < crystal::EVALUATIONS {
        return Ok(Closest::unknown());
    }
    let result = crystal::closest(point)?;
    Ok(Closest {
        distance: DistanceBounds {
            lower: result.lower,
            upper: result.upper,
        },
        witness: Some(Witness {
            point: result.p,
            error: result.error,
        }),
        resolution: Resolution::Unresolved,
        multiplicity: if point[0] == 0.0 && point[1] == 0.0 && point[2] == 0.0 {
            Multiplicity::Unique
        } else {
            Multiplicity::Unknown
        },
        evaluations: crystal::EVALUATIONS,
    })
}
