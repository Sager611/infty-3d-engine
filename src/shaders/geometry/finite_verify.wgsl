// Bounded-kernel GPU probes share the public verification buffers.
@compute @workgroup_size(1)
fn verify_finite_branch(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)){return;}let input=rays[id.x];
  let node=SceneObject(vec4u(0u),vec4f(-1.0),vec4f(1.0),POSE_IDENTITY);
  results[id.x]=ProbeResult(vec4f(f32(branch_intersects(node,input.o.xyz,input.d.xyz,0.0)),0.0,0.0,0.0),
    vec4f(0.0),vec4f(0.0),vec4f(0.0));
}
@compute @workgroup_size(1)
fn verify_finite_shadow(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)){return;}
  let input=rays[id.x];let object=resolved_object(u32(input.o.w));
  let sample=scene_canonical(vec3f(input.o.xy,input.o.z),vec3f(0.0,0.0,-1.0),object);
  let shadow=scene_shadow(sample,object,sample.query.n,input.d.xyz,u32(input.d.w));
  results[id.x]=ProbeResult(vec4f(f32(sample.query.hit),shadow_value(shadow),f32(shadow.uncertain),f32(shadow.blocked)),vec4f(0.0),vec4f(0.0),vec4f(0.0));
}
@compute @workgroup_size(1)
fn verify_leaf_material(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)){return;}
  let input=rays[id.x];let axis=u32(input.o.w);
  let uv=fma(input.o.xy,vec2f(0.5),vec2f(0.5));
  let normal=surface_normal(plane_unframe(vec3f(0.0,0.0,1.0),axis),input.d.xyz,5u);
  let material=leaf_material(uv,normal,input.d.w,input.o.z);
  results[id.x]=ProbeResult(vec4f(material.normal,material.roughness),vec4f(material.albedo,material.metallic),
    vec4f(0.0),vec4f(leaf_fetch(leaf_uv(uv,0u),0u,8,0.0).a,
      leaf_fetch(leaf_uv(uv,3u),3u,11,0.0).a,0.0,0.0));
}
@compute @workgroup_size(1)
fn verify_finite_precise(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)/2u){return;}
  let input=rays[2u*id.x];let low=rays[2u*id.x+1u].o.xyz;let object=resolved_object(0u);
  let sample=scene_object_precise(input.o.xyz,low,input.d.xyz,object);
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
    vec4f(0.0,0.0,0.0,surface_normal(vec3f(0.0,0.0,1.0),vec3f(0.0,0.0,1.0),5u).z),
    vec4f(0.0),vec4f(0.0));
}
@compute @workgroup_size(1)
fn verify_finite_scene(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)){return;}
  let input=rays[id.x];
  let object=SceneObject(vec4u(7u,5u,u32(input.o.w)<<8u,0u),vec4f(0.0,0.0,0.0,1.0),vec4f(0.0,0.0,0.0,input.d.w),POSE_IDENTITY);
  let direction=normalize(input.d.xyz);let sample=scene_object(input.o.xyz,direction,object);let q=sample.query;
  let at=input.o.xyz+q.t*direction;
  results[id.x*2u]=ProbeResult(vec4f(7.0,5.0,f32(q.hit),q.t),vec4f(sample.gap_lower,q.gap,q.p.xy),
    vec4f(q.p.z,q.n),vec4f(q.uv,at.xy));
  results[id.x*2u+1u]=ProbeResult(vec4f(at.z,sample.local),
    vec4f(sample.error,f32(!q.hit),f32(sample.status),0.0),
    vec4f(sample.pending.lower,f32(sample.pending.valid),f32(sample.status<2u),sample.local_error),vec4f(0.0));
}

@compute @workgroup_size(1)
fn verify_texh(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)){return;}
  let r=rays[id.x];
  let object=SceneObject(vec4u(7u,u32(r.o.w),0u,0u),vec4f(0.0,0.0,0.0,1.0),vec4f(0.0,0.0,0.0,r.d.w),POSE_IDENTITY);
  let sample=scene_canonical(r.o.xyz,r.d.xyz,object);let q=sample.query;
  results[id.x]=ProbeResult(vec4f(f32(q.hit),q.t,q.gap,f32(sample.status)),
    vec4f(q.p,sample.gap_lower),vec4f(q.n,0.0),vec4f(q.uv,0.0,0.0));
}

@compute @workgroup_size(1)
fn verify_heightfield(@builtin(global_invocation_id) id:vec3u) {
  if(id.x>=arrayLength(&rays)){return;}
  let uv=rays[id.x].o.xy;
  results[id.x]=ProbeResult(vec4f(hf_height(uv),hf_bilinear(4u,uv,1.0),0.0,0.0),
    vec4f(0.0),vec4f(0.0),vec4f(0.0));
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
