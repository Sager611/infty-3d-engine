use wasm_bindgen::prelude::*;

use crate::math::Ray;
use crate::scene::{
    CameraError, QueryError, QueryMode, parse_camera, parse_objects, query_objects,
};

const SCREEN_ARITY: usize = 4;
const RAY_ARITY: usize = 6;
const PACKED_STRIDE: usize = 24;

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
    mode: u32,
) -> Result<Vec<f64>, JsValue> {
    let objects = parse_objects(objects).map_err(scene_error)?;
    let camera = parse_camera(camera).map_err(camera_error)?;
    let (width, height, x, y) = parse_screen(screen)?;
    let mode = parse_mode(mode)?;
    let ray = camera
        .ray(width, height, x, y)
        .map_err(camera_error)
        .and_then(normalize_ray)?;
    collect_query(&objects, ray, mode)
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
pub fn query_ray(objects: &[f64], ray: &[f64], mode: u32) -> Result<Vec<f64>, JsValue> {
    let objects = parse_objects(objects).map_err(scene_error)?;
    let ray = parse_ray(ray)?;
    let mode = parse_mode(mode)?;
    collect_query(&objects, ray, mode)
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

fn parse_mode(mode: u32) -> Result<QueryMode, JsValue> {
    match mode {
        0 => Ok(QueryMode::First),
        1 => Ok(QueryMode::All),
        2 => Ok(QueryMode::Inspect),
        _ => Err(js_error("query mode must be 0, 1, or 2")),
    }
}

fn collect_query(
    objects: &[crate::scene::Object],
    ray: Ray,
    mode: QueryMode,
) -> Result<Vec<f64>, JsValue> {
    let samples = query_objects(objects, ray, mode).map_err(query_error)?;
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
