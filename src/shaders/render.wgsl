// Main camera and compute render entry point.
fn camera_origin(focus: vec3f, yaw: f32, pitch: f32, distance: f32) -> vec3f {
  return focus + distance * vec3f(cos(pitch) * sin(yaw), sin(pitch), cos(pitch) * cos(yaw));
}

@compute @workgroup_size(8, 8)
fn draw(@builtin(global_invocation_id) id: vec3u) {
  let size = textureDimensions(pixels);
  if (id.x >= size.x || id.y >= size.y) { return; }

  let width = params.screen.x;
  let height = params.screen.y;
  let radius = params.view.w;
  let focus = vec3f(0.0, radius, 0.0);
  let o = camera_origin(focus, params.view.x, params.view.y, params.view.z);
  let f = normalize(focus - o);
  let right = normalize(cross(f, vec3f(0.0, 1.0, 0.0)));
  let up = cross(right, f);
  let pixel = vec2f(f32(id.x), f32(id.y));
  let sx = (2.0 * (pixel.x + 0.5) - width) / height;
  let sy = (height - 2.0 * (pixel.y + 0.5)) / height;
  let d = normalize(1.8 * f + sx * right + sy * up);

  let sq = sphere(o, d, vec3f(0.0, radius, 0.0), radius);
  let pq = plane(o, d);
  var q = sq;
  var is_sphere = true;
  if (!sq.hit || (pq.hit && pq.t < sq.t)) {
    q = pq;
    is_sphere = false;
  }

  let light = vec3f(3.0, 5.0, 4.0);
  var color = vec3f(0.96862745, 0.9647059, 0.9490196);
  if (q.hit) {
    color = shade(q, is_sphere, light, params.screen.z, d, params.screen.y, params.view.w);
  }
  if (params.screen.z == 1.0 && !sq.hit) {
    let halo = clamp(1.0 - sq.gap / 1.5, 0.0, 1.0);
    let uv_color = vec3f(sq.uv.x, sq.uv.y, 1.0 - sq.uv.x);
    color = mix(color, uv_color, halo * halo);
  }
  textureStore(pixels, vec2i(id.xy), vec4f(color, 1.0));
}
