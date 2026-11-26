// Shared direct hit approximation. One height sample; no root search or gap work.
struct TexhContext {base:SceneSample,object:SceneObject,delta:vec3f,footprint:f32,span:f32,coarse:f32};
fn texh_footprint(base:SceneSample,d:vec3f,pixel_span:f32)->f32 {
  return max(1e-30,pixel_span*base.query.t/max(0.01,abs(dot(d,base.query.n))));
}
fn texh_context(base:SceneSample,d:vec3f,object:SceneObject,origin_delta:vec3f,pixel_span:f32)->TexhContext {
  var span=0.0;var coarse=0.0;
  if(object.params.w==2.0){
    coarse=hf_render_level(0u,leaf_uv_unit(geometry_surface(base.local,base,object).uv),0.0);
    if(pixel_span>0.0){
      let c=dot(d,base.query.n);let factor=object.position_size.w*c/max(0.0025,c*c);
      let remaining=fi_up(dot(abs(height_data.amplitudes.yzw),vec3f(128.0/255.0)));
      let closest=max(0.0,fi_down(base.query.t+coarse*factor)-fi_up(remaining*abs(factor)));
      span=pixel_span*closest/object.position_size.w;
    }
  }
  return TexhContext(base,object,scene_local_vector(object,origin_delta+d*base.query.t),texh_footprint(base,d,pixel_span),span,coarse);
}
fn texh_delta(foot:SurfacePoint,context:TexhContext)->vec3f {
  return context.delta+(foot.p-context.base.local)*context.object.position_size.w;
}
fn texh_level(index:u32,foot:SurfacePoint,context:TexhContext,typical:bool)->f32 {
  if(index>=arrayLength(&material_levels)){return 0.0;}
  let level=material_levels[index];let material=u32(context.object.params.w);
  if(level.identity.x!=material){return 0.0;}
  let lod=material_lod(context.footprint,level.surface.x);let visible=material_visible(lod);
  let top=material==1u&&(level.identity.z&1u)!=0u;let datum=select(level.mean.a,1.0,top);
  if(typical){return select(0.0,f32(level.identity.z>>16u)/255.0-1.0,top)*level.surface.y*visible;}
  if(visible==0.0){return 0.0;}
  let coords=material_coords(level,texh_delta(foot,context));
  let weights=material_weights(foot.n);var alpha=0.0;
  if(weights.x>0.0){alpha+=weights.x*material_value(coords.uv.yz,coords.cells.yz,level,lod).a;}
  if(weights.y>0.0){alpha+=weights.y*material_value(coords.uv.xz,coords.cells.xz,level,lod).a;}
  if(weights.z>0.0){alpha+=weights.z*material_value(coords.uv.xy,coords.cells.xy,level,lod).a;}
  return (clamp(alpha,0.0,1.0)-datum)*level.surface.y*visible;
}
fn texh_height(foot:SurfacePoint,context:TexhContext,typical:bool)->f32 {
  if(context.object.params.w==2.0){
    if(typical){return 0.0;}
    return context.coarse+hf_render_details(leaf_uv_unit(foot.uv),context.span);
  }
  var height=0.0;
  // @texh-levels
  if(context.object.params.w==1.0){height=min(0.0,height);}
  return height/context.object.position_size.w;
}
// Sample depth where the ray meets the median-height plane: brick tops sit ~1 mm above the face.
fn texh_source(base:SceneSample,d:vec3f,object:SceneObject,context:TexhContext)->SurfacePoint {
  let carrier=geometry_surface(base.local,base,object);let c=dot(d,base.query.n);
  let reach=texh_height(carrier,context,true)*c/max(0.0025,c*c);
  if(reach==0.0){return carrier;}
  return geometry_surface(fma(vec3f(reach),scene_local_vector(object,d),base.local),base,object);
}
fn texh_gradient(point:vec3f,foot:SurfacePoint,context:TexhContext,normal_filter:f32)->vec3f {
  if(context.object.params.w==2.0){
    let axes=geometry_uv_axes(point,context.base,context.object);let error=abs(axes[0])+abs(axes[1]);
    let quantum_axes=normal_filter*max(error.x,max(error.y,error.z));
    let slope=hf_gradient_lod(leaf_uv_unit(foot.uv),0.5*context.footprint/context.object.position_size.w,context.span,quantum_axes);
    return axes*slope;
  }
  let n=material_unit(foot.n,vec3f(0.0,1.0,0.0));
  let g=material_layers(u32(context.object.params.w),texh_delta(foot,context),n,context.footprint,normal_filter).gradient;
  return g-n*dot(n,g);
}
fn texh_normal(point:vec3f,foot:SurfacePoint,context:TexhContext)->vec3f {
  let g=texh_gradient(point,foot,context,1.0);let n=material_unit(foot.n,vec3f(0.0,1.0,0.0));
  return material_unit(n-g/select(max(1.0,length(g)/2.0),1.0,context.object.params.w==2.0),n);
}
// Largest ray-cone stretch of the frozen-frame direct map, including grazing fade.
// Only shading uses it; carrier height/LOD and depth are never resampled.
fn texh_filter_footprint(base:SceneSample,depth:f32,d:vec3f,g:vec3f,pixel_span:f32)->f32 {
  let c=dot(base.query.n,d);let sine2=max(0.0,1.0-c*c);let along=dot(g,d);
  let kt=base.query.t*c/max(0.0025,c*c);
  let fade=select(0.0,2.0*(depth-base.query.t)*sine2,c*c<0.0025);
  let a=(depth+kt*along-fade)/max(1e-6,abs(c));
  let b=kt*sqrt(max(0.0,sine2*dot(g,g)-along*along));
  let stretch=0.5*(length(vec2f(a+depth,b))+length(vec2f(a-depth,b)));
  return max(1e-30,pixel_span*max(max(base.query.t,depth)/max(0.01,abs(c)),stretch));
}
fn texh_shading_context(sample:SceneSample,base:SceneSample,d:vec3f,object:SceneObject,origin_delta:vec3f,pixel_span:f32)->TexhContext {
  var context=texh_context(base,d,object,origin_delta,pixel_span);
  if(pixel_span>0.0&&sample.query.hit&&base.base_hit!=0u){
    let g=texh_gradient(base.local,texh_source(base,d,object,context),context,0.0);
    context.footprint=texh_filter_footprint(base,sample.query.t,d,scene_world_vector(object,g),pixel_span);
  }
  return context;
}
fn material_texh_depth(base:SceneSample,d:vec3f,object:SceneObject,origin_delta:vec3f,pixel_span:f32)->SceneSample {
  if(base.base_hit==0u){return base;}
  let context=texh_context(base,d,object,origin_delta,pixel_span);
  let height=texh_height(texh_source(base,d,object,context),context,false);
  let incidence=dot(d,base.query.n);
  // Reciprocal incidence above |cos|=.05; fade instead of exploding at grazing angles.
  let offset=height*incidence/max(0.0025,incidence*incidence);
  var result=base;result.query.t+=offset*object.position_size.w;
  result.query.p=fma(vec3f(offset*object.position_size.w),d,base.query.p);
  result.local=fma(vec3f(offset),scene_local_vector(object,d),base.local);
  let foot=geometry_surface(result.local,base,object);
  result.query.hit=result.query.t>=0.0&&fi_finite(result.query.t)&&foot.valid;
  result.query.uv=foot.uv;
  if(object.params.w==2.0){result.query.hit=result.query.hit&&leaf_covered(leaf_uv_unit(foot.uv));}
  if(result.query.hit){
    result.query.gap=0.0;result.gap_lower=0.0;
  }
  return result;
}
// Normals are needed only after nearest-hit selection; probes retain the full adapter.
fn texh_shade_normal(sample:SceneSample,base:SceneSample,d:vec3f,object:SceneObject,origin_delta:vec3f,pixel_span:f32)->vec3f {
  if(!sample.query.hit||base.base_hit==0u){return sample.query.n;}
  let context=texh_shading_context(sample,base,d,object,origin_delta,pixel_span);
  return scene_world_vector(object,texh_normal(sample.local,geometry_surface(sample.local,base,object),context));
}
fn material_texh(base:SceneSample,d:vec3f,object:SceneObject,origin_delta:vec3f,pixel_span:f32)->SceneSample {
  var sample=material_texh_depth(base,d,object,origin_delta,pixel_span);
  sample.query.n=texh_shade_normal(sample,base,d,object,origin_delta,pixel_span);
  return sample;
}
fn texh_surface(sample:SceneSample,base:SceneSample,d:vec3f,object:SceneObject,origin_delta:vec3f,pixel_span:f32)->MaterialSurface {
  let context=texh_shading_context(sample,base,d,object,origin_delta,pixel_span);
  let foot=geometry_surface(sample.local,base,object);
  if(object.params.w==2.0){
    let n=scene_world_vector(object,texh_normal(sample.local,foot,context));
    return leaf_material(sample.query.uv,surface_normal(n,base.query.n,d,sample.kind),object.position_size.w,context.footprint);
  }
  var material=layered_material(u32(object.params.w),texh_delta(foot,context),foot.n,context.footprint);
  material.normal=surface_normal(scene_world_vector(object,material.normal),base.query.n,d,sample.kind);
  return material;
}
