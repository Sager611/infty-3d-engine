// Shared direct hit approximation. One carrier height; no root search or gap work.
struct TexhContext {base:SceneSample,object:SceneObject,delta:vec3f,footprint:f32};
fn texh_footprint(base:SceneSample,d:vec3f,pixel_span:f32)->f32 {
  return max(1e-30,pixel_span*base.query.t/max(0.01,abs(dot(d,base.query.n))));
}
fn texh_delta(foot:SurfacePoint,context:TexhContext)->vec3f {
  return context.delta+(foot.p-context.base.local)*context.object.position_size.w;
}
fn texh_level(index:u32,foot:SurfacePoint,context:TexhContext)->f32 {
  if(index>=arrayLength(&material_levels)){return 0.0;}
  let level=material_levels[index];let material=u32(context.object.params.w);
  if(level.identity.x!=material){return 0.0;}
  let lod=material_lod(context.footprint,level.surface.x);let visible=material_visible(lod);
  if(visible==0.0){return 0.0;}
  let coords=material_coords(level,texh_delta(foot,context),context.object.position_size.xyz);
  let weights=material_weights(foot.n);var alpha=0.0;
  if(weights.x>0.0){alpha+=weights.x*material_value(coords.uv.yz,coords.cells.yz,level,lod).a;}
  if(weights.y>0.0){alpha+=weights.y*material_value(coords.uv.xz,coords.cells.xz,level,lod).a;}
  if(weights.z>0.0){alpha+=weights.z*material_value(coords.uv.xy,coords.cells.xy,level,lod).a;}
  let datum=select(level.mean.a,1.0,material==1u&&(level.identity.z&1u)!=0u);
  return (clamp(alpha,0.0,1.0)-datum)*level.surface.y*visible;
}
fn texh_height(foot:SurfacePoint,context:TexhContext)->f32 {
  if(context.object.params.w==2.0){return hf_height(leaf_uv_unit(foot.uv));}
  var height=0.0;
  // @texh-levels
  if(context.object.params.w==1.0){height=min(0.0,height);}
  return height/context.object.position_size.w;
}
fn texh_normal(point:vec3f,foot:SurfacePoint,context:TexhContext)->vec3f {
  if(context.object.params.w==2.0){
    let slope=hf_gradient(leaf_uv_unit(foot.uv),0.5*context.footprint/context.object.position_size.w);
    return material_unit(foot.n-geometry_uv_axes(point,context.base,context.object)*slope,foot.n);
  }
  return layered_material(u32(context.object.params.w),texh_delta(foot,context),foot.n,context.footprint,context.object.position_size.xyz).normal;
}
fn material_texh(base:SceneSample,d:vec3f,object:SceneObject,origin_delta:vec3f,pixel_span:f32)->SceneSample {
  if(!base.base_hit){return base;}
  let context=TexhContext(base,object,origin_delta+d*base.query.t,texh_footprint(base,d,pixel_span));
  let height=texh_height(geometry_surface(base.local,base,object),context);
  let incidence=dot(d,base.query.n);
  // Reciprocal incidence above |cos|=.05; fade instead of exploding at grazing angles.
  let offset=height*incidence/max(0.0025,incidence*incidence);
  var result=base;result.query.t+=offset*object.position_size.w;
  result.query.p=fma(vec3f(offset*object.position_size.w),d,base.query.p);
  result.local=fma(vec3f(offset),d,base.local);
  let foot=geometry_surface(result.local,base,object);
  result.query.hit=result.query.t>=0.0&&fi_finite(result.query.t)&&foot.valid;
  result.query.uv=foot.uv;
  if(object.params.w==2.0){result.query.hit=result.query.hit&&leaf_covered(leaf_uv_unit(foot.uv));}
  if(result.query.hit){
    result.query.n=texh_normal(result.local,foot,context);
    result.query.gap=0.0;result.gap_lower=0.0;
  }
  return result;
}
fn texh_surface(sample:SceneSample,base:SceneSample,d:vec3f,object:SceneObject,origin_delta:vec3f,pixel_span:f32)->MaterialSurface {
  let footprint=texh_footprint(base,d,pixel_span);let normal=surface_normal(sample.query.n,d,sample.kind);
  if(object.params.w==2.0){return leaf_material(sample.query.uv,normal,object.position_size.w,footprint);}
  let context=TexhContext(base,object,origin_delta+d*base.query.t,footprint);
  let foot=geometry_surface(sample.local,base,object);
  var material=layered_material(u32(object.params.w),texh_delta(foot,context),foot.n,footprint,object.position_size.xyz);
  material.normal=normal;return material;
}
