// Single ray-depth correction from the same phased atlas and mips as brick albedo.
fn brick_height(delta:vec3f,n:vec3f,footprint:f32,anchor:vec3f)->f32 {
  let weights=material_weights(n);
  var height=0.0;
  for(var i=0u;i<arrayLength(&material_levels);i++){
    let level=material_levels[i];
    if(level.identity.x!=1u){continue;}
    let lod=material_lod(footprint,level.surface.x);
    let visible=material_visible(lod);
    if(visible==0.0){continue;}
    let frame=material_coords(level,delta,anchor);
    let uv=frame.uv;let cells=frame.cells;
    var alpha=0.0;
    if(weights.x>0.0001){alpha+=weights.x*material_value(uv.yz,cells.yz,level,lod).a;}
    if(weights.y>0.0001){alpha+=weights.y*material_value(uv.xz,cells.xz,level,lod).a;}
    if(weights.z>0.0001){alpha+=weights.z*material_value(uv.xy,cells.xy,level,lod).a;}
    let datum=select(level.mean.a,1.0,(level.identity.z&1u)!=0u);
    height+=(clamp(alpha,0.0,1.0)-datum)*level.surface.y*visible;
  }
  return min(0.0,height);
}
fn brick_contact(base:SceneSample,d:vec3f,object:SceneObject,origin_delta:vec3f,pixel_span:f32)->SceneSample {
  if(!base.query.hit||object.params.w!=1.0){return base;}
  let slope=dot(d,base.query.n);
  if(abs(slope)<=0.15){return base;}
  let size=object.position_size.w;
  let seed=render_material_distance(base,d,size,object.params.x);
  let footprint=pixel_span*seed/abs(slope);
  let height=brick_height(fma(vec3f(seed),d,origin_delta),base.query.n,footprint,object.position_size.xyz);
  let step=height_step(height*smoothstep(0.15,0.5,abs(slope)),slope,min(0.024,0.45*size));
  let t=seed+step;
  if(!fi_finite(t)||t<0.0){return base;}
  let shift=t-base.query.t;
  let local=fma(vec3f(shift/size),d,base.local);
  var hit=base;
  if(base.kind==5u){
    let frame=plane_frame(local,scene_axis(object));
    if(any(abs(frame.xy)>vec2f(1.0))){return base;}
    hit.query.uv=fma(frame.xy,vec2f(0.5),vec2f(0.5));
  }else if(base.kind==3u){hit.query.uv=local.xz;}
  hit.local=local;
  hit.query.t=t;
  hit.query.p=fma(vec3f(shift),d,base.query.p);
  return hit;
}
