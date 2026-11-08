use super::{
    types::Ray,
    vector::{add, cross, safe_normal, scale, sub},
};

#[derive(Clone, Copy)]
pub struct CameraBasis {
    pub origin: [f64; 3],
    pub right: [f64; 3],
    pub up: [f64; 3],
    pub forward: [f64; 3],
}

#[must_use]
pub fn basis(yaw: f64, pitch: f64, distance: f64, target: [f64; 3]) -> CameraBasis {
    let orbit = [
        pitch.cos() * yaw.sin(),
        pitch.sin(),
        pitch.cos() * yaw.cos(),
    ];
    let origin = add(target, scale(orbit, distance));
    let forward = safe_normal(sub(target, origin), [0.0, 0.0, -1.0]);
    let right = safe_normal(cross(forward, [0.0, 1.0, 0.0]), [1.0, 0.0, 0.0]);
    let up = cross(right, forward);
    CameraBasis {
        origin,
        right,
        up,
        forward,
    }
}

#[must_use]
pub fn camera_at(
    yaw: f64,
    pitch: f64,
    distance: f64,
    target: [f64; 3],
    screen: [f64; 2],
    pixel: [f64; 2],
) -> Ray {
    let CameraBasis {
        origin,
        right,
        up,
        forward,
    } = basis(yaw, pitch, distance, target);
    let [width, height] = screen;
    let sx = 2.0f64.mul_add(pixel[0], -width) / height;
    let sy = 2.0f64.mul_add(-pixel[1], height) / height;
    let direction = add(add(scale(forward, 1.8), scale(right, sx)), scale(up, sy));
    Ray {
        o: origin,
        d: safe_normal(direction, forward),
    }
}

#[cfg(test)]
#[must_use]
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

#[cfg(test)]
mod tests {
    use super::{basis, camera, camera_at};

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1.0e-12, "{a} != {b}");
    }

    fn close_v(a: [f64; 3], b: [f64; 3]) {
        for i in 0..3 {
            close(a[i], b[i]);
        }
    }

    #[test]
    fn translated_camera_uses_basis_target() {
        let target = [3.0, 2.0, -1.0];
        let b = basis(0.3, 0.2, 5.0, target);
        let center = camera_at(0.3, 0.2, 5.0, target, [200.0, 100.0], [100.0, 50.0]);
        close_v(center.o, b.origin);
        close_v(center.d, b.forward);
        close_v(
            b.origin,
            [
                (5.0 * 0.2f64.cos()).mul_add(0.3f64.sin(), target[0]),
                5.0f64.mul_add(0.2f64.sin(), target[1]),
                (5.0 * 0.2f64.cos()).mul_add(0.3f64.cos(), target[2]),
            ],
        );
    }

    #[test]
    fn camera_wrapper_preserves_legacy_target() {
        let old = camera([0.3, 0.2, 5.0, 1.0], [200.0, 100.0], [37.0, 61.0]);
        let new = camera_at(0.3, 0.2, 5.0, [0.0, 1.0, 0.0], [200.0, 100.0], [37.0, 61.0]);
        close_v(old.o, new.o);
        close_v(old.d, new.d);
    }
}
