// Geometry interface: generated trait masks and dispatchers (geometry::api); shared code never branches on geometry.
// @geometry-api
// Direct carrier-frame approximation; never iterate a normal projection.
struct SurfacePoint {p:vec3f,n:vec3f,uv:vec2f,valid:bool};
// Projects once onto the sample's tangent plane, keeping its UV; each geometry then maps UV.
fn geometry_surface(point:vec3f,base:SceneSample,object:SceneObject)->SurfacePoint {
  let n=scene_local_vector(object,base.query.n);
  return geometry_surface_uv(SurfacePoint(point-n*dot(point-base.local,n),n,base.query.uv,true),point,base,object);
}
// Carrier-tangent UV derivatives at the displaced point; unwrap wrapping seams.
fn geometry_uv_axes(point:vec3f,base:SceneSample,object:SceneObject)->mat2x3f {
  let n=scene_local_vector(object,base.query.n);
  let t=material_unit(cross(n,select(vec3f(1.0,0.0,0.0),vec3f(0.0,1.0,0.0),abs(n.x)>0.9)),vec3f(0.0,1.0,0.0));
  let b=cross(n,t);let step=0x1p-10;
  var x=geometry_surface(point+t*step,base,object).uv-geometry_surface(point-t*step,base,object).uv;
  var y=geometry_surface(point+b*step,base,object).uv-geometry_surface(point-b*step,base,object).uv;
  if(geometry_wraps_u(object.identity.y)){x.x-=round(x.x);y.x-=round(y.x);}
  return mat2x3f((t*x.x+b*y.x)/(2.0*step),(t*x.y+b*y.y)/(2.0*step));
}
