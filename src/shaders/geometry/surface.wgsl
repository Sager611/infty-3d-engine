// Direct carrier-frame approximation; never iterate a normal projection.
struct SurfacePoint {p:vec3f,n:vec3f,uv:vec2f,valid:bool};
fn geometry_surface(point:vec3f,base:SceneSample,object:SceneObject)->SurfacePoint {
  let n=scene_local_vector(object,base.query.n);let kind=object.identity.y;
  var p=point-n*dot(point-base.local,n);var uv=base.query.uv;
  if(kind==5u){uv+=0.5*plane_frame(p-base.local,scene_axis(object)).xy;}
  else if(kind==3u){uv+=p.xz-base.local.xz;}
  else if(kind==1u){p=material_unit(point,n);uv=sphere_uv(p);}
  else if(kind==2u){uv+=goursat_uv(p-base.local,n);}
  let valid=kind!=5u||(all(uv>=vec2f(0.0))&&all(uv<=vec2f(1.0)));
  return SurfacePoint(p,n,uv,valid);
}
// Carrier normal curvature along the ray (local units); flat carriers give 0. Closed carriers never curve less than
// their enclosing sphere, bounding grazing offsets.
fn geometry_curvature(base:SceneSample,d:vec3f,object:SceneObject)->f32 {
  let kind=object.identity.y;
  if(kind==1u){return 1.0;}
  if(kind!=2u){return 0.0;}
  let p=base.local;let n=scene_local_vector(object,base.query.n);let v=scene_local_vector(object,d);
  let u=v-n*dot(v,n);
  // Quartic Hessian diag(12p²-4) over the gradient length.
  let k=dot(u*u,12.0*p*p-vec3f(4.0))/max(1e-30,dot(u,u)*length(goursat_gradient(p)));
  return max(k,inverseSqrt(3.0+sqrt(3.0*object.params.x)));
}
// Carrier-tangent UV derivatives at the displaced point; unwrap spherical seams.
fn geometry_uv_axes(point:vec3f,base:SceneSample,object:SceneObject)->mat2x3f {
  let n=scene_local_vector(object,base.query.n);
  let t=material_unit(cross(n,select(vec3f(1.0,0.0,0.0),vec3f(0.0,1.0,0.0),abs(n.x)>0.9)),vec3f(0.0,1.0,0.0));
  let b=cross(n,t);let step=0x1p-10;
  var x=geometry_surface(point+t*step,base,object).uv-geometry_surface(point-t*step,base,object).uv;
  var y=geometry_surface(point+b*step,base,object).uv-geometry_surface(point-b*step,base,object).uv;
  if(object.identity.y==1u){x.x-=round(x.x);y.x-=round(y.x);}
  return mat2x3f((t*x.x+b*y.x)/(2.0*step),(t*x.y+b*y.y)/(2.0*step));
}
