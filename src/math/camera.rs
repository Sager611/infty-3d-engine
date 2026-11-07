use super::{
    types::Ray,
    vector::{add, cross, safe_normal, scale, sub},
};

pub fn camera(view: [f64; 4], size: [f64; 2], pixel: [f64; 2]) -> Ray {
    let [yaw, pitch, distance, radius] = view;
    let [width, height] = size;
    let target = [0.0, radius, 0.0];
    let orbit = [
        pitch.cos() * yaw.sin(),
        pitch.sin(),
        pitch.cos() * yaw.cos(),
    ];
    let origin = add(target, scale(orbit, distance));
    let forward = safe_normal(sub(target, origin), [0.0, 0.0, -1.0]);
    let right = safe_normal(cross(forward, [0.0, 1.0, 0.0]), [1.0, 0.0, 0.0]);
    let up = cross(right, forward);
    let sx = 2.0f64.mul_add(pixel[0], -width) / height;
    let sy = 2.0f64.mul_add(-pixel[1], height) / height;
    let direction = add(add(scale(forward, 1.8), scale(right, sx)), scale(up, sy));
    Ray {
        o: origin,
        d: safe_normal(direction, forward),
    }
}
