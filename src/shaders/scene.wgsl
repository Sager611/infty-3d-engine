// Canonical scene adapters; finite instance XYZ/unused uniform W retain low origin parts.
struct SceneObject {identity:vec4u,position_size:vec4f,params:vec4f};
struct SceneSample {
  query:Query,local:vec3f,id:u32,kind:u32,gap_lower:f32,error:f32,status:u32,
  pending:Pending,local_error:f32,
};
fn scene_axis(object:SceneObject)->u32 {return (object.identity.z>>8u)&3u;}
fn scene_selected(object:SceneObject)->bool {return (object.identity.z&1u)!=0u;}
fn scene_object(o:vec3f,d:vec3f,object:SceneObject)->SceneSample {
  return scene_object_precise(o,vec3f(0.0),d,object);
}
fn scene_object_precise(o:vec3f,low:vec3f,d:vec3f,object:SceneObject)->SceneSample {
  var local=(o-object.position_size.xyz)/object.position_size.w;
  if(object.identity.y==5u){
    local=((o-object.position_size.xyz)+(low-object.params.xyz))/object.position_size.w;
  }
  return scene_canonical(local,d,object);
}
fn scene_canonical(local_o:vec3f,d:vec3f,obj:SceneObject)->SceneSample {
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
    if(obj.params.w==2.0){canonical=finite_leaf(local_o,d,scene_axis(obj));}
    else{canonical=finite_plane(local_o,d,scene_axis(obj));}
    pending=Pending(0.0,canonical.status==1u||canonical.status==3u);
    query=canonical.query;gap_lower=canonical.gap_lower;error=canonical.error;status=canonical.status;
  }else if(kind==3u){query=plane(local_o,d);gap_lower=query.gap;status=0u;}
  let local_error=error;let local=query.p;let canonical_gap=query.gap;let canonical_lower=gap_lower;
  if(obj.params.w==2.0&&kind!=5u&&!query.hit&&status==0u){
    query=texh(local_o,d,query,gap_lower,false);
    if(query.hit){gap_lower=0.0;}
  }
  query.p=position+size*query.p;query.t*=size;query.gap*=size;gap_lower*=size;error*=size;
  if(pending.valid){
    let bound=fi_scale(fi_exact(pending.lower),size);
    pending.lower=select(0.0,max(0.0,bound.v.x),bound.valid);
  }
  if(kind==5u){
    query.p+=obj.params.xyz;
    let magnitude=abs(position)+abs(size*local)+abs(obj.params.xyz);
    let transform_error=8.0*FI_U*max(1.0,max(magnitude.x,max(magnitude.y,magnitude.z)));
    error+=transform_error;
    if(pending.valid){pending.lower=max(0.0,fi_down(pending.lower-transform_error));}
    if(status<2u&&!query.hit){
      let lower=fi_sub(fi_scale(fi_exact(canonical_lower),size),fi_exact(transform_error));
      let upper=fi_add(fi_scale(fi_exact(canonical_gap),size),fi_exact(transform_error));
      if(lower.valid&&upper.valid){gap_lower=max(0.0,lower.v.x);query.gap=upper.v.y;}
      else{status=3u;gap_lower=0.0;query.gap=0.0;}
    }
  }
  return SceneSample(query,local,id,kind,gap_lower,error,status,pending,local_error);
}

fn pending_min(a:Pending,b:Pending)->Pending {
  if(!a.valid){return b;}if(!b.valid){return a;}return Pending(min(a.lower,b.lower),true);
}
fn pending_before(pending:Pending,hit:bool,time:f32,error:f32)->bool {
  if(!pending.valid){return false;}if(!hit){return true;}
  let upper=fi_add(fi_exact(time),fi_exact(error));
  return !upper.valid||pending.lower<=upper.v.y;
}
