// Main camera and compute render entry point.
@group(0) @binding(5) var<storage, read> scene_objects: array<SceneObject>;

fn empty_sample() -> SceneSample {
  return SceneSample(
    Query(false, 1e30, 0.0, vec3f(0.0), vec3f(0.0, 1.0, 0.0), vec2f(0.0)),
    vec3f(0.0), 0u, 0u, 1e30, 0.0, 0u
  );
}

fn selected_slot(slot: u32) -> bool {
  return (frame_params.selection.x & (1u << slot)) != 0u;
}

fn outline_eligible(sample: SceneSample, selected: bool, thickness: f32) -> bool {
  return selected && (sample.kind == 1u || sample.kind == 2u) &&
    !sample.query.hit && sample.status < 2u && sample.query.gap < thickness;
}

fn shadow_factor(p: vec3f, n: vec3f, size: f32, light: vec3f, count: u32) -> f32 {
  let to_light = light - p;
  let distance = length(to_light);
  if (distance <= 0.0) { return 1.0; }
  let bias = max(1e-4 * size,
    8.0 * EPS * max(1.0, length(p)));
  let origin = p + n * bias;
  let direction = to_light / distance;
  var factor = 1.0;
  for (var i = 0u; i < 16u; i++) {
    if (i < count) {
      let blocker = scene_object(origin, direction, scene_objects[i], false);
      if (blocker.query.hit
          && blocker.query.t >= 2.0 * bias
          && blocker.query.t < distance) {
        factor = 0.25;
      }
    }
  }
  return factor;
}

@compute @workgroup_size(8, 8)
fn draw(@builtin(global_invocation_id) id: vec3u) {
  let size = textureDimensions(pixels);
  if (id.x >= size.x || id.y >= size.y) { return; }

  let width = f32(frame_params.screen.x);
  let height = f32(frame_params.screen.y);
  let pixel = vec2f(f32(id.x), f32(id.y));
  let sx = (2.0 * (pixel.x + 0.5) - width) / height;
  let sy = (height - 2.0 * (pixel.y + 0.5)) / height;
  let forward = normalize(frame_params.forward.xyz);
  let direction = normalize(frame_params.forward.w * forward
    + sx * normalize(frame_params.right.xyz)
    + sy * normalize(frame_params.up.xyz));
  let origin = frame_params.origin.xyz;
  let count = min(frame_params.screen.w, 16u);
  let display_gap = frame_params.screen.z == 1u;
  let light = vec3f(3.0, 5.0, 4.0);

  var nearest = empty_sample();
  var nearest_hit = false;
  var nearest_t = 1e30;
  var nearest_size = 1.0;
  var outline_uv = vec2f(0.0);
  var halo_uv = vec2f(0.0);
  var halo_gap = 1e30;
  var outline_found = false;
  var outline_t = 1e30;
  var outline_thickness = 0.0;

  for (var i = 0u; i < 16u; i++) {
    if (i < count) {
      let object = scene_objects[i];
      let selected = selected_slot(i);
      let need_gap = object.identity.y == 2u && (display_gap || selected);

      let sample = scene_object(origin, direction, object, need_gap);
      if (sample.query.hit && sample.query.t < nearest_t) {
        nearest = sample;
        nearest_t = sample.query.t;
        nearest_size = object.position_size.w;
        nearest_hit = true;
      }

      if (display_gap && object.identity.y != 3u && !sample.query.hit &&
          sample.status < 2u && sample.query.gap < halo_gap) {
        halo_gap = sample.query.gap;
        halo_uv = sample.query.uv;
      }
      let witness_t = sample.query.t;
      let thickness = 2.0 * witness_t * 3.0 / (1.8 * height);
      if (outline_eligible(sample, selected, thickness) && witness_t < outline_t) {
        outline_uv = sample.query.uv;
        outline_t = witness_t;
        outline_thickness = thickness;
        outline_found = true;
      }
    }
  }

  var color = vec3f(0.96862745, 0.9647059, 0.9490196);
  if (nearest_hit) {
    let shadow = shadow_factor(nearest.query.p, normalize(nearest.query.n),
      nearest_size, light, count);
    color = surface_shade(nearest.query, nearest.local, nearest.kind,
      direction, height, nearest_size,
      frame_params.screen.z, light, shadow);
  }

  if (display_gap) {
    let halo = clamp(1.0 - halo_gap / 1.5, 0.0, 1.0);
    color = mix(color, vec3f(halo_uv, 1.0 - halo_uv.x), halo * halo);
  }
  if (outline_found && outline_t <= nearest_t + outline_thickness) {
    let neon = mix(vec3f(1.0, 0.12, 0.25), vec3f(1.0, 0.42, 0.72),
      clamp(outline_uv.x + outline_uv.y, 0.0, 1.0));
    color = mix(color, neon, 0.9);
  }
  textureStore(pixels, vec2i(id.xy), vec4f(color, 1.0));
}
