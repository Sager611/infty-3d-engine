use wasm_bindgen::prelude::*;

use crate::diagnostic::{Recorder, TraceLog};
use crate::geometry::{Geometry, point::Request};
use crate::math::{Ray, checked_u32};
use crate::scene::{
    CameraError, QueryError, QueryMode, parse_camera, parse_objects, query_objects,
};

const SCREEN_ARITY: usize = 4;
const RAY_ARITY: usize = 6;
const PACKED_STRIDE: usize = 24;

/// Immutable validated instances, shared definitions and reusable hierarchy.
#[wasm_bindgen]
pub struct PreparedScene {
    pub(super) core: crate::scene::prepared::Prepared,
    pub(super) identity: std::rc::Rc<()>,
}

#[wasm_bindgen]
impl PreparedScene {
    /// Prepare object8N once; queries never rebuild its hierarchy.
    ///
    /// # Errors
    /// Rejects invalid packets, duplicate IDs and excessive scene capacity.
    #[wasm_bindgen(constructor)]
    pub fn new(objects: &[f64]) -> Result<Self, JsValue> {
        let objects = parse_objects(objects).map_err(scene_error)?;
        let core = crate::scene::prepared::Prepared::new(&objects).map_err(scene_error)?;
        Ok(Self {
            core,
            identity: std::rc::Rc::new(()),
        })
    }

    /// Number of immutable scene instances.
    #[must_use]
    pub fn instance_count(&self) -> usize {
        self.core.instances().len()
    }

    /// Number of shared canonical definitions.
    #[must_use]
    pub fn geometry_count(&self) -> usize {
        self.core.geometries().len()
    }

    /// Query the prepared scene with a normalized origin/direction ray6.
    ///
    /// # Errors
    /// Rejects invalid rays, modes, numerical evaluations and packed results.
    pub fn query_ray(&self, ray: &[f64], mode: f64) -> Result<Vec<f64>, JsValue> {
        let ray = parse_ray(ray)?;
        let samples = crate::scene::query::query_prepared(&self.core, ray, parse_mode(mode)?)
            .map_err(query_error)?;
        pack_samples(samples, ray)
    }

    /// Ray query with at most 256 diagnostic events; header3, query24N, then rows8.
    ///
    /// # Errors
    /// Rejects invalid trace limits and the same inputs/evaluations as `query_ray`.
    pub fn trace_ray(&self, ray: &[f64], mode: f64, max_events: f64) -> Result<Vec<f64>, JsValue> {
        let mut log = trace_log(max_events)?;
        let ray = parse_ray(ray)?;
        let samples = crate::scene::query::query_prepared_traced(
            &self.core,
            ray,
            parse_mode(mode)?,
            Some(&mut log),
        )
        .map_err(query_error)?;
        let result = pack_samples(samples, ray)?;
        log.packed(&result)
            .map_err(|error| js_error(error.to_string()))
    }

    /// Closest surface of one instance with bounded diagnostics in world units.
    ///
    /// # Errors
    /// Rejects invalid trace limits and the same inputs/evaluations as `closest_point`.
    pub fn trace_closest_point(
        &self,
        id: f64,
        point: &[f64],
        request: &[f64],
        max_events: f64,
    ) -> Result<Vec<f64>, JsValue> {
        let id = parse_id(id)?;
        let (point, request) = parse_point_request(point, request)?;
        let mut log = trace_log(max_events)?;
        let result = crate::scene::query::closest_prepared_traced(
            &self.core,
            id,
            point,
            request,
            &mut Recorder::new(Some(&mut log), id),
        )
        .map_err(query_error)?;
        log.packed(&result.packed())
            .map_err(|error| js_error(error.to_string()))
    }

    /// Closest surface of one instance by stable ID; point/accuracy are world-space.
    ///
    /// # Errors
    /// Rejects invalid/unknown IDs, malformed queries and unrepresentable arithmetic.
    pub fn closest_point(
        &self,
        id: f64,
        point: &[f64],
        request: &[f64],
    ) -> Result<Vec<f64>, JsValue> {
        let id = parse_id(id)?;
        let (point, request) = parse_point_request(point, request)?;
        let result = crate::scene::query::closest_prepared(&self.core, id, point, request)
            .map_err(query_error)?;
        Ok(result.packed().to_vec())
    }

    /// Query the prepared scene through camera6 and screen4.
    ///
    /// # Errors
    /// Rejects invalid cameras, pixels, modes, evaluations and packed results.
    pub fn query_camera(
        &self,
        camera: &[f64],
        screen: &[f64],
        mode: f64,
    ) -> Result<Vec<f64>, JsValue> {
        let camera = parse_camera(camera).map_err(camera_error)?;
        let (width, height, x, y) = parse_screen(screen)?;
        let ray = camera
            .ray(width, height, x, y)
            .map_err(camera_error)
            .and_then(normalize_ray)?;
        let samples = crate::scene::query::query_prepared(&self.core, ray, parse_mode(mode)?)
            .map_err(query_error)?;
        pack_samples(samples, ray)
    }
}

/// Query a canonical closest surface point without an instance transform.
///
/// Request3 is [absolute accuracy, relative accuracy, max evaluations].
/// Result12 is [resolution, multiplicity, has witness, lower, upper,
/// x, y, z, position error, evaluations, 0, 0]. Resolution0 is converged;
/// multiplicity0/1/2 is unique/non-unique/unknown. Unresolved upper may be infinity.
///
/// # Errors
/// Rejects malformed/invalid requests, points, kinds, parameters and
/// unrepresentable arithmetic. Budget exhaustion is not a miss.
#[wasm_bindgen]
pub fn closest_point(
    kind: f64,
    parameter: f64,
    point: &[f64],
    request: &[f64],
) -> Result<Vec<f64>, JsValue> {
    let geometry = parse_geometry(kind, parameter)?;
    let (point, request) = parse_point_request(point, request)?;
    let result = geometry
        .closest(point, request)
        .map_err(|e| js_error(e.to_string()))?;
    Ok(result.packed().to_vec())
}

/// Canonical closest query plus a bounded numerical trace, never normal-path logging.
///
/// Header3 is [result values, retained events, omitted events], then closest12
/// and rows8 [tag, object ID, six tag-specific values]. Limit is 0..=256.
///
/// # Errors
/// Rejects invalid limits and the same inputs/evaluations as `closest_point`.
#[wasm_bindgen]
pub fn trace_closest_point(
    kind: f64,
    parameter: f64,
    point: &[f64],
    request: &[f64],
    max_events: f64,
) -> Result<Vec<f64>, JsValue> {
    let geometry = parse_geometry(kind, parameter)?;
    let (point, request) = parse_point_request(point, request)?;
    let mut log = trace_log(max_events)?;
    let result = geometry
        .closest_traced(point, request, &mut Recorder::new(Some(&mut log), 0))
        .map_err(|error| js_error(error.to_string()))?;
    log.packed(&result.packed())
        .map_err(|error| js_error(error.to_string()))
}

fn parse_geometry(kind: f64, parameter: f64) -> Result<Geometry, JsValue> {
    let kind = checked_u32(kind)
        .and_then(crate::geometry::Kind::from_tag)
        .ok_or_else(|| js_error("invalid geometry kind"))?;
    Geometry::new(kind, parameter).map_err(|error| js_error(error.to_string()))
}

fn parse_id(id: f64) -> Result<u32, JsValue> {
    checked_u32(id)
        .filter(|&id| id > 0)
        .ok_or_else(|| js_error("invalid object ID"))
}

fn trace_log(max_events: f64) -> Result<TraceLog, JsValue> {
    let limit = checked_u32(max_events).ok_or_else(|| js_error("invalid trace event limit"))?;
    TraceLog::new(limit).map_err(|error| js_error(error.to_string()))
}

fn parse_point_request(point: &[f64], request: &[f64]) -> Result<([f64; 3], Request), JsValue> {
    let &[x, y, z] = point else {
        return Err(js_error("point packet must contain three values"));
    };
    let &[absolute, relative, evaluations] = request else {
        return Err(js_error("request packet must contain three values"));
    };
    let max_evaluations = checked_u32(evaluations)
        .filter(|&n| n <= 65_536)
        .ok_or_else(|| js_error("invalid evaluation count"))?;
    Ok((
        [x, y, z],
        Request {
            absolute,
            relative,
            max_evaluations,
        },
    ))
}

/// Query the validated scene through a camera pixel.
///
/// The object packet uses eight values per object. The camera packet is
/// [yaw, pitch, distance, target-x, target-y, target-z]. The screen packet is
/// [width, height, pixel-x, pixel-y]. Results contain 24 values per record.
///
/// # Errors
///
/// Returns a JavaScript error when packet arity, finiteness, dimensions,
/// pixel bounds, mode, scene validation, ray construction, query evaluation,
/// or packed output validation fails.
#[wasm_bindgen]
pub fn query_scene(
    objects: &[f64],
    camera: &[f64],
    screen: &[f64],
    mode: f64,
) -> Result<Vec<f64>, JsValue> {
    let objects = parse_objects(objects).map_err(scene_error)?;
    let camera = parse_camera(camera).map_err(camera_error)?;
    let (width, height, x, y) = parse_screen(screen)?;
    let mode = parse_mode(mode)?;
    let ray = camera
        .ray(width, height, x, y)
        .map_err(camera_error)
        .and_then(normalize_ray)?;
    pack_samples(
        query_objects(&objects, ray, mode).map_err(query_error)?,
        ray,
    )
}

/// Query the validated scene with an origin and direction ray.
///
/// The object packet uses eight values per object. The ray packet is
/// [origin-x, origin-y, origin-z, direction-x, direction-y, direction-z].
/// Results contain 24 values per record.
///
/// # Errors
///
/// Returns a JavaScript error when packet arity, finiteness, ray direction,
/// mode, scene validation, query evaluation, or packed output validation fails.
#[wasm_bindgen]
pub fn query_ray(objects: &[f64], ray: &[f64], mode: f64) -> Result<Vec<f64>, JsValue> {
    let objects = parse_objects(objects).map_err(scene_error)?;
    let ray = parse_ray(ray)?;
    let mode = parse_mode(mode)?;
    pack_samples(
        query_objects(&objects, ray, mode).map_err(query_error)?,
        ray,
    )
}

fn parse_screen(screen: &[f64]) -> Result<(u32, u32, f64, f64), JsValue> {
    if screen.len() != SCREEN_ARITY {
        return Err(js_error("screen packet must contain exactly four values"));
    }
    if screen.iter().any(|value| !value.is_finite()) {
        return Err(js_error("screen packet contains a non-finite value"));
    }

    let width = checked_dimension(screen[0], "width")?;
    let height = checked_dimension(screen[1], "height")?;
    let x = screen[2];
    let y = screen[3];
    if !(0.0..f64::from(width)).contains(&x) || !(0.0..f64::from(height)).contains(&y) {
        return Err(js_error("screen pixel is out of bounds"));
    }
    Ok((width, height, x, y))
}

fn checked_dimension(value: f64, name: &str) -> Result<u32, JsValue> {
    if value <= 0.0 || value.fract() != 0.0 || value > f64::from(u32::MAX) {
        return Err(js_error(format!("{name} must be a positive integer")));
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "The finite, positive, integral, u32-bounded value is checked above."
    )]
    let dimension = value as u32;
    Ok(dimension)
}

fn parse_ray(packet: &[f64]) -> Result<Ray, JsValue> {
    if packet.len() != RAY_ARITY {
        return Err(js_error("ray packet must contain exactly six values"));
    }
    if packet.iter().any(|value| !value.is_finite()) {
        return Err(js_error("ray packet contains a non-finite value"));
    }
    normalize_ray(Ray {
        o: [packet[0], packet[1], packet[2]],
        d: [packet[3], packet[4], packet[5]],
    })
}

fn normalize_ray(ray: Ray) -> Result<Ray, JsValue> {
    let length_squared = dot(ray.d, ray.d);
    if !length_squared.is_finite() || length_squared <= f64::EPSILON {
        return Err(js_error("ray direction must be finite and nonzero"));
    }
    let length = length_squared.sqrt();
    let direction = [ray.d[0] / length, ray.d[1] / length, ray.d[2] / length];
    if direction.iter().any(|value| !value.is_finite()) {
        return Err(js_error("ray direction normalization failed"));
    }
    Ok(Ray {
        o: ray.o,
        d: direction,
    })
}

fn parse_mode(mode: f64) -> Result<QueryMode, JsValue> {
    match checked_u32(mode) {
        Some(0) => Ok(QueryMode::First),
        Some(1) => Ok(QueryMode::All),
        Some(2) => Ok(QueryMode::Inspect),
        _ => Err(js_error("query mode must be 0, 1, or 2")),
    }
}

fn pack_samples(
    samples: Vec<crate::scene::query::SceneSample>,
    ray: Ray,
) -> Result<Vec<f64>, JsValue> {
    let mut packed = Vec::with_capacity(samples.len() * PACKED_STRIDE);
    for sample in samples {
        if sample.error.to_bits() == f64::MAX.to_bits() {
            return Err(js_error("scene query produced unresolved bounds"));
        }
        let record = sample.packed(ray);
        if record.iter().any(|value| !value.is_finite()) {
            return Err(js_error("scene query produced non-finite output"));
        }
        packed.extend_from_slice(&record);
    }
    Ok(packed)
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}

fn scene_error(error: crate::scene::SceneError) -> JsValue {
    js_error(format!("invalid scene: {error}"))
}

fn camera_error(error: CameraError) -> JsValue {
    js_error(format!("invalid camera: {error}"))
}

fn query_error(error: QueryError) -> JsValue {
    js_error(format!("scene query failed: {error}"))
}

fn js_error(message: impl AsRef<str>) -> JsValue {
    JsValue::from_str(message.as_ref())
}
