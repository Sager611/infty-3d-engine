// Canonical scene adapters; finite instance XYZ/unused uniform W retain low origin parts.
struct SceneObject {identity:vec4u,position_size:vec4f,params:vec4f,rotation:vec4f};
const POSE_IDENTITY=vec4f(0.0,0.0,0.0,1.0);
fn scene_rotate(rotation:vec4f,point:vec3f)->vec3f {
  if(all(rotation==POSE_IDENTITY)){return point;}
  let crossed=cross(rotation.xyz,point);
  return point+(2.0/dot(rotation,rotation))*(rotation.w*crossed+cross(rotation.xyz,crossed));
}
fn scene_world_vector(object:SceneObject,point:vec3f)->vec3f {return scene_rotate(object.rotation,point);}
fn scene_local_vector(object:SceneObject,point:vec3f)->vec3f {return scene_rotate(vec4f(-object.rotation.xyz,object.rotation.w),point);}
fn scene_origin_delta(o:vec3f,low:vec3f,object:SceneObject)->vec3f {
  var delta=o-object.position_size.xyz;
  if(object.identity.y==5u){delta+=low-object.params.xyz;}
  return delta;
}
struct SceneSample {
  query:Query,local:vec3f,id:u32,kind:u32,gap_lower:f32,error:f32,status:u32,
  pending:Pending,local_error:f32,base_hit:u32, // Full word avoids Metal's 24-bit padding-read bug.
};
fn scene_axis(object:SceneObject)->u32 {return (object.identity.z>>8u)&3u;}
fn scene_selected(object:SceneObject)->bool {return (object.identity.z&1u)!=0u;}
fn scene_object(o:vec3f,d:vec3f,object:SceneObject)->SceneSample {
  return scene_object_precise(o,vec3f(0.0),d,object);
}
fn scene_object_precise(o:vec3f,low:vec3f,d:vec3f,object:SceneObject)->SceneSample {
  let local=scene_local_vector(object,scene_origin_delta(o,low,object)/object.position_size.w);
  return scene_canonical(local,scene_local_vector(object,d),object,true);
}
// Full height envelope includes direct hits and witness-gap hits. Reject only
// a certified miss/occluded leaf; never run nine-region gap math for it.
fn scene_leaf_relevant(local_o:vec3f,local_d:vec3f,object:SceneObject,limit:f32,padding:f32)->bool {
  if(object.identity.y!=5u||object.params.w!=2.0){return true;}
  let o=plane_frame(local_o,scene_axis(object));let d=plane_frame(local_d,scene_axis(object));
  let reach=fi_up(hf_reach());
  let bounds=vec3f(1.0+reach,1.0+reach,reach)+vec3f(padding+plane_rounding(o,length(o)+2.0));
  var span=plane_axis(plane_range(),o.z,d.z,-bounds.z,bounds.z);
  span=plane_axis(span,o.x,d.x,-bounds.x,bounds.x);
  span=plane_axis(span,o.y,d.y,-bounds.y,bounds.y);
  if(span.uncertain){return true;}
  if(!span.valid||span.near>fi_up(limit/object.position_size.w)){return false;}
  // With no origin minimum or carrier crossing, a nonparallel ray's closest
  // witness is on the square boundary. A zero-opacity border cannot cover it.
  if(padding==0.0&&height_data.info.w!=0u&&d.z!=0.0&&plane_rounding(o,length(o)+2.0)*f32(height_data.info.x)<0.25){
    let carrier=plane_box(o,d,vec3f(-1.0,-1.0,0.0),vec3f(1.0,1.0,0.0));
    let derivative=plane_origin_derivative(o,d);
    if(!carrier.valid&&!carrier.uncertain&&derivative.valid&&derivative.v.y<0.0){return false;}
  }
  return true;
}
// Relief may land on this object: a carrier hit, or a near miss under its highest relief; a card's relief slab
// must also cross its coverage outline. Lookup-free, so traversal cost stays independent of overlaps.
fn scene_relief_candidate(carrier:SceneSample,local_o:vec3f,local_d:vec3f,object:SceneObject,relief:f32)->bool {
  if(carrier.base_hit==0u&&(carrier.status!=0u||carrier.query.gap>=relief*material_spec(u32(object.params.w)).rise)){return false;}
  if(!material_field(object.params.w)||object.identity.y!=5u){return true;}
  let o=plane_frame(local_o,scene_axis(object));let d=plane_frame(local_d,scene_axis(object));
  if(abs(d.z)<1e-6){return true;}
  let reach=fi_up(hf_reach());let a=o.xy+d.xy*((reach-o.z)/d.z);let b=o.xy+d.xy*((-reach-o.z)/d.z);
  return field_outline(fma(a,vec2f(0.5),vec2f(0.5)),fma(b,vec2f(0.5),vec2f(0.5)));
}
// Exact adds field lookups (coverage, witness-gap relief); traversal passes false and lets the one relief pass decide.
fn scene_canonical(local_o:vec3f,d:vec3f,obj:SceneObject,exact:bool)->SceneSample {
  let id=obj.identity.x;let kind=obj.identity.y;let position=obj.position_size.xyz;let size=obj.position_size.w;
  var query=Query(false,0.0,0.0,local_o,vec3f(0.0,1.0,0.0),vec2f(0.0));
  var gap_lower=0.0;var error=0.0;var status=3u;var pending=Pending(0.0,false);
  if(kind==1u){
    query=sphere_at(local_o,d,vec3f(0.0),1.0);gap_lower=query.gap;status=0u;
  }else if(kind==2u||kind==4u){
    var canonical:GeometryQuery;
    if(kind==2u){canonical=goursat(local_o,d,obj.params.x);}else{canonical=crystal(local_o,d);}
    query=canonical.query;gap_lower=canonical.gap_lower;error=canonical.error;status=canonical.status;
  }else if(kind==5u){
    var canonical:GeometryQuery;
    canonical=finite_plane(local_o,d,scene_axis(obj));
    pending=Pending(0.0,canonical.status==1u||canonical.status==3u);
    query=canonical.query;gap_lower=canonical.gap_lower;error=canonical.error;status=canonical.status;
  }else if(kind==3u){query=plane(local_o,d);gap_lower=query.gap;status=0u;}
  let base_hit=query.hit;
  if(material_field(obj.params.w)&&kind==5u&&base_hit){
    if(exact){query.hit=leaf_covered(query.uv);}else{query.hit=field_outline(query.uv,query.uv);}
  }
  let local_error=error;let local=query.p;let canonical_gap=query.gap;let canonical_lower=gap_lower;
  if(exact&&material_field(obj.params.w)&&!base_hit&&status==0u){
    query=texh(local_o,d,query,gap_lower,kind==5u);
    if(query.hit){gap_lower=0.0;}
  }
  query.p=position+size*scene_world_vector(obj,query.p);query.n=scene_world_vector(obj,query.n);
  query.t*=size;query.gap*=size;gap_lower*=size;error*=size;
  if(pending.valid){
    let bound=fi_scale(fi_exact(pending.lower),size);
    pending.lower=select(0.0,max(0.0,bound.v.x),bound.valid);
  }
  if(kind==5u){
    query.p+=obj.params.xyz;
    let magnitude=abs(position)+vec3f(size*length(local))+abs(obj.params.xyz);
    let operations=select(32.0,8.0,all(obj.rotation==POSE_IDENTITY));
    let transform_error=operations*FI_U*max(1.0,max(magnitude.x,max(magnitude.y,magnitude.z)));
    error+=transform_error;
    if(pending.valid){pending.lower=max(0.0,fi_down(pending.lower-transform_error));}
    if(status<2u&&!query.hit){
      let lower=fi_sub(fi_scale(fi_exact(canonical_lower),size),fi_exact(transform_error));
      let upper=fi_add(fi_scale(fi_exact(canonical_gap),size),fi_exact(transform_error));
      if(lower.valid&&upper.valid){gap_lower=max(0.0,lower.v.x);query.gap=upper.v.y;}
      else{status=3u;gap_lower=0.0;query.gap=0.0;}
    }
  }
  return SceneSample(query,local,id,kind,gap_lower,error,status,pending,local_error,u32(base_hit));
}

fn pending_min(a:Pending,b:Pending)->Pending {
  if(!a.valid){return b;}if(!b.valid){return a;}return Pending(min(a.lower,b.lower),true);
}
fn pending_before(pending:Pending,hit:bool,time:f32,error:f32)->bool {
  if(!pending.valid){return false;}if(!hit){return true;}
  let upper=fi_add(fi_exact(time),fi_exact(error));
  return !upper.valid||pending.lower<=upper.v.y;
}
