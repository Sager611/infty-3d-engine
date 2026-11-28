// Analytic sphere intersection and UV mapping.
fn sphere_uv(n: vec3f) -> vec2f {
  let horizontal = length(n.xz);
  let v = atan2(horizontal, n.y) / PI;
  if (horizontal <= 0.0) { return vec2f(0.0, v); }
  return vec2f(fract(atan2(n.z, n.x) / (2.0 * PI) + 0.5), v);
}

fn sphere_at(o: vec3f, d: vec3f, c: vec3f, r: f32) -> Query {
  let oc = o - c;
  let b = dot(oc, d);
  let cc = dot(oc, oc) - r * r;
  let disc = b * b - cc;
  var t = 1e30;
  if (disc >= 0.0) {
    let root = sqrt(disc);
    let q = -b - select(root, -root, b < 0.0);
    if (q != 0.0) {
      let t0 = q;
      let t1 = cc / q;
      if (t0 >= 0.0) { t = min(t, t0); }
      if (t1 >= 0.0) { t = min(t, t1); }
    } else if (-b >= 0.0) {
      t = -b;
    }
  }
  if (t < 1e29) {
    let p = o + t * d;
    let n = normalize(p - c);
    return Query(true, t, 0.0, p, n, sphere_uv(n));
  }

  let closest_t = max(0.0, -b);
  let closest = o + closest_t * d;
  let v = closest - c;
  let l = length(v);
  var n = vec3f(0.0, 1.0, 0.0);
  if (l > 0.0) { n = v / l; }
  let p = c + r * n;
  return Query(false, closest_t, max(l - r, 0.0), p, n, sphere_uv(n));
}

fn sphere(o: vec3f, d: vec3f, c: vec3f, r: f32) -> Query {
  return sphere_at(o, d, c, r);
}

// Geometry interface (geometry::api).
fn sphere_query(o:vec3f,d:vec3f,object:SceneObject)->GeometryQuery {
  let query=sphere_at(o,d,vec3f(0.0),1.0);return GeometryQuery(query,query.gap,0.0,0u);
}
fn sphere_surface_uv(surface:SurfacePoint,point:vec3f,base:SceneSample,object:SceneObject)->SurfacePoint {
  let p=material_unit(point,surface.n);return SurfacePoint(p,surface.n,sphere_uv(p),true);
}
fn sphere_curvature(base:SceneSample,d:vec3f,object:SceneObject)->f32 {return 1.0;}
fn sphere_enclosing_curvature(object:SceneObject)->f32 {return 1.0;}
fn sphere_field_relevant(o:vec3f,d:vec3f,object:SceneObject,limit:f32,padding:f32)->bool {return true;}
fn sphere_field_chord(o:vec3f,d:vec3f,object:SceneObject)->bool {return true;}
fn sphere_shade(kind:u32,q:Query,n:vec3f,local:vec3f,ray:vec3f,height:f32,size:f32,mode:u32,diffuse:f32,shadow:f32)->vec3f {
  let low=vec3f(0.31,0.40,0.48);let high=vec3f(0.48,0.55,0.59);
  if(mode!=2u){return surface_lit(surface_base(sphere_uv(n),12.0,low,high),diffuse,shadow);}
  // UV density grows toward the poles; undefined on the axis.
  let radial=length(q.n.xz);let density=select(max(1.0/PI,1.0/(TAU*radial)),0.0,radial==0.0);
  return surface_lit(mix(low,high,pureweight(sphere_uv(n),surface_rho(q,ray,height,size,density))),diffuse,shadow);
}
