// Finite cardinal square; analytic clipping and nine convex miss-gap regions.
struct PlaneRange {
  near:f32,near_upper:f32,far:f32,far_lower:f32,
  unbounded:bool,valid:bool,uncertain:bool
};
fn plane_range()->PlaneRange {return PlaneRange(0.0,0.0,0.0,0.0,true,true,false);}
fn plane_clip(input:PlaneRange,o:f32,d:f32,bound:f32,lower:bool)->PlaneRange {
  var span=input;if(!span.valid){return span;}
  if(d==0.0){
    span.valid=select(o<=bound,o>=bound,lower);return span;
  }
  let time=fi_div(fi_sub(fi_exact(bound),fi_exact(o)),fi_exact(d));
  if(!time.valid){span.uncertain=true;return span;}
  if(lower==(d>0.0)){
    span.near=max(span.near,time.v.x);span.near_upper=max(span.near_upper,time.v.y);
  }else{
    if(span.unbounded){span.far=time.v.y;span.far_lower=time.v.x;}
    else{span.far=min(span.far,time.v.y);span.far_lower=min(span.far_lower,time.v.x);}
    span.unbounded=false;
  }
  span.valid=span.unbounded||span.far>=span.near;return span;
}
fn plane_axis(input:PlaneRange,o:f32,d:f32,lo:f32,hi:f32)->PlaneRange {
  return plane_clip(plane_clip(input,o,d,lo,true),o,d,hi,false);
}
fn plane_box(o:vec3f,d:vec3f,lo:vec3f,hi:vec3f)->PlaneRange {
  var span=plane_range();
  for(var axis=0u;axis<3u;axis++){span=plane_axis(span,o[axis],d[axis],lo[axis],hi[axis]);}
  return span;
}
fn plane_frame(p:vec3f,axis:u32)->vec3f {
  if(axis==1u){return p.yzx;}if(axis==2u){return p.zxy;}return p;
}
fn plane_unframe(p:vec3f,axis:u32)->vec3f {
  if(axis==1u){return p.zxy;}if(axis==2u){return p.yzx;}return p;
}
fn plane_sample(p:vec3f,t:f32,hit:bool)->Query {
  return Query(hit,t,0.0,p,vec3f(0.0,0.0,1.0),fma(p.xy,vec2f(0.5),vec2f(0.5)));
}
fn plane_rounding(o:vec3f,t:f32)->f32 {
  return 32.0*FI_U*max(1.0,max(abs(t),max(abs(o.x),max(abs(o.y),abs(o.z)))));
}
fn plane_clamp_time(time:FInterval,span:PlaneRange)->FInterval {
  var value=max(time.v,vec2f(span.near,span.near_upper));
  if(!span.unbounded){value=min(value,vec2f(span.far_lower,span.far));}
  return FInterval(value,time.valid&&value.x<=value.y&&!span.uncertain);
}
fn plane_project(offset:vec3f,tangent:vec3f,span:PlaneRange)->FInterval {
  let scale=max(abs(tangent.x),max(abs(tangent.y),abs(tangent.z)));
  if(scale==0.0){return FInterval(vec2f(span.near,span.near_upper),!span.uncertain);}
  var numerator=fi_exact(0.0);var denominator=fi_exact(0.0);
  for(var axis=0u;axis<3u;axis++){
    let unit=fi_div(fi_exact(tangent[axis]),fi_exact(scale));
    numerator=fi_add(numerator,fi_scale(unit,offset[axis]));
    denominator=fi_add(denominator,fi_square(unit));
  }
  return plane_clamp_time(fi_neg(fi_div(numerator,fi_scale(denominator,scale))),span);
}
fn plane_gap(o:vec3f,d:vec3f)->GeometryQuery {
  var best=GeometryQuery(plane_sample(vec3f(0.0),0.0,false),0.0,0.0,0u);
  var found=false;var lower=fi_exact(0.0);var imprecise=false;var have_lower=false;
  for(var ix=0u;ix<3u;ix++){
    for(var iy=0u;iy<3u;iy++){
      let region=vec2f(f32(ix)-1.0,f32(iy)-1.0);var span=plane_range();
      for(var axis=0u;axis<2u;axis++){
        if(region[axis]==0.0){span=plane_axis(span,o[axis],d[axis],-1.0,1.0);}
        else{span=plane_clip(span,o[axis],d[axis],region[axis],region[axis]>0.0);}
      }
      if(!span.valid){continue;}
      let inside=region==vec2f(0.0);
      let offset=vec3f(select(o.xy-region,vec2f(0.0),inside),o.z);
      let tangent=vec3f(select(d.xy,vec2f(0.0),inside),d.z);
      let time=plane_project(offset,tangent,span);
      var t=span.near;
      if(time.valid){t=max(0.0,fi_middle(time));}
      let position=fma(vec3f(t),d,o);
      let witness=vec3f(clamp(position.xy,vec2f(-1.0),vec2f(1.0)),0.0);
      let error=plane_rounding(o,t);
      let metric=fi_add(fi_distance(position,witness),fi_exact(error));
      var squared=fi_exact(0.0);
      for(var axis=0u;axis<3u;axis++){
        squared=fi_add(squared,fi_square(fi_add(fi_exact(offset[axis]),fi_scale(time,tangent[axis]))));
      }
      let minimum=fi_sqrt(squared);var bound=0.0;
      imprecise=imprecise||!minimum.valid||span.uncertain;
      if(minimum.valid){bound=max(0.0,minimum.v.x);}
      if(!have_lower){lower=fi_exact(bound);have_lower=true;}else{lower.v.x=min(lower.v.x,bound);}
      if(!metric.valid){best.status=1u;continue;}
      if(!found||metric.v.y<best.query.gap){
        var query=plane_sample(witness,t,false);query.gap=metric.v.y;
        best=GeometryQuery(query,0.0,error,select(0u,1u,span.uncertain));found=true;
      }
    }
  }
  best.status=max(best.status,select(0u,1u,imprecise));
  if(!found){best.status=3u;}best.gap_lower=lower.v.x;return best;
}
fn plane_supported(o:vec3f,d:vec3f)->bool {
  for(var axis=0u;axis<3u;axis++){
    if(!fi_finite(o[axis])||!fi_finite(d[axis])||fi_subnormal(o[axis])||
       fi_subnormal(d[axis])||abs(o[axis])>0x1p+60||abs(d[axis])>2.0){return false;}
  }
  return abs(dot(d,d)-1.0)<=32.0*FI_U;
}
fn plane_flat(o:vec3f,d:vec3f)->GeometryQuery {
  if(!plane_supported(o,d)){return GeometryQuery(plane_sample(vec3f(0.0),0.0,false),0.0,0.0,3u);}
  var query=plane_sample(vec3f(0.0),0.0,false);var uncertain=false;var time_error=0.0;
  if(d.z!=0.0){
    if(o.z==0.0||((o.z>0.0)!=(d.z>0.0))){
      let time=fi_div(fi_neg(fi_exact(o.z)),fi_exact(d.z));
      if(!time.valid){uncertain=true;}else{
        let t=max(0.0,fi_middle(time));let position=fma(vec3f(t),d,o);
        var possible=time.v.y>=0.0;var certain=time.v.x>=0.0;
        for(var axis=0u;axis<2u;axis++){
          let coordinate=fi_add(fi_exact(o[axis]),fi_scale(time,d[axis]));
          if(!coordinate.valid){uncertain=true;}else{possible=possible&&coordinate.v.y>=-1.0&&coordinate.v.x<=1.0;}
          certain=certain&&coordinate.valid&&coordinate.v.x>=-1.0&&coordinate.v.y<=1.0;
        }
        if(possible){
          uncertain=uncertain||!certain;time_error=time.v.y-time.v.x;
          if(all(abs(position.xy)<=vec2f(1.0))){query=plane_sample(vec3f(position.xy,0.0),t,true);}
          else{uncertain=true;}
        }
      }
    }
  }else if(o.z==0.0){
    let span=plane_box(o,d,vec3f(-1.0,-1.0,0.0),vec3f(1.0,1.0,0.0));
    if(span.valid){
      let position=fma(vec3f(span.near),d,o);
      query=plane_sample(vec3f(clamp(position.xy,vec2f(-1.0),vec2f(1.0)),0.0),span.near,true);
      uncertain=span.uncertain||(!span.unbounded&&span.far_lower<span.near_upper);
      time_error=span.near_upper-span.near;
    }
  }
  if(query.hit){return GeometryQuery(query,0.0,plane_rounding(o,query.t)+time_error,select(0u,1u,uncertain));}
  var gap=plane_gap(o,d);gap.status=max(gap.status,select(0u,1u,uncertain));return gap;
}
fn finite_plane(o:vec3f,d:vec3f,axis:u32)->GeometryQuery {
  var query=plane_flat(plane_frame(o,axis),plane_frame(d,axis));
  query.query.p=plane_unframe(query.query.p,axis);
  query.query.n=plane_unframe(query.query.n,axis);return query;
}
