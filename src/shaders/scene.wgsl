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
  if(geometry_card(object.identity.y)){delta+=low-object.params.xyz;}
  return delta;
}
struct SceneSample {
  query:Query,local:vec3f,id:u32,kind:u32,gap_lower:f32,error:f32,status:u32,
  pending:Pending,local_error:f32,base_hit:u32, // Full word avoids Metal's 24-bit padding-read bug.
};
fn scene_selected(object:SceneObject)->bool {return (object.identity.z&1u)!=0u;}
fn scene_object(o:vec3f,d:vec3f,object:SceneObject)->SceneSample {
  return scene_object_precise(o,vec3f(0.0),d,object);
}
fn scene_object_precise(o:vec3f,low:vec3f,d:vec3f,object:SceneObject)->SceneSample {
  let local=scene_local_vector(object,scene_origin_delta(o,low,object)/object.position_size.w);
  return scene_canonical(local,scene_local_vector(object,d),object,true);
}
// Field reliefs skip only through their geometry's conservative envelope; never run nine-region gap math.
fn scene_leaf_relevant(local_o:vec3f,local_d:vec3f,object:SceneObject,limit:f32,padding:f32)->bool {
  if(!material_field(object.params.w)){return true;}
  return geometry_field_relevant(local_o,local_d,object,limit,padding);
}
// Relief may land on this object: a carrier hit, or a near miss under its highest relief; a card's relief slab
// must also cross its coverage outline. Lookup-free, so traversal cost stays independent of overlaps.
fn scene_relief_candidate(carrier:SceneSample,local_o:vec3f,local_d:vec3f,object:SceneObject,relief:f32)->bool {
  // Fields keep their witness rule: a near miss whose lower gap lies within the field's height reach.
  let field=material_field(object.params.w);
  let bound=relief*select(material_spec(u32(object.params.w)).rise,fi_up(hf_reach())*object.position_size.w,field);
  if(carrier.base_hit==0u&&(carrier.status!=0u||select(carrier.query.gap,carrier.gap_lower,field)>=bound)){return false;}
  if(!material_field(object.params.w)){return true;}
  return geometry_field_chord(local_o,local_d,object);
}
// Exact adds field lookups (coverage, witness-gap relief); traversal passes false and lets the one relief pass decide.
fn scene_canonical(local_o:vec3f,d:vec3f,obj:SceneObject,exact:bool)->SceneSample {
  let id=obj.identity.x;let kind=obj.identity.y;let position=obj.position_size.xyz;let size=obj.position_size.w;
  let card=geometry_card(kind);let canonical=geometry_query(local_o,d,obj);
  var query=canonical.query;var gap_lower=canonical.gap_lower;var error=canonical.error;var status=canonical.status;
  var pending=Pending(0.0,card&&(status==1u||status==3u));
  let base_hit=query.hit;
  if(material_field(obj.params.w)&&card&&base_hit){
    if(exact){query.hit=leaf_covered(query.uv);}else{query.hit=field_outline(query.uv,query.uv);}
  }
  let local_error=error;let local=query.p;let canonical_gap=query.gap;let canonical_lower=gap_lower;
  if(exact&&material_field(obj.params.w)&&!base_hit&&status==0u){
    query=texh(local_o,d,query,gap_lower,card);
    if(query.hit){gap_lower=0.0;}
  }
  query.p=position+size*scene_world_vector(obj,query.p);query.n=scene_world_vector(obj,query.n);
  query.t*=size;query.gap*=size;gap_lower*=size;error*=size;
  if(pending.valid){
    let bound=fi_scale(fi_exact(pending.lower),size);
    pending.lower=select(0.0,max(0.0,bound.v.x),bound.valid);
  }
  if(card){
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
