// Main camera and compute render entry point.
@group(0) @binding(5) var<storage, read> scene_objects: array<SceneObject>;

fn empty_sample() -> SceneSample {
  return SceneSample(
    Query(false, 0.0, 0.0, vec3f(0.0), vec3f(0.0, 1.0, 0.0), vec2f(0.0)),
    vec3f(0.0), 0u, 0u, 0.0, 0.0, 2u,Pending(0.0,false),0.0,false
  );
}

// Nodes are threaded: a rejected branch jumps to its escape index.
fn branch_intersects(node: SceneObject, o: vec3f, d: vec3f, padding: f32) -> bool {
  let rounding = 8.0 * EPS * max(1.0, max(length(node.position_size.xyz), length(node.params.xyz)));
  if(!fi_finite(rounding)||!fi_finite(padding)){return true;}
  let lo = node.position_size.xyz - vec3f(padding + rounding);
  let hi = node.params.xyz + vec3f(padding + rounding);
  var near = 0.0;
  var far=0.0;var bounded=false;
  for (var axis = 0u; axis < 3u; axis++) {
    if (d[axis] == 0.0) {
      if (o[axis] < lo[axis] || o[axis] > hi[axis]) { return false; }
    } else {
      let a = (lo[axis] - o[axis]) / d[axis];
      let b = (hi[axis] - o[axis]) / d[axis];
      if(!fi_finite(a)||!fi_finite(b)){return true;}
      near=max(near,min(a,b));
      let endpoint=max(a,b);if(!bounded||endpoint<far){far=endpoint;}bounded=true;
    }
  }
  return !bounded||near<=far;
}

fn render_goursat_residual(p: vec3f, parameter: f32) -> f32 {
  let a=fma(p,p,vec3f(-1.0));
  return fma(a.x,a.x,fma(a.y,a.y,fma(a.z,a.z,-parameter)));
}
fn render_material_distance(sample: SceneSample, ray: vec3f, size: f32, parameter: f32) -> f32 {
  let original=sample.query.t;
  if (sample.kind!=2u || !sample.query.hit) { return original; }
  let p=sample.local;let gradient=goursat_gradient(p);
  let slope=dot(gradient,ray);
  if (abs(slope)<=1e-6*length(gradient)) { return original; }
  let residual=render_goursat_residual(p,parameter);
  let step=-residual/slope;
  let candidate=original+step*size;
  // Appearance-only tangent projection: never change hits, proofs, or silhouettes.
  if (abs(step)<=1e-3 && candidate>0.0 &&
      abs(render_goursat_residual(fma(vec3f(step),ray,p),parameter))<abs(residual)) {
    return candidate;
  }
  return original;
}

fn branch_padding(node: SceneObject, origin: vec3f, height: f32, display_gap: bool) -> f32 {
  let extent = max(abs(node.position_size.xyz - origin), abs(node.params.xyz - origin));
  return max(6.0 * length(extent) / (1.8 * height), select(0.0, 1.5, display_gap));
}

fn resolved_object(index: u32) -> SceneObject {
  var object = scene_objects[index];
  if(object.identity.y!=5u){object.params=vec4f(scene_objects[object.identity.w].params.xyz,object.params.w);}
  return object;
}

fn scene_needs_gap(kind: u32, display_gap: bool, selected: bool) -> bool {
  return (kind == 2u || kind == 4u || kind==5u) && (display_gap || selected);
}

fn outline_eligible(sample: SceneSample, selected: bool, thickness: f32) -> bool {
  return selected && (sample.kind == 1u || sample.kind == 2u || sample.kind == 4u || sample.kind==5u) &&
    !sample.query.hit && sample.gap_computed && sample.query.gap < thickness;
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
  let count = frame_params.screen.w;
  let display_gap = frame_params.screen.z == 1u;
  let light = vec3f(3.0, 5.0, 4.0);

  var nearest = empty_sample();
  var nearest_hit = false;
  var nearest_t=0.0;
  var nearest_object=SceneObject(vec4u(0u),vec4f(0.0),vec4f(0.0));
  var pending=Pending(0.0,false);
  var nearest_size = 1.0;
  var nearest_parameter = 0.0;
  var nearest_material = 0u;
  var nearest_anchor = vec3f(0.0);
  var outline_uv = vec2f(0.0);
  var halo_uv = vec2f(0.0);
  var halo_gap=0.0;var halo_found=false;
  var outline_found = false;
  var outline_t=0.0;
  var outline_thickness = 0.0;

  var i = 0u;
  while (i < count) {
    let record = scene_objects[i];
    if (record.identity.x == 0u) {
      // A surface witness lies inside the node. Its forward t cannot exceed
      // the farthest corner distance; expand for the full pixel outline cone.
      let padding = branch_padding(record, origin, height, display_gap);
      if (!branch_intersects(record, origin, direction, padding)) {
        i = record.identity.y;
        continue;
      }
    } else {
      let object = resolved_object(i);
      let selected = scene_selected(object);
      let need_gap = scene_needs_gap(object.identity.y, display_gap, selected);

      let sample = scene_object_precise(origin,vec3f(frame_params.origin.w,frame_params.right.w,frame_params.up.w), direction, object, need_gap);
      pending=pending_min(pending,sample.pending);
      if (sample.query.hit && (!nearest_hit || sample.query.t < nearest_t || (sample.query.t == nearest_t && sample.id < nearest.id))) {
        nearest = sample;
        nearest_object=object;
        nearest_t = sample.query.t;
        nearest_size = object.position_size.w;
        nearest_parameter = object.params.x;
        nearest_material = u32(object.params.w);
        nearest_anchor = object.position_size.xyz;
        nearest_hit = true;
      }

      if (display_gap && object.identity.y != 3u && !sample.query.hit &&
          sample.gap_computed && (!halo_found||sample.query.gap<halo_gap)) {
        halo_gap = sample.query.gap;halo_found=true;
        halo_uv = sample.query.uv;
      }
      let witness_t = sample.query.t;
      let thickness = 2.0 * witness_t * 3.0 / (1.8 * height);
      if (outline_eligible(sample, selected, thickness) && (!outline_found||witness_t<outline_t)) {
        outline_uv = sample.query.uv;
        outline_t = witness_t;
        outline_thickness = thickness;
        outline_found = true;
      }
    }
    i++;
  }

  var color = vec3f(0.96862745, 0.9647059, 0.9490196);
  let unknown=pending_before(pending,nearest_hit,nearest_t,nearest.error);
  var shadow_unknown=false;
  if (nearest_hit&&!unknown) {
    let visibility=scene_shadow(nearest,nearest_object,surface_normal(nearest.query.n,direction,nearest.kind),light,count);
    let shadow=shadow_value(visibility);shadow_unknown=visibility.uncertain;
    if (frame_params.screen.z==3u) {
      let normal=surface_normal(nearest.query.n,direction,nearest.kind);
      let material_t=render_material_distance(nearest,direction,nearest_size,nearest_parameter);
      let footprint=max(1e-30,2.0*material_t/(frame_params.forward.w*height*max(0.05,abs(dot(normal,direction)))));
      var material:MaterialSurface;
      if(nearest.kind==5u&&nearest_material==2u){material=leaf_material(nearest.local,scene_axis(nearest_object),direction,nearest_size,footprint);}
      else{material=layered_material(nearest_material,direction*material_t,normal,footprint,nearest_anchor);}
      color=material_shade(material,nearest.query.p,direction,light,shadow);
    } else {
      color = surface_shade(nearest.query, nearest.local, nearest.kind,
        direction, height, nearest_size,
        frame_params.screen.z, light, shadow);
    }
  }

  if (display_gap&&halo_found) {
    let halo = clamp(1.0 - halo_gap / 1.5, 0.0, 1.0);
    color = mix(color, vec3f(halo_uv, 1.0 - halo_uv.x), halo * halo);
  }
  if (outline_found && (!nearest_hit||outline_t<=nearest_t+outline_thickness)) {
    let neon = mix(vec3f(1.0, 0.12, 0.25), vec3f(1.0, 0.42, 0.72),
      clamp(outline_uv.x + outline_uv.y, 0.0, 1.0));
    color = mix(color, neon, 0.9);
  }
  if(unknown||shadow_unknown){color=mix(vec3f(0.96,0.78,0.28),vec3f(0.99,0.93,0.73),f32((((id.x+id.y)/5u)&1u)!=0u));}
  textureStore(pixels, vec2i(id.xy), vec4f(color, 1.0));
}
