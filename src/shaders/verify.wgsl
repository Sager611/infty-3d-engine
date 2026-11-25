// GPU verification entry point and present pass.
struct ProbeRay {
  o: vec4f,
  d: vec4f,
};

struct ProbeResult {
  a: vec4f,
  b: vec4f,
  c: vec4f,
  d: vec4f,
};

@group(0) @binding(3) var<storage, read> rays: array<ProbeRay>;
@group(0) @binding(4) var<storage, read_write> results: array<ProbeResult>;

@compute @workgroup_size(1)
fn verify_crystal_render(@builtin(global_invocation_id) id: vec3u) {
  if (id.x >= arrayLength(&rays)) { return; }
  let input = rays[id.x];
  let kind = u32(input.o.x);
  let normal = surface_normal(input.d.xyz, vec3f(0.0, 0.0, 1.0), kind);
  results[id.x] = ProbeResult(vec4f(0.0, normal),
    vec4f(0.0), vec4f(0.0), vec4f(0.0));
}

@compute @workgroup_size(1)
fn verify_material(@builtin(global_invocation_id) id: vec3u) {
  if (id.x >= arrayLength(&rays)) { return; }
  let input=rays[id.x];
  let q=layered_material(u32(input.o.w),input.o.xyz,normalize(input.d.xyz),input.d.w);
  results[id.x]=ProbeResult(vec4f(q.albedo,q.roughness),vec4f(q.normal,q.metallic),
    vec4f(f32(arrayLength(&material_levels)),q.occlusion,
      clamp(log2(max(1e-30,input.d.w*512.0/material_levels[2u+4u*u32(input.o.w)].surface.x)),0.0,9.0),
      1.0-smoothstep(4.0,8.0,clamp(log2(max(1e-30,input.d.w*512.0/material_levels[2u+4u*u32(input.o.w)].surface.x)),0.0,9.0))),
    vec4f(material_shade(q,input.o.xyz,vec3f(0.0,0.0,-1.0),vec3f(3.0,5.0,4.0),1.0),0.0));
}
@compute @workgroup_size(1)
fn verify_studio_brushing(@builtin(global_invocation_id) id: vec3u) {
  if (id.x>=arrayLength(&rays)) { return; }
  let input=rays[id.x];let metal=select(0.0,1.0,input.o.w==0.0);
  let q=MaterialSurface(vec3f(0.56),vec3f(0.0,0.0,1.0),0.29,metal,1.0,input.o.z);
  let color=material_shade(q,vec3f(0.0),vec3f(0.0,0.0,-1.0),input.d.xyz,1.0);
  results[id.x]=ProbeResult(vec4f(color,1.0),vec4f(material_environment(q,vec3f(0.0,0.0,1.0)),0.0),vec4f(0.0),vec4f(0.0));
}
@compute @workgroup_size(1)
fn verify_material_fill(@builtin(global_invocation_id) id: vec3u) {
  if (id.x>=arrayLength(&rays)) { return; }
  results[id.x]=ProbeResult(vec4f(material_diffuse_light(rays[id.x].o.xyz),1.0),vec4f(0.0),vec4f(0.0),vec4f(0.0));
}
@compute @workgroup_size(1)
fn verify_material_variance(@builtin(global_invocation_id) id: vec3u) {
  if (id.x>=arrayLength(&rays)) { return; }
  let r=rays[id.x].o;
  let q=material_compensate(vec4f(0.0,0.0,0.0,r.x),vec4f(0.0,0.0,0.0,r.y),vec4f(0.5,0.5,0.5,r.z),r.w);
  results[id.x]=ProbeResult(q,vec4f(0.0),vec4f(0.0),vec4f(0.0));
}
@compute @workgroup_size(1)
fn verify_instance_pose(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)){return;}
  let input=rays[id.x];let object=resolved_object(u32(input.o.w));let d=normalize(input.d.xyz);
  let base=scene_object(input.o.xyz,d,object);
  let delta=scene_origin_delta(input.o.xyz,vec3f(0.0),object);
  let span=0.001*abs(dot(d,base.query.n))/max(1e-20,base.query.t);
  let sample=material_texh(base,d,object,delta,span);
  let material=texh_surface(sample,base,d,object,delta,span);
  results[id.x]=ProbeResult(vec4f(f32(base.query.hit),base.query.t,f32(sample.query.hit),sample.query.t),
    vec4f(sample.query.n,f32(outline_eligible(sample,true,0.01))),
    vec4f(sample.query.uv,0.0,0.0),vec4f(material.albedo,sample.query.gap));
}
@compute @workgroup_size(1)
fn verify_material_hit(@builtin(global_invocation_id) id: vec3u) {
  if (id.x>=arrayLength(&rays)) { return; }
  let r=rays[id.x];let d=normalize(r.d.xyz);
  var parameter=0.0;
  if(u32(r.d.w)==2u){parameter=select(1.5,16.0*fract(r.d.w),fract(r.d.w)>0.0);}
  let object=SceneObject(vec4u(1u,u32(r.d.w),0u,0u),vec4f(0.0,0.0,0.0,1.0),vec4f(parameter,0.0,0.0,r.o.w),POSE_IDENTITY);
  let base=scene_object(r.o.xyz,d,object);
  let sample=material_texh(base,d,object,r.o.xyz,0.001);let q=sample.query;
  let footprint=texh_footprint(base,d,0.001);
  let context=TexhContext(base,object,r.o.xyz+d*base.query.t,footprint);
  let height=texh_height(geometry_surface(base.local,base,object),context);
  results[id.x]=ProbeResult(vec4f(f32(q.hit),q.t,base.query.t,f32(sample.status)),vec4f(q.p,sample.error),
    vec4f(q.uv,height,f32(base.base_hit)),vec4f(q.n,f32(base.query.hit)));
}
@compute @workgroup_size(1)
fn verify_material_color(@builtin(global_invocation_id) id: vec3u) {
  if(id.x>=arrayLength(&rays)){return;}
  let r=rays[id.x];let d=normalize(r.d.xyz);
  let object=SceneObject(vec4u(1u,5u,0u,0u),vec4f(0.0,0.0,0.0,1.0),vec4f(0.0,0.0,0.0,r.o.w),POSE_IDENTITY);
  let base=scene_object(r.o.xyz,d,object);let sample=material_texh(base,d,object,r.o.xyz,0.001);
  let material=texh_surface(sample,base,d,object,r.o.xyz,0.001);let footprint=texh_footprint(base,d,0.001);
  var expected=layered_material(u32(r.o.w),vec3f(sample.query.p.xy,0.0),vec3f(0.0,0.0,1.0),footprint);
  var original=layered_material(u32(r.o.w),vec3f(base.query.p.xy,0.0),vec3f(0.0,0.0,1.0),footprint);
  if(r.o.w==2.0){
    expected=leaf_material(sample.query.uv,material.normal,1.0,footprint);
    original=leaf_material(base.query.uv,material.normal,1.0,footprint);
  }
  results[id.x]=ProbeResult(vec4f(material.albedo,f32(sample.query.hit)),vec4f(expected.albedo,sample.query.t),
    vec4f(original.albedo,0.0),vec4f(sample.query.uv,base.query.uv));
}
// Fixed object-local normal rays: camera metadata must not move relief or colour.
@compute @workgroup_size(1)
fn verify_material_anchor(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)){return;}
  let r=rays[id.x];let half=0.5*r.d.w;
  let object=SceneObject(vec4u(1u,5u,0u,0u),vec4f(r.d.xyz,1.0),vec4f(0.0,0.0,0.0,r.o.w),vec4f(0.0,sin(half),0.0,cos(half)));
  let d=scene_world_vector(object,vec3f(0.0,0.0,-1.0));
  let origin=object.position_size.xyz+scene_world_vector(object,r.o.xyz);
  let delta=scene_origin_delta(origin,vec3f(0.0),object);
  let base=scene_object(origin,d,object);
  let sample=material_texh(base,d,object,delta,0.0);
  let material=texh_surface(sample,base,d,object,delta,0.0);
  results[id.x]=ProbeResult(vec4f(material.albedo,sample.query.t-base.query.t),
    vec4f(material.normal,material.occlusion),vec4f(sample.query.uv,base.query.uv),
    vec4f(f32(sample.query.hit),sample.local));
}
@compute @workgroup_size(1)
fn verify_material_display(@builtin(global_invocation_id) id: vec3u) {
  if (id.x>=arrayLength(&rays)) { return; }
  results[id.x]=ProbeResult(vec4f(material_display(rays[id.x].o.xyz),1.0),vec4f(0.0),vec4f(0.0),vec4f(0.0));
}
@compute @workgroup_size(1)
fn verify_material_mapping(@builtin(global_invocation_id) id: vec3u) {
  if (id.x>=arrayLength(&rays)) { return; }
  let input=rays[id.x];let level=material_levels[u32(input.o.z)];
  let q=material_tile(input.o.xy,input.d.xy,level,input.o.w);
  let raw=textureSampleLevel(material_tiles,material_sampler,input.o.xy+input.d.xy,i32(level.identity.y),input.o.w);
  results[id.x]=ProbeResult(vec4f(q.color,q.height),vec4f(q.gradient,0.0,0.0),raw,vec4f(0.0));
}
@compute @workgroup_size(1)
fn verify_material_instance(@builtin(global_invocation_id) id: vec3u) {
  if (id.x >= arrayLength(&rays)) { return; }
  let object=resolved_object(u32(rays[id.x].o.x));
  results[id.x]=ProbeResult(vec4f(object.params.w,f32(object.identity.z),object.params.x,0.0),
    vec4f(0.0),vec4f(0.0),vec4f(0.0));
}

fn pack(q: Query, rho: f32) -> ProbeResult {
  let h = select(0.0, 1.0, q.hit);
  return ProbeResult(
    vec4f(h, q.t, q.gap, q.uv.x),
    vec4f(q.uv.y, q.p),
    vec4f(q.n, pureweight(q.uv, rho)),
    vec4f(q.p, rho)
  );
}

fn result(q: Query, o: vec3f, d: vec3f, rho: f32) -> ProbeResult {
  var packed = pack(q, rho);
  packed.d = vec4f(o + q.t * d, rho);
  return packed;
}

@compute @workgroup_size(1)
fn verify_goursat(@builtin(global_invocation_id) id: vec3u) {
  if (id.x >= arrayLength(&rays)) { return; }
  let ray = rays[id.x];
  let query = goursat(ray.o.xyz, ray.d.xyz, ray.o.w);
  let hit = select(0.0, 1.0, query.query.hit);
  results[id.x] = ProbeResult(
    vec4f(hit, query.query.t, query.query.gap, query.query.uv.x),
    vec4f(query.query.uv.y, query.query.p),
    vec4f(query.query.n, query.gap_lower),
    vec4f(query.error, f32(query.status), 0.0, 0.0)
  );
}

// Query24 packet, followed by eight zero padding floats for the shared harness.
@compute @workgroup_size(1)
fn verify_scene(@builtin(global_invocation_id) id: vec3u) {
  if (id.x >= arrayLength(&rays)) { return; }
  let ray = rays[id.x];
  let kind = u32(ray.o.w);
  let object = SceneObject(vec4u(7u, kind, 0u, 0u),
    vec4f(0.0, 1.0, 0.0, ray.d.w),
    vec4f(select(0.0, 1.5, kind == 2u), 0.0, 0.0, 0.0),POSE_IDENTITY);
  let direction = normalize(ray.d.xyz);
  let sample = scene_object(ray.o.xyz, direction, object);
  let q = sample.query;
  let ray_point = ray.o.xyz + q.t * direction;
  results[2u * id.x] = ProbeResult(
    vec4f(f32(sample.id), f32(kind), select(0.0, 1.0, q.hit), q.t),
    vec4f(sample.gap_lower, q.gap, q.p.xy),
    vec4f(q.p.z, q.n),
    vec4f(q.uv, ray_point.xy)
  );
  results[2u * id.x + 1u] = ProbeResult(
    vec4f(ray_point.z, sample.local),
    vec4f(sample.error, select(0.0, 1.0, (kind == 2u || kind == 4u || kind == 5u) && !q.hit), f32(sample.status), 0.0),
    vec4f(0.0), vec4f(0.0)
  );
}

@compute @workgroup_size(1)
fn verify(@builtin(global_invocation_id) id: vec3u) {
  if (id.x >= arrayLength(&rays)) { return; }
  let ray = rays[id.x];
  let o = ray.o.xyz;
  let d = ray.d.xyz;
  let height = ray.d.w;
  let sphere_q = sphere(o, d, vec3f(0.0, 1.0, 0.0), 1.0);
  let plane_q = plane(o, d);
  results[2u * id.x] = result(sphere_q, o, d, footprint(sphere_q, d, height, 1.0, true));
  results[2u * id.x + 1u] = result(plane_q, o, d, footprint(plane_q, d, height, 1.0, false));
}

@compute @workgroup_size(1)
fn verify_outline(@builtin(global_invocation_id) id: vec3u) {
  if (id.x >= arrayLength(&rays)) { return; }
  let ray = rays[id.x];
  let q = Query(ray.d.x != 0.0, 2.0, ray.o.x, vec3f(0.0), vec3f(0.0, 1.0, 0.0), vec2f(0.0));
  let sample = SceneSample(q, vec3f(0.0), 7u, u32(ray.o.w), ray.o.y, ray.o.z, u32(ray.d.w),Pending(0.0,false),ray.o.z,q.hit);
  let eligible = outline_eligible(sample, ray.d.y != 0.0, ray.d.z);
  results[id.x] = ProbeResult(vec4f(select(0.0, 1.0, eligible), 0.0, 0.0, 0.0),
    vec4f(0.0), vec4f(0.0), vec4f(0.0));
}

@compute @workgroup_size(1)
fn verify_surface(@builtin(global_invocation_id) id: vec3u) {
  if (id.x >= arrayLength(&rays)) { return; }
  let ray = rays[id.x];
  let q = Query(true, 4.0, 0.0, vec3f(1.1, 0.0, 0.2),
    vec3f(0.0, 1.0, 0.0), vec2f(0.6, 0.2));
  let direction = vec3f(0.0, -1.0, 0.0);
  let rho = surface_rho(q, vec3f(0.6, 0.0, 0.2), direction, 100.0, ray.d.w, false);
  let color = surface_shade(q, vec3f(0.6, 0.0, 0.2), 3u, direction, 100.0,
    ray.d.w, u32(ray.o.w), q.p + vec3f(0.0, 1.0, 0.0), 1.0);
  results[id.x] = ProbeResult(vec4f(rho, color), vec4f(0.0), vec4f(0.0), vec4f(0.0));
}

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  var p = array<vec2f, 3>(
    vec2f(-1.0, -1.0),
    vec2f(3.0, -1.0),
    vec2f(-1.0, 3.0)
  );
  return vec4f(p[i], 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f {
  return textureLoad(present, vec2i(p.xy), 0);
}

// ray.o.w = threaded record count; ray.d.w selects brute-force traversal.
@compute @workgroup_size(1)
fn verify_spatial(@builtin(global_invocation_id) id: vec3u) {
  let ray = rays[id.x];
  let direction = normalize(ray.d.xyz);
  var nearest = empty_sample();
  var visits = 0u;
  var i = 0u;
  while (i < u32(ray.o.w)) {
    let record = scene_objects[i];
    if (record.identity.x == 0u) {
      if (ray.d.w == 0.0 && !branch_intersects(record, ray.o.xyz, direction, 0.0)) {
        i = record.identity.y;
        continue;
      }
    } else {
      visits++;
      let sample = scene_object(ray.o.xyz, direction, resolved_object(i));
      if (sample.query.hit && (!nearest.query.hit || sample.query.t < nearest.query.t ||
          (sample.query.t == nearest.query.t && sample.id < nearest.id))) { nearest = sample; }
    }
    i++;
  }
  results[id.x] = ProbeResult(vec4f(f32(nearest.id), nearest.query.t, f32(visits), f32(nearest.kind)),
    vec4f(nearest.query.p, nearest.query.uv.x), vec4f(nearest.query.n, nearest.query.uv.y), vec4f(0.0));
}

@compute @workgroup_size(1)
fn verify_spatial_gaps(@builtin(global_invocation_id) id: vec3u) {
  let ray = rays[id.x];
  var outlines = 0.0;
  var gap = 1.5;
  var i = 0u;
  while (i < u32(ray.o.w)) {
    let record = scene_objects[i];
    if (record.identity.x == 0u) {
      let padding = branch_padding(record, ray.o.xyz, 1000.0, true);
      if (ray.d.w == 0.0 && !branch_intersects(record, ray.o.xyz, ray.d.xyz, padding)) {
        i = record.identity.y;
        continue;
      }
    } else {
      let sample = scene_object(ray.o.xyz, ray.d.xyz, resolved_object(i));
      if (!sample.query.hit && sample.kind != 3u && sample.status < 2u) {
        gap = min(gap, sample.query.gap);
      }
      let thickness = 6.0 * sample.query.t / 1800.0;
      if (outline_eligible(sample,scene_selected(record),thickness)) { outlines += 1.0; }
    }
    i++;
  }
  results[id.x] = ProbeResult(vec4f(outlines, gap, 0.0, 0.0), vec4f(0.0), vec4f(0.0), vec4f(0.0));
}

@compute @workgroup_size(1)
fn verify_crystal(@builtin(global_invocation_id) id: vec3u) {
  if (id.x >= arrayLength(&rays)) { return; }
  let ray = rays[id.x];
  let q = crystal(ray.o.xyz, ray.d.xyz);
  results[id.x] = ProbeResult(
    vec4f(select(0.0, 1.0, q.query.hit), q.query.t, q.query.gap, q.query.uv.x),
    vec4f(q.query.uv.y, q.query.p), vec4f(q.query.n, q.gap_lower),
    vec4f(q.error, f32(q.status), 0.0, 0.0));
}
@compute @workgroup_size(1)
fn verify_crystal_point(@builtin(global_invocation_id) id: vec3u) {
  if (id.x >= arrayLength(&rays)) { return; }
  let q = crystal_closest(rays[id.x].o.xyz);
  results[id.x] = ProbeResult(vec4f(q.upper, q.error, 0.0, 0.0),
    vec4f(q.p, 0.0), vec4f(q.n, 0.0), vec4f(q.uv, 0.0, 0.0));
}
