// Per-instance finite surface: immutable summed relief or an unchanged flat carrier.
struct FiniteSurface {geometry:GeometryQuery,pending:Pending,gap_computed:bool};
fn pending_min(a:Pending,b:Pending)->Pending {
  if(!a.valid){return b;}if(!b.valid){return a;}return Pending(min(a.lower,b.lower),true);
}
fn pending_before(pending:Pending,hit:bool,time:f32,error:f32)->bool {
  if(!pending.valid){return false;}if(!hit){return true;}
  let upper=fi_add(fi_exact(time),fi_exact(error));
  return !upper.valid||pending.lower<=upper.v.y;
}
fn hf_miss_gap(o:vec3f,d:vec3f)->GeometryQuery {
  var result=GeometryQuery(plane_sample(vec3f(0.0),0.0,false),0.0,0.0,3u);
  let anchor=hf_sample(vec2f(0.0));
  if(!anchor.height.valid||!anchor.mask.valid||anchor.mask.v.x<127.5){return result;}
  let point=vec3f(0.0,0.0,anchor.scalar);
  let time=max(0.0,dot(point-o,d)/dot(d,d));var squared=fi_exact(0.0);
  for(var axis=0u;axis<3u;axis++){
    var witness=fi_exact(0.0);if(axis==2u){witness=anchor.height;}
    squared=fi_add(squared,fi_square(fi_sub(hf_ray_coordinate(o,d,fi_exact(time),axis),witness)));
  }
  let upper=fi_sqrt(squared);if(!upper.valid){return result;}
  let bounds=hf_bounds(vec3u(0u));let carrier=plane_gap(o,d);var lower=0.0;
  if(bounds.valid&&carrier.status<2u){
    let envelope=max(abs(bounds.lo.z),abs(bounds.hi.z));
    let bound=fi_sub(fi_exact(carrier.gap_lower),fi_exact(envelope));
    if(bound.valid){lower=max(0.0,bound.v.x);}
  }
  let error=fi_sub(anchor.height,fi_exact(anchor.scalar));
  var query=plane_sample(point,time,false);query.gap=upper.v.y;
  query.n=normalize(vec3f(-anchor.gradient,1.0));
  return GeometryQuery(query,lower,max(abs(error.v.x),abs(error.v.y)),0u);
}
fn finite_surface(o:vec3f,d:vec3f,axis:u32,leaf:bool,need_gap:bool)->FiniteSurface {
  if(axis>2u){
    return FiniteSurface(GeometryQuery(plane_sample(vec3f(0.0),0.0,false),0.0,0.0,3u),Pending(0.0,true),false);
  }
  if(!leaf){
    let query=finite_plane(o,d,axis,need_gap);
    return FiniteSurface(query,Pending(0.0,query.status==1u||query.status==3u),query.query.hit||(need_gap&&query.status<2u));
  }
  let local_o=plane_frame(o,axis);let local_d=plane_frame(d,axis);
  if(!hf_header_ok()||!plane_supported(local_o,local_d)){
    return FiniteSurface(GeometryQuery(plane_sample(vec3f(0.0),0.0,false),0.0,0.0,3u),Pending(0.0,true),false);
  }
  let traversal=hf_query(local_o,local_d,64u);
  var query=GeometryQuery(plane_sample(vec3f(0.0),0.0,false),0.0,0.0,2u);var computed=false;
  if(traversal.contact.found){
    let hit=traversal.contact;query.query=plane_sample(hit.p,fi_middle(hit.time),true);
    query.query.n=hit.n;query.error=hit.error;query.status=0u;computed=true;
  }else if(need_gap){query=hf_miss_gap(local_o,local_d);computed=query.status<2u;}
  if(!traversal.complete&&query.status<3u){query.status=1u;}
  query.query.p=plane_unframe(query.query.p,axis);query.query.n=plane_unframe(query.query.n,axis);
  return FiniteSurface(query,Pending(traversal.pending,traversal.pending_valid),computed);
}
