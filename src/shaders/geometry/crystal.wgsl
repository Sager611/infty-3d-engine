// Binary-rational corrugations; direct exponent addressing, fixed analytic calls.
const CRYSTAL_MIN: f32 = 0.00000095367431640625;
const CRYSTAL_RADIUS: f32 = 1.75;
const CRYSTAL_U: f32 = 1.1920928955078125e-7;
const CRYSTAL_FAR: f32 = 1e30;

struct CrystalFoot { p: vec3f, n: vec3f, uv: vec2f, upper: f32, error: f32 };
struct CrystalRoot { t: f32, error: f32, status: u32 };
struct CrystalSpan { lo: f32, hi: f32, error: f32, status: u32 };
struct CrystalShot { root: CrystalRoot, segment: vec4f, axis: u32 };
struct CrystalCoefficients { value: vec3f, error: vec3f, shift: f32 };

fn crystal_power(value: f32) -> f32 {
  return bitcast<f32>(bitcast<u32>(clamp(value, 0.5 * CRYSTAL_MIN, 1.0)) & 0x7f800000u);
}
fn crystal_rise(s: f32) -> vec4f { return vec4f(s, 0.5*s, 0.5*s, 2.5*s); }
fn crystal_fall(s: f32) -> vec4f { return vec4f(1.5*s, 3.0*s, 0.5*s, -2.0*s); }
fn crystal_core() -> vec4f { return vec4f(0.0, 0.0, CRYSTAL_MIN, 0.5*CRYSTAL_MIN); }
fn crystal_uv(p: vec3f, n: vec3f) -> vec2f {
  let a = abs(n);
  if (a.x >= a.y && a.x >= a.z) { return p.yz; }
  if (a.y >= a.z) { return p.xz; }
  return p.xy;
}
fn crystal_segment(p: vec3f, segment: vec4f) -> CrystalFoot {
  let r = length(p.xy);
  var direction = vec2f(1.0, 0.0);
  if (r > 0.0) { direction = p.xy / r; }
  let t = clamp(dot(vec2f(r, p.z) - segment.xy, segment.zw) / dot(segment.zw, segment.zw), 0.0, 1.0);
  let q2 = fma(vec2f(t), segment.zw, segment.xy);
  let q = vec3f(q2.x * direction, q2.y);
  let slope = segment.w / segment.z;
  let n = normalize(vec3f(-slope * direction, 1.0));
  let error = 64.0 * CRYSTAL_U * max(1e-30, max(max(abs(q.x), abs(q.y)), abs(q.z)));
  // Cover the squared-length underflow range, including flush-to-zero devices.
  let distance_error = max(1e-18, 64.0 * CRYSTAL_U * (length(p) + length(q)));
  return CrystalFoot(q, n, crystal_uv(q, n), length(p-q) + error + distance_error, error);
}
fn crystal_best_foot(a: CrystalFoot, b: CrystalFoot) -> CrystalFoot {
  if (b.upper < a.upper) { return b; }
  return a;
}
fn crystal_point_component(p: vec3f) -> CrystalFoot {
  let r = length(p.xy);
  var q = vec2f(r, p.z);
  if (p.z < 0.5*r) {
    let t = max(0.0, (4.0*r + 2.0*p.z) / 5.0);
    q = vec2f(t, 0.5*t);
  } else if (p.z > 2.0*r) {
    let t = (r + 2.0*p.z) / 5.0;
    q = vec2f(t, 2.0*t);
  }
  let a = crystal_power(clamp((5.0*q.x - q.y) / 4.5, CRYSTAL_MIN, 0.5));
  let b = crystal_power(clamp((4.0*q.x + q.y) / 9.0, CRYSTAL_MIN, 0.5));
  let rising = crystal_best_foot(crystal_segment(p, crystal_rise(a)),
    crystal_segment(p, crystal_rise(min(0.5, 2.0*a))));
  let falling = crystal_best_foot(crystal_segment(p, crystal_fall(b)),
    crystal_segment(p, crystal_fall(min(0.5, 2.0*b))));
  return crystal_best_foot(crystal_best_foot(rising, falling), crystal_segment(p, crystal_core()));
}
fn crystal_turn_foot(q: CrystalFoot) -> CrystalFoot {
  return CrystalFoot(q.p.zxy, q.n.zxy, q.uv, q.upper, q.error);
}
fn crystal_closest(p: vec3f) -> CrystalFoot {
  return crystal_best_foot(crystal_best_foot(crystal_point_component(p),
    crystal_turn_foot(crystal_point_component(p.yzx))),
    crystal_turn_foot(crystal_turn_foot(crystal_point_component(p.zxy))));
}
fn crystal_no_root(status: u32) -> CrystalRoot { return CrystalRoot(CRYSTAL_FAR, 0.0, status); }
fn crystal_no_shot() -> CrystalShot { return CrystalShot(crystal_no_root(0u), crystal_core(), 0u); }
fn crystal_best_shot(a: CrystalShot, b: CrystalShot) -> CrystalShot {
  var q = a;
  if (b.root.t < a.root.t) { q = b; }
  q.root.status = max(a.root.status, b.root.status);
  return q;
}
fn crystal_span(lo: f32, hi: f32) -> CrystalSpan { return CrystalSpan(lo, hi, 0.0, 0u); }
fn crystal_span_intersection(a: CrystalSpan, b: CrystalSpan) -> CrystalSpan {
  return CrystalSpan(max(a.lo, b.lo), min(a.hi, b.hi), a.error + b.error, max(a.status,b.status));
}
fn crystal_coefficients(o: vec3f, d: vec3f, slope: f32, intercept: f32) -> CrystalCoefficients {
  let shift = -dot(o,d) / dot(d,d);
  let center = fma(d, vec3f(shift), o);
  let z = center.z - intercept;
  let m2 = slope*slope;
  let a = d.z*d.z - m2*dot(d.xy,d.xy);
  let b = 2.0*(z*d.z - m2*dot(center.xy,d.xy));
  let c = z*z - m2*dot(center.xy,center.xy);
  let delta = 64.0 * CRYSTAL_U * max(1e-30, length(center));
  let ea = 64.0 * CRYSTAL_U * (d.z*d.z + m2*dot(d.xy,d.xy));
  let eb = 128.0 * CRYSTAL_U * (abs(z*d.z) + m2*dot(abs(center.xy),abs(d.xy)))
    + 4.0*delta*(abs(d.z) + m2*(abs(d.x)+abs(d.y)));
  let ec = 64.0 * CRYSTAL_U * (z*z + m2*dot(center.xy,center.xy))
    + 4.0*delta*(abs(z)+m2*(abs(center.x)+abs(center.y))) + 8.0*(1.0+m2)*delta*delta;
  return CrystalCoefficients(vec3f(a,b,c),vec3f(ea,eb,ec),shift);
}
fn crystal_root(t: f32, error: f32, shift: f32, status: u32) -> CrystalRoot {
  let total = t + shift;
  let allowance = error + 32.0*CRYSTAL_U*(abs(t)+abs(shift));
  return CrystalRoot(total, allowance, max(status, select(0u,1u,allowance>0.0002*max(CRYSTAL_MIN,abs(total)))));
}
fn crystal_roots(c: CrystalCoefficients) -> array<CrystalRoot,2> {
  let a=c.value.x; let b=c.value.y; let v=c.value.z;
  let ea=c.error.x; let eb=c.error.y; let ec=c.error.z;
  if (a==0.0) {
    if (b==0.0) { return array<CrystalRoot,2>(crystal_no_root(1u),crystal_no_root(1u)); }
    let t=-v/b;
    let error=(ec+abs(t)*eb)/max(abs(b)-eb,1e-30);
    return array<CrystalRoot,2>(crystal_root(t,error,c.shift,1u),crystal_no_root(0u));
  }
  let disc=fma(b,b,-4.0*a*v);
  let de=64.0*CRYSTAL_U*(b*b+4.0*abs(a*v)) + 2.0*abs(b)*eb
    + 4.0*(abs(v)*ea+abs(a)*ec) + eb*eb+4.0*ea*ec;
  if (disc < -de) { return array<CrystalRoot,2>(crystal_no_root(0u),crystal_no_root(0u)); }
  let square=sqrt(max(0.0,disc));
  let square_error=max(square-sqrt(max(0.0,disc-de)),sqrt(max(0.0,disc+de))-square);
  let sign=select(-1.0,1.0,b>=0.0);
  let q=-0.5*(b+sign*square);
  let qe=0.5*(eb+square_error+16.0*CRYSTAL_U*(abs(b)+square));
  let status=select(0u,1u,abs(a)<=ea || disc<=de);
  var left=crystal_no_root(status);
  var right=left;
  if (q==0.0) {
    let t=-b/(2.0*a);
    let error=(eb+2.0*abs(t)*ea)/max(2.0*(abs(a)-ea),1e-30);
    left=crystal_root(t,error,c.shift,1u);right=left;
  } else {
    let x=q/a;let y=v/q;
    let ex=(qe+abs(x)*ea)/max(abs(a)-ea,1e-30);
    let ey=(ec+abs(y)*qe)/max(abs(q)-qe,1e-30);
    left=crystal_root(x,min(ex,CRYSTAL_FAR),c.shift,status);
    right=crystal_root(y,min(ey,CRYSTAL_FAR),c.shift,status);
  }
  if (left.t>right.t) {return array<CrystalRoot,2>(right,left);}
  return array<CrystalRoot,2>(left,right);
}
fn crystal_cone(o: vec3f,d: vec3f,slope: f32,strict: bool) -> CrystalSpan {
  let coefficients=crystal_coefficients(o,d,slope,0.0);
  let a=coefficients.value.x;
  let rr=crystal_roots(coefficients);
  if (strict && all(abs(coefficients.value)<=coefficients.error)) {return CrystalSpan(1.0,0.0,0.0,1u);}
  var q=crystal_span(0.0,CRYSTAL_FAR);
  if (a>0.0) {
    var crossing=rr[0];
    if (d.z>0.0) {crossing=rr[1];q.lo=crossing.t;} else {q.hi=crossing.t;}
    if (crossing.t>=CRYSTAL_FAR) {return CrystalSpan(1.0,0.0,0.0,crossing.status);}
    q.error=crossing.error;q.status=crossing.status;
  } else if (a<0.0) {
    if (rr[0].t>=CRYSTAL_FAR) {return CrystalSpan(1.0,0.0,0.0,rr[0].status);}
    q=CrystalSpan(rr[0].t,rr[1].t,max(rr[0].error,rr[1].error),max(rr[0].status,rr[1].status));
  } else {
    let b=coefficients.value.y;let c=coefficients.value.z;
    if (b>0.0) {q.lo=coefficients.shift-c/b;}
    else if (b<0.0) {q.hi=coefficients.shift-c/b;}
    else if (c<0.0) {return CrystalSpan(1.0,0.0,0.0,1u);}
    q.status=1u;
  }
  if (d.z>0.0) {q.lo=max(q.lo,-o.z/d.z);}
  else if (d.z<0.0) {q.hi=min(q.hi,-o.z/d.z);}
  else if (o.z<0.0) {return crystal_span(1.0,0.0);}
  q.lo=max(0.0,q.lo);
  return q;
}
fn crystal_cylinder(o: vec3f,d: vec3f,radius: f32) -> CrystalSpan {
  let a=dot(d.xy,d.xy);
  if (a==0.0) {
    if (dot(o.xy,o.xy)<=radius*radius) {return crystal_span(0.0,CRYSTAL_FAR);}
    return crystal_span(1.0,0.0);
  }
  let shift=-dot(o.xy,d.xy)/a;
  let near=fma(d.xy,vec2f(shift),o.xy);
  let square=(radius*radius-dot(near,near))/a;
  let error=64.0*CRYSTAL_U*(radius*radius+dot(near,near))/a;
  if (square < -error) {return crystal_span(1.0,0.0);}
  let extent=sqrt(max(0.0,square));
  let allowance=max(extent-sqrt(max(0.0,square-error)),sqrt(max(0.0,square+error))-extent)
    + 32.0*CRYSTAL_U*(abs(shift)+extent);
  return CrystalSpan(max(0.0,shift-extent),shift+extent,allowance,select(0u,1u,square<=error));
}
fn crystal_accept(o: vec3f,d: vec3f,segment: vec4f,span: CrystalSpan,root: CrystalRoot) -> CrystalShot {
  if (root.t>=CRYSTAL_FAR) {return crystal_no_shot();}
  let time_allowance=span.error+root.error+64.0*CRYSTAL_U*(length(o)+abs(root.t));
  if (root.t < 0.0) {
    var miss = crystal_no_shot();
    miss.root.status = max(root.status, select(0u, 1u, root.t+root.error>=0.0));
    return miss;
  }
  if (root.t < span.lo-time_allowance || root.t > span.hi+time_allowance) {
    return crystal_no_shot();
  }
  let p=fma(d,vec3f(max(0.0,root.t)),o);
  let r=length(p.xy);
  let slope=segment.w/segment.z;
  let intercept=fma(-slope,segment.x,segment.y);
  let residual=p.z-fma(slope,r,intercept);
  let positional=64.0*CRYSTAL_U*max(1e-30,length(o)+abs(root.t));
  let radial_allowance=16.0*CRYSTAL_U*max(r,segment.x+segment.z);
  if (r<segment.x-radial_allowance || r>segment.x+segment.z+radial_allowance ||
      (p.z-intercept)*slope<0.0 || abs(residual)>16.0*positional) {
    return crystal_no_shot();
  }
  let edge=r-segment.x<=positional || segment.x+segment.z-r<=positional;
  let accepted=CrystalRoot(max(0.0,root.t),root.error+positional,max(max(root.status,span.status),select(0u,1u,edge)));
  return CrystalShot(accepted,segment,0u);
}
fn crystal_intersect(o: vec3f,d: vec3f,segment: vec4f,span: CrystalSpan) -> CrystalShot {
  if (span.lo>span.hi) {return crystal_no_shot();}
  let slope=segment.w/segment.z;
  let coefficients=crystal_coefficients(o,d,slope,fma(-slope,segment.x,segment.y));
  if (all(coefficients.value==vec3f(0.0))) {
    let outside=crystal_span_intersection(crystal_cylinder(o,d,segment.x+segment.z),span);
    let inside=crystal_cylinder(o,d,segment.x);
    var t=outside.lo;
    if (t>=inside.lo && t<inside.hi) {t=inside.hi;}
    return crystal_accept(o,d,segment,outside,CrystalRoot(t,outside.error+inside.error,1u));
  }
  let rr=crystal_roots(coefficients);
  var q=crystal_best_shot(crystal_accept(o,d,segment,span,rr[0]),crystal_accept(o,d,segment,span,rr[1]));
  q.root.status=max(q.root.status,max(rr[0].status,rr[1].status));
  return q;
}
fn crystal_band(o: vec3f,d: vec3f,scale: f32,above: bool,span: CrystalSpan) -> CrystalShot {
  var a=crystal_no_shot();var b=a;
  var rising=scale;
  if (above) {rising=2.0*scale;}
  if (scale>=CRYSTAL_MIN && scale<=0.5) {a=crystal_intersect(o,d,crystal_fall(scale),span);}
  if (rising>=CRYSTAL_MIN && rising<=0.5) {b=crystal_intersect(o,d,crystal_rise(rising),span);}
  return crystal_best_shot(a,b);
}
fn crystal_window(o: vec3f,d: vec3f,span: CrystalSpan) -> CrystalShot {
  if (span.lo>span.hi || span.lo>=CRYSTAL_FAR) {return crystal_no_shot();}
  let p=fma(d,vec3f(span.lo),o);
  let r=length(p.xy);let s=min(0.5,crystal_power(max(r,CRYSTAL_MIN)));
  var height=0.5*r;
  if (r>=CRYSTAL_MIN) {
    if (r<=1.5*s) {height=fma(5.0,r,-4.5*s);} else {height=fma(-4.0,r,9.0*s);}
  }
  let above=p.z>=height;
  let scale=crystal_power(max(r,CRYSTAL_MIN)/select(1.0,1.5,above));
  return crystal_best_shot(crystal_best_shot(crystal_band(o,d,0.5*scale,above,span),
    crystal_band(o,d,scale,above,span)),crystal_band(o,d,2.0*scale,above,span));
}
fn crystal_component(o: vec3f,d: vec3f) -> CrystalShot {
  if ((o.z<0.0 && d.z<=0.0) || (o.z>1.5 && d.z>=0.0)) {return crystal_no_shot();}
  let outside=crystal_span_intersection(crystal_cone(o,d,0.5,false),crystal_cylinder(o,d,1.0));
  let inside=crystal_span_intersection(crystal_cone(o,d,2.0,true),outside);
  var q=crystal_intersect(o,d,crystal_core(),crystal_span(0.0,CRYSTAL_FAR));
  if (outside.lo<=outside.hi) {
    if (inside.lo>inside.hi) {q=crystal_best_shot(q,crystal_window(o,d,outside));}
    else {
      let state=max(outside.status,inside.status);let error=outside.error+inside.error;
      let a=CrystalSpan(outside.lo,min(inside.lo,outside.hi),error,state);
      let b=CrystalSpan(max(inside.hi,outside.lo),outside.hi,error,state);
      q=crystal_best_shot(q,crystal_best_shot(crystal_window(o,d,a),crystal_window(o,d,b)));
    }
  }
  q.root.status=max(q.root.status,max(outside.status,inside.status));
  return q;
}
fn crystal(o: vec3f,d: vec3f) -> GeometryQuery {
  let empty=Query(false,0.0,0.0,vec3f(0.0),vec3f(0.0,0.0,1.0),vec2f(0.0));
  if (!all(abs(o)<vec3f(1e15)) || !(abs(dot(d,d)-1.0)<=32.0*CRYSTAL_U)) {
    return GeometryQuery(empty,0.0,0.0,3u);
  }
  let a=crystal_component(o,d);
  var b=crystal_component(o.yzx,d.yzx);b.axis=1u;
  var c=crystal_component(o.zxy,d.zxy);c.axis=2u;
  let shot=crystal_best_shot(crystal_best_shot(a,b),c);
  if (shot.root.t<CRYSTAL_FAR) {
    let p=fma(d,vec3f(shot.root.t),o);
    var local=p;
    if (shot.axis==1u) {local=p.yzx;} else if (shot.axis==2u) {local=p.zxy;}
    var q=crystal_segment(local,shot.segment);
    if (shot.axis==1u) {q=crystal_turn_foot(q);}
    else if (shot.axis==2u) {q=crystal_turn_foot(crystal_turn_foot(q));}
    return GeometryQuery(Query(true,shot.root.t,0.0,q.p,q.n,q.uv),0.0,
      shot.root.error+q.upper,shot.root.status);
  }
  let a2=dot(d,d);let od=dot(o,d);let t=max(0.0,-od/a2);
  let p=fma(d,vec3f(t),o);let q=crystal_closest(p);
  let roundoff=64.0*CRYSTAL_U*(length(o)+abs(t));
  let gap=q.upper+roundoff;
  let projection=od*od/a2;
  let square=dot(o,o)-projection;
  let square_error=128.0*CRYSTAL_U*(dot(o,o)+projection);
  let lower=max(0.0,sqrt(max(0.0,square-square_error))-CRYSTAL_RADIUS);
  return GeometryQuery(Query(false,t,gap,q.p,q.n,q.uv),lower,q.error,shot.root.status);
}
