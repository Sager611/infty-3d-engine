// Threaded scene visibility.
struct Shadow {blocked:bool,uncertain:bool};
fn shadow_value(shadow:Shadow)->f32 {
  if(shadow.blocked){return 0.25;}return select(1.0,0.6,shadow.uncertain);
}
fn shadow_local_origin(source:SceneSample,object:SceneObject,other:SceneObject,n:vec3f)->vec3f {
  var error=source.local_error;
  if(source.kind==5u){error=0.0;} // Exact carrier projection.
  let p=abs(source.local);
  let bias=max(0x1p-30,4.0*max(error,FI_U*max(p.x,max(p.y,p.z))));
  let local=source.local+scene_local_vector(object,n)*bias;
  if(source.id!=0u&&source.id==other.identity.x){return local;}
  var a=vec3f(0.0);var b=vec3f(0.0);
  if(object.identity.y==5u){a=object.params.xyz;}if(other.identity.y==5u){b=other.params.xyz;}
  let delta=(object.position_size.xyz-other.position_size.xyz)+(a-b)+scene_world_vector(object,local)*object.position_size.w;
  return scene_local_vector(other,delta/other.position_size.w);
}
fn shadow_finite_result(sample:SceneSample,other:SceneObject,distance:f32)->Shadow {
  let error=sample.local_error*other.position_size.w+4.0*FI_U*abs(sample.query.t);
  let time=fi_round(max(0.0,sample.query.t-error),sample.query.t+error);
  // A certified existential contact blocks light; frontmost ordering is not required.
  let real_contact=sample.status==0u;
  if(sample.query.hit&&real_contact&&time.valid&&time.v.y<distance){return Shadow(true,false);}
  if(pending_before(sample.pending,true,distance,0.0)||
    (sample.query.hit&&(!time.valid||time.v.x<distance))||sample.status==1u||sample.status==3u){
    return Shadow(false,true);
  }
  return Shadow(false,false);
}
fn scene_shadow(source:SceneSample,object:SceneObject,n:vec3f,light:vec3f,count:u32)->Shadow {
  let p=source.query.p;let to_light=light-p;let distance=length(to_light);
  if(!fi_finite(distance)){return Shadow(false,true);}if(distance<=0.0){return Shadow(false,false);}
  let bias=max(1e-4*object.position_size.w,8.0*EPS*max(1.0,length(p)));
  let origin=p+n*bias;let direction=to_light/distance;
  let leaves_plane=source.kind==5u&&source.query.hit&&dot(n,direction)>=0.0;
  var result=Shadow(false,false);var i=0u;
  while(i<count){
    let record=scene_objects[i];
    if(record.identity.x==0u){
      if(!branch_intersects(record,origin,direction,0.0)){i=record.identity.y;continue;}
    }else{
      let other=resolved_object(i);
      // An outward ray from a biased plane cannot recross it. Leaf omits self-shadows.
      if(source.id==other.identity.x&&(leaves_plane||object.params.w==2.0)){i++;continue;}
      let legacy=source.kind!=5u&&other.identity.y!=5u;
      var local=scene_local_vector(other,(origin-other.position_size.xyz)/other.position_size.w);
      if(!legacy){local=shadow_local_origin(source,object,other,n);}
      var sample=scene_canonical(local,scene_local_vector(other,direction),other);
      if(other.params.w==2.0&&sample.query.hit&&!leaf_covered(leaf_uv_unit(sample.query.uv))){sample.query.hit=false;}
      if(legacy){
        if(sample.query.hit&&sample.query.t>=2.0*bias&&sample.query.t<distance){return Shadow(true,false);}
      }else{
        let blocker=shadow_finite_result(sample,other,distance);
        if(blocker.blocked){return Shadow(true,false);}result.uncertain=result.uncertain||blocker.uncertain;
      }
    }i++;
  }return result;
}
// Legacy verification entry: identical bias/query behaviour when no finite object is involved.
fn shadow_factor(p:vec3f,n:vec3f,size:f32,light:vec3f,count:u32)->f32 {
  var source=empty_sample();source.query.p=p;source.local=p/size;
  let object=SceneObject(vec4u(0u),vec4f(0.0,0.0,0.0,size),vec4f(0.0),POSE_IDENTITY);
  return shadow_value(scene_shadow(source,object,n,light,count));
}
