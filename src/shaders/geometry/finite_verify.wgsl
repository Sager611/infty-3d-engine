// Bounded-kernel GPU probes share the public verification buffers.
@compute @workgroup_size(1)
fn verify_finite_branch(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)){return;}let input=rays[id.x];
  let node=SceneObject(vec4u(0u),vec4f(-1.0),vec4f(1.0));
  results[id.x]=ProbeResult(vec4f(f32(branch_intersects(node,input.o.xyz,input.d.xyz,0.0)),0.0,0.0,0.0),
    vec4f(0.0),vec4f(0.0),vec4f(0.0));
}
@compute @workgroup_size(1)
fn verify_finite_shadow(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)){return;}
  let input=rays[id.x];let object=resolved_object(u32(input.o.w));
  var height=0.0;if(object.params.w==2.0){height=hf_sample(input.o.xy).scalar;}
  let sample=scene_canonical(vec3f(input.o.xy,height+input.o.z),vec3f(0.0,0.0,-1.0),object,false);
  let shadow=scene_shadow(sample,object,sample.query.n,input.d.xyz,u32(input.d.w));
  results[id.x]=ProbeResult(vec4f(f32(sample.query.hit),shadow_value(shadow),f32(shadow.uncertain),f32(shadow.blocked)),vec4f(0.0),vec4f(0.0),vec4f(0.0));
}
@compute @workgroup_size(1)
fn verify_leaf_material(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)){return;}
  let input=rays[id.x];let axis=u32(input.o.w);let value=hf_sample(input.o.xy);
  let local=plane_unframe(vec3f(input.o.xy,value.scalar),axis);
  let material=leaf_material(local,axis,input.d.xyz,input.d.w,input.o.z);
  results[id.x]=ProbeResult(vec4f(material.normal,material.roughness),vec4f(material.albedo,material.metallic),
    vec4f(value.height.v,value.gradient),vec4f(
      leaf_fetch(leaf_uv(input.o.xy,0u),0u,8,0.0).a,
      leaf_fetch(leaf_uv(input.o.xy,3u),3u,11,0.0).a,0.0,0.0));
}
@compute @workgroup_size(1)
fn verify_finite_precise(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)/2u){return;}
  let input=rays[2u*id.x];let low=rays[2u*id.x+1u].o.xyz;let object=resolved_object(0u);
  let sample=scene_object_precise(input.o.xyz,low,input.d.xyz,object,true);
  results[id.x*2u]=ProbeResult(vec4f(f32(sample.query.hit),sample.query.t,sample.error,f32(sample.status)),
    vec4f(sample.query.p,sample.local_error),vec4f(f32(scene_selected(object)),sample.query.n),vec4f(sample.local,0.0));
  results[id.x*2u+1u]=ProbeResult(vec4f(0.0),vec4f(0.0),vec4f(0.0),vec4f(0.0));
}
@compute @workgroup_size(1)
fn verify_finite_pending(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)){return;}
  let input=rays[id.x];
  let unknown=pending_before(Pending(input.o.x,input.o.y>0.0),input.o.z>0.0,input.o.w,input.d.x);
  results[id.x]=ProbeResult(vec4f(f32(unknown),0.0,0.0,0.0),
    vec4f(f32(scene_needs_gap(5u,false,true)),f32(scene_needs_gap(5u,true,false)),
      f32(scene_needs_gap(5u,false,false)),surface_normal(vec3f(0.0,0.0,1.0),vec3f(0.0,0.0,1.0),5u).z),
    vec4f(0.0),vec4f(0.0));
}
@compute @workgroup_size(1)
fn verify_finite_scene(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)){return;}
  let input=rays[id.x];
  let object=SceneObject(vec4u(7u,5u,u32(input.o.w)<<8u,0u),vec4f(0.0,0.0,0.0,1.0),vec4f(0.0,0.0,0.0,input.d.w));
  let direction=normalize(input.d.xyz);let sample=scene_object(input.o.xyz,direction,object,true);let q=sample.query;
  let at=input.o.xyz+q.t*direction;
  results[id.x*2u]=ProbeResult(vec4f(7.0,5.0,f32(q.hit),q.t),vec4f(sample.gap_lower,q.gap,q.p.xy),
    vec4f(q.p.z,q.n),vec4f(q.uv,at.xy));
  results[id.x*2u+1u]=ProbeResult(vec4f(at.z,sample.local),
    vec4f(sample.error,f32(!q.hit),f32(sample.status),0.0),
    vec4f(sample.pending.lower,f32(sample.pending.valid),f32(sample.gap_computed),sample.local_error),vec4f(0.0));
}

@compute @workgroup_size(1)
fn verify_height_ray(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)){return;}
  let input=rays[id.x];let axis=u32(input.o.w);
  let query=hf_query(plane_frame(input.o.xyz,axis),plane_frame(input.d.xyz,axis),
    u32(clamp(input.d.w,0.0,64.0)));
  let hit=query.contact;let point=plane_unframe(hit.p,axis);let normal=plane_unframe(hit.n,axis);
  var coverage=fi_invalid();if(hit.found){coverage=hf_sample(hit.p.xy).mask;}
  results[id.x*2u]=ProbeResult(vec4f(f32(hit.found),fi_middle(hit.time),hit.time.v),
    vec4f(point,hit.error),vec4f(normal,f32(query.visits)),
    vec4f(0.5*hit.p.xy+vec2f(0.5),f32(query.complete),f32(query.pending_valid)));
  results[id.x*2u+1u]=ProbeResult(vec4f(query.pending,coverage.v,0.0),vec4f(0.0),vec4f(0.0),vec4f(0.0));
}

@compute @workgroup_size(1)
fn verify_heightfield(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)){return;}
  let input=rays[id.x];var value=HFValue(fi_invalid(),0.0,vec2f(0.0),fi_invalid());
  var bounds=HFBounds(vec3f(0.0),vec3f(0.0),true,false);
  if(hf_header_ok()){
    value=hf_sample(input.o.xy);bounds=hf_bounds(vec3u(u32(input.d.x),u32(input.d.y),u32(input.o.w)));
  }
  let normal=normalize(vec3f(-value.gradient,1.0));
  results[id.x*2u]=ProbeResult(vec4f(fi_middle(value.height),value.height.v,fi_middle(value.mask)),
    vec4f(value.gradient,value.mask.v),vec4f(normal,f32(value.height.valid&&value.mask.valid)),
    vec4f(bounds.lo,f32(bounds.covered)));
  results[id.x*2u+1u]=ProbeResult(vec4f(bounds.hi,f32(bounds.valid)),vec4f(height_data.info),
    vec4f(0.0),vec4f(0.0));
}

@compute @workgroup_size(1)
fn verify_finite_interval(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)){return;}
  let a=fi_exact(rays[id.x].o.x);let b=fi_exact(rays[id.x].o.y);
  let sum=fi_add(a,b);let product=fi_mul(a,b);let quotient=fi_div(a,b);
  results[id.x]=ProbeResult(vec4f(sum.v,f32(sum.valid),product.v.x),
    vec4f(product.v.y,f32(product.valid),quotient.v),
    vec4f(f32(quotient.valid),0.0,0.0,0.0),vec4f(0.0));
}
