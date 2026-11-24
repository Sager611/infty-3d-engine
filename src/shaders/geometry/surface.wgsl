// Direct carrier-frame approximation; never iterate a normal projection.
struct SurfacePoint {p:vec3f,n:vec3f,uv:vec2f,valid:bool};
fn geometry_surface(point:vec3f,base:SceneSample,object:SceneObject)->SurfacePoint {
  let n=base.query.n;let kind=object.identity.y;
  var p=point-n*dot(point-base.local,n);var uv=base.query.uv;
  if(kind==5u){uv+=0.5*plane_frame(p-base.local,scene_axis(object)).xy;}
  else if(kind==3u){uv+=p.xz-base.local.xz;}
  else if(kind==1u){p=material_unit(point,n);uv=sphere_uv(p);}
  else if(kind==2u){uv+=goursat_uv(p-base.local,n);}
  else if(kind==4u){let foot=crystal_closest(point);p=foot.p;uv=foot.uv;}
  let valid=kind!=5u||(all(uv>=vec2f(0.0))&&all(uv<=vec2f(1.0)));
  return SurfacePoint(p,n,uv,valid);
}
