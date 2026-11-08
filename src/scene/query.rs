use crate::geometry::{Error as GeometryError, Geometry};
use crate::math::{Ray, Sample};
use crate::scene::{
    model::{Kind, Object, SceneError},
    prepared::Prepared,
    transform::Transform,
};

/// Which subset of the scene query results to return.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryMode {
    First,
    All,
    Inspect,
}

/// A canonical-shape sample transformed back into world space.
#[derive(Clone, Copy)]
pub struct SceneSample {
    pub id: u32,
    pub kind: Kind,
    pub sample: Sample,
    pub local: [f64; 3],
    pub gap_lower: f64,
    pub error: f64,
    pub uncertain: bool,
}

/// Errors raised before or during a deterministic scene query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryError {
    InvalidRay,
    Scene(SceneError),
    Geometry(GeometryError),
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRay => formatter.write_str("ray origin/direction is invalid"),
            Self::Scene(error) => error.fmt(formatter),
            Self::Geometry(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for QueryError {}

impl SceneSample {
    /// Pack this sample using the locked 24-value query layout.
    #[must_use]
    pub fn packed(self, ray: Ray) -> [f64; 24] {
        let ray = normalized_ray(ray);
        let hit = if self.sample.hit { 1.0 } else { 0.0 };
        let gap_upper = if self.sample.hit {
            0.0
        } else {
            self.sample.gap
        };
        [
            f64::from(self.id),
            f64::from(self.kind.tag()),
            hit,
            self.sample.t,
            self.gap_lower,
            gap_upper,
            self.sample.p[0],
            self.sample.p[1],
            self.sample.p[2],
            self.sample.n[0],
            self.sample.n[1],
            self.sample.n[2],
            self.sample.uv[0],
            self.sample.uv[1],
            self.sample.t.mul_add(ray.d[0], ray.o[0]),
            self.sample.t.mul_add(ray.d[1], ray.o[1]),
            self.sample.t.mul_add(ray.d[2], ray.o[2]),
            self.local[0],
            self.local[1],
            self.local[2],
            self.error,
            if !self.sample.hit && self.kind == Kind::Goursat {
                1.0
            } else {
                0.0
            },
            if self.uncertain { 1.0 } else { 0.0 },
            0.0,
        ]
    }
}

/// Query one object. The ray is normalized once, then transformed to the
/// object's canonical unit-size coordinates.
#[cfg(test)]
pub fn query_object(object: &Object, ray: Ray, need_gap: bool) -> Result<SceneSample, QueryError> {
    validate_ray(ray)?;
    query_definition(
        object.id(),
        object.geometry(),
        object.transform(),
        normalized_ray(ray),
        need_gap,
    )
}

fn query_definition(
    id: u32,
    geometry: Geometry,
    transform: Transform,
    ray: Ray,
    need_gap: bool,
) -> Result<SceneSample, QueryError> {
    let result = geometry
        .query(transform.local_ray(ray), need_gap)
        .map_err(QueryError::Geometry)?;
    let local = result.sample.p;
    let world = transform
        .world_query(result)
        .map_err(QueryError::Geometry)?;
    Ok(SceneSample {
        id,
        kind: geometry.kind(),
        sample: world.sample,
        local,
        gap_lower: world.gap_lower,
        error: world.error,
        uncertain: world.uncertain,
    })
}

/// Query a scene without mutating the object slice.
pub fn query_objects(
    objects: &[Object],
    ray: Ray,
    mode: QueryMode,
) -> Result<Vec<SceneSample>, QueryError> {
    validate_ray(ray)?;
    let scene = Prepared::new(objects).map_err(QueryError::Scene)?;
    query_prepared(&scene, ray, mode)
}

/// Reuse an immutable scene's definitions and hierarchy; normalize once.
pub fn query_prepared(
    scene: &Prepared,
    ray: Ray,
    mode: QueryMode,
) -> Result<Vec<SceneSample>, QueryError> {
    validate_ray(ray)?;
    let ray = normalized_ray(ray);
    let need_gap = mode == QueryMode::Inspect;
    let candidates = if need_gap {
        (0..scene.instances().len()).collect()
    } else {
        scene.spatial().candidates(ray.o, ray.d)
    };
    let mut samples = candidates
        .into_iter()
        .map(|index| {
            let instance = scene.instances()[index];
            query_definition(
                instance.id(),
                scene.geometries()[instance.geometry()],
                instance.transform(),
                ray,
                need_gap,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    match mode {
        QueryMode::First | QueryMode::All => {
            samples.retain(|sample| sample.sample.hit);
            samples.sort_by(compare_world);
            if mode == QueryMode::First {
                samples.truncate(1);
            }
        }
        QueryMode::Inspect => samples.sort_by(compare_inspect),
    }
    Ok(samples)
}

fn validate_ray(ray: Ray) -> Result<(), QueryError> {
    if ray
        .o
        .iter()
        .chain(ray.d.iter())
        .any(|value| !value.is_finite())
    {
        return Err(QueryError::InvalidRay);
    }
    let length_squared = dot(ray.d, ray.d);
    if !length_squared.is_finite() || length_squared <= f64::EPSILON {
        return Err(QueryError::InvalidRay);
    }
    Ok(())
}

fn normalized_ray(ray: Ray) -> Ray {
    let length = dot(ray.d, ray.d).sqrt();
    Ray {
        o: ray.o,
        d: [ray.d[0] / length, ray.d[1] / length, ray.d[2] / length],
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}

fn compare_world(a: &SceneSample, b: &SceneSample) -> std::cmp::Ordering {
    a.sample
        .t
        .total_cmp(&b.sample.t)
        .then_with(|| a.id.cmp(&b.id))
}

fn compare_inspect(a: &SceneSample, b: &SceneSample) -> std::cmp::Ordering {
    b.sample
        .hit
        .cmp(&a.sample.hit)
        .then_with(|| a.sample.t.total_cmp(&b.sample.t))
        .then_with(|| a.id.cmp(&b.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::model::Object;

    fn ray(o: [f64; 3], d: [f64; 3]) -> Ray {
        Ray { o, d }
    }

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn sphere(
        id: u32,
        position: [f64; 3],
        size: f64,
    ) -> Result<Object, crate::scene::model::SceneError> {
        Object::new(id, Kind::Sphere, position, size, 0.0)
    }

    #[test]
    fn translated_scaled_hit_keeps_local_witness() -> TestResult {
        let object = sphere(7, [0.0, 0.0, 1.0], 2.0)?;
        let sample = query_object(&object, ray([0.0, 0.0, -5.0], [0.0, 0.0, 1.0]), false)?;
        assert!(sample.sample.hit);
        assert_eq!(sample.sample.t.to_bits(), 4.0f64.to_bits());
        assert_eq!(
            sample.local.map(f64::to_bits),
            [0.0, 0.0, -1.0].map(f64::to_bits)
        );
        assert_eq!(
            sample.sample.p.map(f64::to_bits),
            [0.0, 0.0, -1.0].map(f64::to_bits)
        );
        assert_eq!(sample.sample.gap.to_bits(), 0.0f64.to_bits());
        Ok(())
    }

    #[test]
    fn tangent_goursat_hit_packs_valid_uncertainty_status_in_first_and_all() -> TestResult {
        let object = Object::new(1, Kind::Goursat, [0.0, 0.0, 0.0], 1.0, 1.125)?;
        let ray = ray([0.5, 0.5, -3.0], [0.0, 0.0, 1.0]);

        for mode in [QueryMode::First, QueryMode::All] {
            let samples = query_objects(&[object], ray, mode)?;
            assert_eq!(samples.len(), 1);
            let sample = samples[0];
            assert!(sample.sample.hit);
            assert!((sample.sample.t - 2.0).abs() <= 1.0e-12);
            let packed = sample.packed(ray);
            assert!(packed.iter().all(|value| value.is_finite()));
            assert_eq!(packed[2].to_bits(), 1.0f64.to_bits());
            assert!([0.0f64.to_bits(), 1.0f64.to_bits()].contains(&packed[22].to_bits()));
        }
        Ok(())
    }

    #[test]
    fn first_and_all_sort_by_world_t_then_id() -> TestResult {
        let objects = [
            sphere(2, [0.0, 0.0, 5.0], 1.0)?,
            sphere(1, [0.0, 0.0, 2.0], 1.0)?,
        ];
        let ray = ray([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let all = query_objects(&objects, ray, QueryMode::All)?;
        assert_eq!(
            all.iter().map(|sample| sample.id).collect::<Vec<_>>(),
            [1, 2]
        );
        let first = query_objects(&objects, ray, QueryMode::First)?;
        assert_eq!(
            first.iter().map(|sample| sample.id).collect::<Vec<_>>(),
            [1]
        );
        let tied = [
            sphere(4, [0.0, 0.0, 2.0], 1.0)?,
            sphere(3, [0.0, 0.0, 2.0], 1.0)?,
        ];
        let tied = query_objects(&tied, ray, QueryMode::All)?;
        assert_eq!(
            tied.iter().map(|sample| sample.id).collect::<Vec<_>>(),
            [3, 4]
        );
        Ok(())
    }

    #[test]
    fn inspect_is_deterministic_and_empty_or_miss_is_valid() -> TestResult {
        let objects = [
            sphere(2, [0.0, 0.0, 2.0], 1.0)?,
            sphere(1, [4.0, 0.0, 0.0], 1.0)?,
        ];
        let result = query_objects(
            &objects,
            ray([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            QueryMode::Inspect,
        )?;
        assert_eq!(
            result.iter().map(|sample| sample.id).collect::<Vec<_>>(),
            [2, 1]
        );
        assert!(query_objects(&[], ray([0.0; 3], [1.0, 0.0, 0.0]), QueryMode::Inspect)?.is_empty());
        let miss = query_objects(
            &[sphere(1, [4.0, 0.0, 0.0], 1.0)?],
            ray([0.0; 3], [0.0, 0.0, 1.0]),
            QueryMode::All,
        )?;
        assert!(miss.is_empty());
        Ok(())
    }

    #[test]
    fn invalid_ray_and_duplicate_ids_are_explicit() -> TestResult {
        let object = sphere(1, [0.0; 3], 1.0)?;
        assert!(matches!(
            query_objects(&[object, object], ray([0.0; 3], [0.0; 3]), QueryMode::All),
            Err(QueryError::InvalidRay)
        ));
        assert!(matches!(
            query_object(&object, ray([0.0; 3], [0.0; 3]), false),
            Err(QueryError::InvalidRay)
        ));
        assert!(matches!(
            query_objects(
                &[object, object],
                ray([0.0; 3], [1.0, 0.0, 0.0]),
                QueryMode::All
            ),
            Err(QueryError::Scene(SceneError::DuplicateId))
        ));
        Ok(())
    }

    #[test]
    fn packed_quartic_bounds_enclose_axis_hole_gap() -> Result<(), Box<dyn std::error::Error>> {
        let object = Object::new(1, Kind::Goursat, [0.0; 3], 1.0, 1.5)?;
        let ray = ray([0.0, 0.0, -3.0], [0.0, 0.0, 1.0]);
        let sample = query_object(&object, ray, true)?;
        let packet = sample.packed(ray);
        let true_gap = (2.0 - 3.0f64.sqrt()).sqrt();
        assert!(!sample.sample.hit);
        assert!(
            packet[4] <= true_gap && true_gap <= packet[5],
            "gap {true_gap} outside [{}, {}], error {}",
            packet[4],
            packet[5],
            packet[20]
        );
        assert!((packet[5] - true_gap).abs() <= 2.0e-4);
        assert_eq!(packet[5].to_bits(), sample.sample.gap.to_bits());
        Ok(())
    }

    #[test]
    fn packed_layout_is_finite() -> TestResult {
        let object = sphere(1, [0.0; 3], 1.0)?;
        let ray = ray([0.0, 0.0, -3.0], [0.0, 0.0, 2.0]);
        let packet = query_object(&object, ray, false)?.packed(ray);
        assert!(packet.iter().all(|value| value.is_finite()));
        assert_eq!(packet.len(), 24);
        assert_eq!(
            [packet[14], packet[15], packet[16]].map(f64::to_bits),
            [0.0, 0.0, -1.0].map(f64::to_bits)
        );
        Ok(())
    }
}
