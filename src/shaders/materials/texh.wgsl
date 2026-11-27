// Shared direct relief approximation for hits and gaps. No root search or gap solver.
struct TexhContext {base:SceneSample,object:SceneObject,delta:vec3f,footprint:f32,span:f32,coarse:f32,along:vec3f};
fn texh_footprint(base:SceneSample,d:vec3f,pixel_span:f32)->f32 {
  return max(1e-30,pixel_span*base.query.t/max(0.01,abs(dot(d,base.query.n))));
}
fn texh_context(base:SceneSample,d:vec3f,object:SceneObject,origin_delta:vec3f,pixel_span:f32)->TexhContext {
  var span=0.0;var coarse=0.0;
  if(material_field(object.params.w)){
    coarse=hf_render_level(0u,leaf_uv_unit(geometry_surface(base.local,base,object).uv),0.0);
    if(pixel_span>0.0){
      let c=dot(d,base.query.n);let factor=object.position_size.w*c/max(0.0025,c*c);
      let remaining=fi_up(hf_detail_bound());
      let closest=max(0.0,fi_down(base.query.t+coarse*factor)-fi_up(remaining*abs(factor)));
      span=pixel_span*closest/object.position_size.w;
    }
  }
  // Hits anchor at the ray point, gaps at their witness: both fixed in object space.
  let at=select(base.local*object.position_size.w,scene_local_vector(object,origin_delta+d*base.query.t),base.base_hit!=0u);
  return TexhContext(base,object,at,texh_footprint(base,d,pixel_span),span,coarse,vec3f(0.0));
}
fn texh_delta(foot:SurfacePoint,context:TexhContext)->vec3f {
  return context.delta+(foot.p-context.base.local)*context.object.position_size.w;
}
// Alpha at a foot and, for walls, world distances to the first contour ahead (+along) and behind (-along).
fn texh_sample(level:MaterialLevel,foot:SurfacePoint,context:TexhContext,lod:f32,walls:bool)->vec4f {
  let coords=material_coords(level,texh_delta(foot,context));
  let weights=material_weights(foot.n);let t=context.along*512.0/level.surface.x;var value=vec4f(0.0);
  if(weights.x>0.0){value+=weights.x*texh_project(level,coords.uv.yz,coords.cells.yz,t.yz,lod,walls);}
  if(weights.y>0.0){value+=weights.y*texh_project(level,coords.uv.xz,coords.cells.xz,t.xz,lod,walls);}
  if(weights.z>0.0){value+=weights.z*texh_project(level,coords.uv.xy,coords.cells.xy,t.xy,lod,walls);}
  return vec4f(clamp(value.x,0.0,1.0),value.yzw);
}
// Alpha, reach ahead and behind, and the nearer wall's ramp stretch.
fn texh_project(level:MaterialLevel,uv:vec2f,origin:vec2f,t:vec2f,lod:f32,walls:bool)->vec4f {
  let alpha=material_value(uv,origin,level,lod).a;
  if(!walls){return vec4f(alpha,0.0,0.0,1.0);}
  let runs=material_runs(uv,origin,level,lod,alpha>=f32(level.identity.z>>24u)/255.0);
  let ahead=material_reach(runs,t);let behind=material_reach(runs,-t);
  return vec4f(alpha,ahead.x,behind.x,select(ahead.y,behind.y,behind.x<ahead.x));
}
// Typical mode: typical, deepest and highest heights. Otherwise the height at foot; texh_walls then adds the
// material's largest-step level's classes, heights and contour distances around foot and bottom, once.
struct TexhLevel {height:f32,deepest:f32,highest:f32,ahead:f32,back:f32,behind:f32,onward:f32,near:f32,far:f32,groove:f32,bevel:f32,near_high:bool,far_high:bool,structural:bool,index:u32};
fn texh_merge(sum:TexhLevel,level:TexhLevel)->TexhLevel {
  var merged=sum;if(level.structural){merged=level;}
  merged.height=sum.height+level.height;merged.deepest=sum.deepest+level.deepest;merged.highest=sum.highest+level.highest;
  return merged;
}
fn texh_level(index:u32,foot:SurfacePoint,context:TexhContext,typical:bool)->TexhLevel {
  var result=TexhLevel(0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,false,false,false,index);
  if(index>=arrayLength(&material_levels)){return result;}
  let level=material_levels[index];let material=u32(context.object.params.w);
  if(level.identity.x!=material){return result;}
  let lod=material_lod(context.footprint,level.surface.x);let gain=level.surface.y*material_visible(lod);
  let datum=select(level.mean.a,1.0,(level.identity.z&1u&(MATERIAL_CARVE>>material))!=0u);
  if(typical){result.height=(f32((level.identity.z>>16u)&255u)/255.0-datum)*gain;result.deepest=-datum*gain;result.highest=(1.0-datum)*gain;return result;}
  if(gain==0.0){return result;}
  result.height=(texh_sample(level,foot,context,lod,false).x-datum)*gain;result.structural=(level.identity.z&2u)!=0u;
  return result;
}
fn texh_walls(sum:TexhLevel,foot:SurfacePoint,bottom:SurfacePoint,context:TexhContext)->TexhLevel {
  let level=material_levels[sum.index];let material=u32(context.object.params.w);
  let lod=material_lod(context.footprint,level.surface.x);let gain=level.surface.y*material_visible(lod);
  let datum=select(level.mean.a,1.0,(level.identity.z&1u&(MATERIAL_CARVE>>material))!=0u);
  let near=texh_sample(level,foot,context,lod,true);let far=texh_sample(level,bottom,context,lod,true);
  let edge=f32(level.identity.z>>24u)/255.0;let bevel=near.w*f32((level.identity.z>>2u)&63u)/8.0*level.surface.x/512.0;// half ramp width
  return TexhLevel(sum.height,0.0,0.0,near.y,near.z,far.z,far.y,(near.x-datum)*gain,(far.x-datum)*gain,(level.border.a-datum)*gain,bevel,near.x>=edge,far.x>=edge,true,sum.index);
}
fn texh_height(foot:SurfacePoint,bottom:SurfacePoint,context:TexhContext,typical:bool)->TexhLevel {
  var sum=TexhLevel(0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0,false,false,false,0u);
  if(material_field(context.object.params.w)){
    if(!typical){sum.height=context.coarse+hf_render_details(leaf_uv_unit(foot.uv),context.span);}
    return sum;
  }
  // @texh-levels
  if(!typical&&sum.structural){sum=texh_walls(sum,foot,bottom,context);}
  let size=context.object.position_size.w;
  sum.height/=size;sum.deepest/=size;sum.highest/=size;sum.ahead/=size;sum.back/=size;sum.behind/=size;sum.onward/=size;sum.near/=size;sum.far/=size;sum.groove/=size;sum.bevel/=size;
  return sum;
}
// Hit height above the carrier, shared by hit and gap paths: s sums heights at q (ray at the typical plane) with the
// largest-step level's walls toward bottom (ray at the deepest plane); shell holds typical/deepest; reach = |q→bottom|.
// Others stay at q. Runs reach a top's falling crest or a groove's rising foot. On a top: land before its crest, else
// cross to the far wall ramp or the groove floor. Over a groove: its floor, the wall ramp ahead, or the plateau beyond.
fn texh_depth(s:TexhLevel,shell:TexhLevel,reach:f32)->f32 {
  let top=shell.height;let span=max(top-shell.deepest,1e-30);let landing=(top-s.height)/span;let half=s.bevel/reach;
  let ahead=s.ahead/reach;let behind=1.0-s.behind/reach;let plateau=s.height-s.near+s.far;let groove=s.height-s.near+s.groove;
  if(!s.structural){return s.height;}
  // Above the ray at q, the hit lies behind q: on q's height, or past where the surface falls away behind q (a top's
  // crest, or q's own rising ramp) on that ramp.
  if(s.height>=top){
    let back=select(select(1e30,0.0,s.ahead<=0.0),s.back/reach,s.near_high);
    let rise=max(max(s.height,plateau)-groove,0.0)/max(2.0*half,1e-30);
    return select(s.height,top+span*(s.height-top+rise*back)/(span+rise),s.height-top>back*span);
  }
  var depth=s.height;
  // A top point whose fall lies just behind it sits on a ramp still rising ahead. Crest runs end at 80 % of the fall;
  // rounded shoulders start about a ramp earlier, so pass unless landing first.
  if(s.near_high&&s.back<s.bevel){depth=texh_ramp(top,span,half,half,s.height,max(plateau,s.height));}
  else if(s.near_high){if(landing>ahead-2.0*half){depth=select(plateau,texh_ramp(top,span,max(behind-half,ahead+half),half,groove,plateau),s.far_high);}}
  // A groove point on a falling slope (its own ramp rises behind it) drops to the groove floor and meets the far wall
  // seen from bottom; on a rising ramp it keeps its own height.
  else{
    let falling=s.back<0.25*s.bevel&&s.ahead>0.25*s.bevel&&s.far_high;
    let centre=select(ahead+half,max(behind-half,ahead+half),falling);
    depth=texh_ramp(top,span,centre,half,select(min(s.height,groove),s.height,ahead<=0.0&&!falling),select(top,plateau,s.far_high));
  }
  return depth;
}
// First meeting of the ray z = top - λ·span with a floor lo, then a straight wall up to hi across centre ± half, then hi.
fn texh_ramp(top:f32,span:f32,centre:f32,half:f32,lo:f32,hi:f32)->f32 {
  let start=max(centre-half,0.0);
  if((top-lo)/span<=start){return lo;}
  let k=(hi-lo)/max(centre+half-start,1e-6);let meet=(top-lo+k*start)/(span+k);
  return select(hi,top-meet*span,meet<=centre+half);
}
// Ray through the relief from anchor, its point at the base t: signed height g over the carrier and slope c along its
// normal there (hit: crossing, g=0; gap: closest approach, c≈0 on closed carriers), curving k along the ray, so heights
// follow z=g+cτ+kτ²/2. Hits and gaps share all else.
struct TexhRay {anchor:vec3f,ld:vec3f,g:f32,c:f32,k:f32};
fn texh_ray(base:SceneSample,d:vec3f,object:SceneObject,origin_delta:vec3f)->TexhRay {
  let gap=base.base_hit==0u;let size=object.position_size.w;let n=scene_local_vector(object,base.query.n);
  let anchor=select(base.local,scene_local_vector(object,origin_delta+d*base.query.t)/size,gap);let ld=scene_local_vector(object,d);
  let g=select(0.0,sign(dot(anchor-base.local,n))*base.query.gap/size,gap);
  return TexhRay(anchor,ld,g,dot(ld,n),geometry_curvature(base,d,object));
}
// Slope magnitude for offsets: |c|, steepened below .05 over carriers curving less than .005 per size, so flat grazing
// offsets fade (z·c/max(c²,.0025)) instead of exploding; curved carriers keep the exact parabola, continuous in k and c.
fn texh_slope(ray:TexhRay)->f32 {let c=abs(ray.c);return c+2.0*max(max(0.0025-0.5*ray.k,0.0)-c*c,0.0)/max(max(c,sqrt(ray.k)),1e-30);}
// Ray offset from the anchor to height z, or to the lowest point when z is out of reach. Curved rays rising at the
// anchor (gaps past their closest approach, rounding at tangency) take the entry root, which the descending form
// meets at c=0: no flip near tangency. Flat ones keep their one crossing, ahead when leaving a back face.
fn texh_ray_at(ray:TexhRay,z:f32)->f32 {
  let dz=z-ray.g;let a=texh_slope(ray);let r=ray.c*ray.c+2.0*ray.k*dz;let q=sqrt(max(r,0.0));
  let near=select(-1.0,1.0,ray.c>0.0)*2.0*dz/max(a+q,1e-30);
  return select(select(near,(-ray.c-q)/max(ray.k,1e-30),ray.c>0.0&&ray.k>=0.005),-ray.c*abs(ray.c)/max(ray.k*a,1e-30),r<0.0);
}
// Lowest height the ray reaches: its closest approach over curved carriers; flat carriers keep descending.
fn texh_lowest(ray:TexhRay)->f32 {return ray.g-0.5*ray.c*ray.c/max(ray.k,1e-30);}
fn texh_z(ray:TexhRay,t:f32)->f32 {return ray.g+t*(ray.c+0.5*ray.k*t);}
// Landing offset, and whether relief is met. Descending, at the wall depth; out of reach, at the lowest point (where the
// wall depth lands as it sinks to it) while the wall depth or bottom's plateau lies within the margin under it; then,
// over one more margin, onward to the rising far wall at offset wall if any (rise): landings never jump.
fn texh_land(ray:TexhRay,depth:f32,plateau:f32,wall:f32,rise:bool,margin:f32)->vec2f {
  let sink=(texh_lowest(ray)-max(depth,plateau))/max(margin,1e-30);
  return vec2f(mix(texh_ray_at(ray,depth),wall,select(0.0,clamp(sink-1.0,0.0,1.0),rise)),f32(sink<1.0||rise));
}
// Share of the descent q→bottom made at distance x along it (inside only): the ray bows below their chord, a and b
// being q's and bottom's offsets from its lowest point, so straight-descent walls land where the ray meets them.
fn texh_warp(x:f32,reach:f32,a:f32,b:f32)->f32 {
  let f=x/reach;let g=clamp(f,0.0,1.0);
  return select(x,reach*g*(2.0*a+g*(b-a))/min(a+b,-1e-30),f==g);
}
// Foot under the ray at height z: q at the median plane (brick face, not tops), bottom at the deepest plane.
fn texh_source(ray:TexhRay,base:SceneSample,object:SceneObject,z:f32)->SurfacePoint {
  return geometry_surface(fma(vec3f(texh_ray_at(ray,z)),ray.ld,ray.anchor),base,object);
}
fn texh_gradient(point:vec3f,foot:SurfacePoint,context:TexhContext,normal_filter:f32)->vec3f {
  if(material_field(context.object.params.w)){
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
  return material_unit(n-g/select(max(1.0,length(g)/2.0),1.0,material_field(context.object.params.w)),n);
}
// Largest ray-cone stretch of the frozen-frame direct map, including grazing fade.
// Only shading uses it; carrier height/LOD and depth are never resampled.
fn texh_filter_footprint(base:SceneSample,depth:f32,d:vec3f,g:vec3f,pixel_span:f32,c:f32)->f32 {
  let sine2=max(0.0,1.0-c*c);let along=dot(g,d);
  let kt=base.query.t*c/max(0.0025,c*c);
  let fade=select(0.0,2.0*(depth-base.query.t)*sine2,c*c<0.0025);
  let a=(depth+kt*along-fade)/max(1e-6,abs(c));
  let b=kt*sqrt(max(0.0,sine2*dot(g,g)-along*along));
  let stretch=0.5*(length(vec2f(a+depth,b))+length(vec2f(a-depth,b)));
  return max(1e-30,pixel_span*max(max(base.query.t,depth)/max(0.01,abs(c)),stretch));
}
fn texh_shading_context(sample:SceneSample,base:SceneSample,d:vec3f,object:SceneObject,origin_delta:vec3f,pixel_span:f32)->TexhContext {
  var context=texh_context(base,d,object,origin_delta,pixel_span);
  if(pixel_span>0.0&&sample.query.hit){
    let carrier=geometry_surface(base.local,base,object);let z=texh_height(carrier,carrier,context,true).height;
    let g=texh_gradient(base.local,texh_source(texh_ray(base,d,object,origin_delta),base,object,z),context,0.0);
    // The map's slope at the landing: the carrier's own on flat carriers; on curved ones flatter toward the ray's
    // lowest point, floored like grazing footprints.
    let ray=texh_ray(base,d,object,origin_delta);let slope=ray.c+ray.k*(sample.query.t-base.query.t)/object.position_size.w;
    let c=select(dot(base.query.n,d),-max(abs(slope),0.01),ray.k>=0.005);
    context.footprint=texh_filter_footprint(base,sample.query.t,d,scene_world_vector(object,g),pixel_span,c);
  }
  return context;
}
fn material_texh_depth(base:SceneSample,d:vec3f,object:SceneObject,origin_delta:vec3f,pixel_span:f32,relief:f32)->SceneSample {
  // Gaps share the hit path; past the rise bound they are culled exactly like scene_relief_candidate, so the one
  // relief pass per pixel lands on its candidate or misses it.
  let gap=base.base_hit==0u;
  if(gap&&(base.status!=0u||base.query.gap>=relief*material_spec(u32(object.params.w)).rise)){return base;}
  let ray=texh_ray(base,d,object,origin_delta);
  var context=texh_context(base,d,object,origin_delta,pixel_span);
  // Direct walls, at most 4 lookups: heights at q (ray at the typical plane); for the largest-step level also its
  // contour distance ahead of q, and its height and contour distance behind the deepest point. Others stay at q.
  let carrier=geometry_surface(base.local,base,object);let shell=texh_height(carrier,carrier,context,true);
  context.along=material_unit(ray.ld-carrier.n*dot(ray.ld,carrier.n),vec3f(0.0));
  // A chord above the deepest plane descends only to its lowest point: relief below it is never met.
  let v=select(0.0,-ray.c*abs(ray.c)/max(ray.k*texh_slope(ray),1e-30),ray.k>0.0);
  let lowest=texh_lowest(ray);var descent=shell;descent.deepest=max(shell.deepest,lowest);
  let tq=texh_ray_at(ray,shell.height);let tb=texh_ray_at(ray,descent.deepest);
  let q=geometry_surface(fma(vec3f(tq),ray.ld,ray.anchor),base,object);let bottom=geometry_surface(fma(vec3f(tb),ray.ld,ray.anchor),base,object);
  let s=texh_height(q,bottom,context,false);let reach=max(length(bottom.p-q.p),1e-30);
  var w=s;
  // From a groove toward a top, the far wall is the nearer of the runs from q and from bottom (capped or oblique runs).
  // Out of reach, walls past the lowest point lie on the rising side, met below; the descent never meets them.
  if(ray.k>0.0){
    if(!s.near_high&&s.far_high){w.ahead=min(s.ahead,reach-s.behind);}
    w.ahead=select(texh_warp(w.ahead,reach,tq-v,tb-v),1e30,lowest>shell.deepest&&w.ahead>reach);
    w.behind=reach-texh_warp(reach-s.behind,reach,tq-v,tb-v);
  }
  // Unclamped at the lowest point: how far relief out of reach sinks under it blends the landing below.
  let depth=relief*texh_depth(w,descent,reach);
  // Bottom's plateau: its largest-step level, q's others. Out of reach it is met within a pixel cone at the lowest
  // point, at most 1/128 of the relief range: unsampled relief rarely rises more. Past that point the ray climbs again:
  // over a groove there it meets the groove's far wall while still under the typical face, or q's if higher.
  let margin=min(pixel_span*(base.query.t/object.position_size.w+v),relief*(shell.highest-shell.deepest)/128.0);
  let wall=tb+s.onward;let face=relief*max(shell.height,select(shell.height,s.height,s.near_high));
  let land=texh_land(ray,depth,relief*(s.height-s.near+s.far),wall,!s.far_high&&texh_z(ray,wall)<face+margin,margin);
  // Land on the ray itself: gaps start at their ray point, not their witness.
  var result=base;result.query.t+=land.x*object.position_size.w;result.local=fma(vec3f(land.x),ray.ld,ray.anchor);
  result.query.p+=scene_world_vector(object,fma(vec3f(land.x),ray.ld,ray.anchor-base.local))*object.position_size.w;
  let foot=geometry_surface(result.local,base,object);
  result.query.hit=land.y>0.5&&result.query.t>=0.0&&fi_finite(result.query.t)&&foot.valid;
  if(gap&&!result.query.hit){return base;}
  result.query.uv=foot.uv;
  if(material_field(object.params.w)){result.query.hit=result.query.hit&&leaf_covered(leaf_uv_unit(foot.uv));}
  if(result.query.hit){
    result.query.gap=0.0;result.gap_lower=0.0;
  }
  return result;
}
// Normals are needed only after nearest-hit selection; probes retain the full adapter.
fn texh_shade_normal(sample:SceneSample,base:SceneSample,d:vec3f,object:SceneObject,origin_delta:vec3f,pixel_span:f32)->vec3f {
  if(!sample.query.hit){return sample.query.n;}
  let context=texh_shading_context(sample,base,d,object,origin_delta,pixel_span);
  return scene_world_vector(object,texh_normal(sample.local,geometry_surface(sample.local,base,object),context));
}
fn material_texh(base:SceneSample,d:vec3f,object:SceneObject,origin_delta:vec3f,pixel_span:f32)->SceneSample {
  var sample=material_texh_depth(base,d,object,origin_delta,pixel_span,1.0);
  sample.query.n=texh_shade_normal(sample,base,d,object,origin_delta,pixel_span);
  return sample;
}
fn texh_surface(sample:SceneSample,base:SceneSample,d:vec3f,object:SceneObject,origin_delta:vec3f,pixel_span:f32)->MaterialSurface {
  let context=texh_shading_context(sample,base,d,object,origin_delta,pixel_span);
  let foot=geometry_surface(sample.local,base,object);
  if(material_field(object.params.w)){
    let n=scene_world_vector(object,texh_normal(sample.local,foot,context));
    return leaf_material(u32(object.params.w),sample.query.uv,surface_normal(n,base.query.n,d,sample.kind),object.position_size.w,context.footprint);
  }
  var material=layered_material(u32(object.params.w),texh_delta(foot,context),foot.n,context.footprint);
  material.normal=surface_normal(scene_world_vector(object,material.normal),base.query.n,d,sample.kind);
  return material;
}
