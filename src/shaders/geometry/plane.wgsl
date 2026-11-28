// Analytic plane intersection.
fn plane(o: vec3f, d: vec3f) -> Query {
  var t = 0.0;
  var hit = false;
  if (d.y != 0.0) {
    let candidate = -o.y / d.y;
    if (candidate >= 0.0) {
      t = candidate;
      hit = true;
    }
  } else {
    hit = o.y == 0.0;
  }
  let p = o + t * d;
  return Query(hit, t, select(abs(o.y), 0.0, hit), vec3f(p.x, 0.0, p.z),
    vec3f(0.0, 1.0, 0.0), vec2f(p.x, p.z));
}

// Geometry interface (geometry::api).
fn plane_query(o:vec3f,d:vec3f,object:SceneObject)->GeometryQuery {
  let query=plane(o,d);return GeometryQuery(query,query.gap,0.0,0u);
}
fn plane_surface_uv(surface:SurfacePoint,point:vec3f,base:SceneSample,object:SceneObject)->SurfacePoint {
  var s=surface;s.uv+=s.p.xz-base.local.xz;return s;
}
fn plane_curvature(base:SceneSample,d:vec3f,object:SceneObject)->f32 {return 0.0;}
fn plane_enclosing_curvature(object:SceneObject)->f32 {return 0.0;}
fn plane_field_relevant(o:vec3f,d:vec3f,object:SceneObject,limit:f32,padding:f32)->bool {return true;}
fn plane_field_chord(o:vec3f,d:vec3f,object:SceneObject)->bool {return true;}
// Evenly lit floor grid.
fn plane_shade(kind:u32,q:Query,n:vec3f,local:vec3f,ray:vec3f,height:f32,size:f32,mode:u32,diffuse:f32,shadow:f32)->vec3f {
  var base=surface_base(q.uv,1.25,SURFACE_LOW,SURFACE_HIGH);
  if(mode==2u){base=mix(SURFACE_LOW,SURFACE_HIGH,pureweight(q.uv,surface_rho(q,ray,height,size,1.0)));}
  return base*(0.6+0.4*diffuse*shadow)+SURFACE_PAPER*0.02;
}
