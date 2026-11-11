// Outward f32 enclosures. Invalid is typed state, never a finite numeric sentinel.
const FI_U:f32=0x1p-23;
const FI_MIN:f32=0x1p-126;
const FI_LIMIT:f32=0x1p+120;
struct FInterval {v:vec2f,valid:bool};
fn fi_finite(x:f32)->bool {return (bitcast<u32>(x)&0x7f800000u)!=0x7f800000u;}
fn fi_invalid()->FInterval {return FInterval(vec2f(0.0),false);}
fn fi_subnormal(x:f32)->bool {
  let bits=bitcast<u32>(x)&0x7fffffffu;return bits>0u&&bits<0x00800000u;
}
fn fi_exact(x:f32)->FInterval {
  if(fi_subnormal(x)){return FInterval(vec2f(-FI_MIN,FI_MIN),true);}
  return FInterval(vec2f(x),fi_finite(x)&&abs(x)<=FI_LIMIT);
}
fn fi_down(x:f32)->f32 {
  if(abs(x)<FI_MIN){return -FI_MIN;}
  let bits=bitcast<u32>(x);return bitcast<f32>(select(bits+1u,bits-1u,x>0.0));
}
fn fi_up(x:f32)->f32 {
  if(abs(x)<FI_MIN){return FI_MIN;}
  let bits=bitcast<u32>(x);return bitcast<f32>(select(bits-1u,bits+1u,x>0.0));
}
fn fi_round(lo:f32,hi:f32)->FInterval {
  if(!fi_finite(lo)||!fi_finite(hi)||max(abs(lo),abs(hi))>FI_LIMIT||lo>hi){return fi_invalid();}
  return FInterval(vec2f(fi_down(lo),fi_up(hi)),true);
}
fn fi_zero(a:FInterval)->bool {return a.valid&&all(a.v==vec2f(0.0));}
fn fi_contains_zero(a:FInterval)->bool {return a.valid&&a.v.x<=0.0&&a.v.y>=0.0;}
fn fi_middle(a:FInterval)->f32 {return a.v.x+0.5*(a.v.y-a.v.x);}
fn fi_neg(a:FInterval)->FInterval {return FInterval(-a.v.yx,a.valid);}
fn fi_add(a:FInterval,b:FInterval)->FInterval {
  if(!a.valid||!b.valid){return fi_invalid();}
  if(fi_zero(a)){return b;}if(fi_zero(b)){return a;}
  if(a.v.x==a.v.y&&b.v.x==b.v.y&&a.v.x==-b.v.x){return fi_exact(0.0);}
  return fi_round(a.v.x+b.v.x,a.v.y+b.v.y);
}
fn fi_sub(a:FInterval,b:FInterval)->FInterval {return fi_add(a,fi_neg(b));}
fn fi_mul(a:FInterval,b:FInterval)->FInterval {
  if(!a.valid||!b.valid){return fi_invalid();}
  if(fi_zero(a)||fi_zero(b)){return fi_exact(0.0);}
  if(all(a.v==vec2f(1.0))){return b;}if(all(b.v==vec2f(1.0))){return a;}
  if(all(a.v==vec2f(-1.0))){return fi_neg(b);}if(all(b.v==vec2f(-1.0))){return fi_neg(a);}
  let magnitude=max(abs(a.v.x),abs(a.v.y));
  if(magnitude>1.0&&max(abs(b.v.x),abs(b.v.y))>FI_LIMIT/magnitude*(1.0-4.0*FI_U)){return fi_invalid();}
  let p=vec4f(a.v.x*b.v.x,a.v.x*b.v.y,a.v.y*b.v.x,a.v.y*b.v.y);
  return fi_round(min(min(p.x,p.y),min(p.z,p.w)),max(max(p.x,p.y),max(p.z,p.w)));
}
fn fi_scale(a:FInterval,b:f32)->FInterval {return fi_mul(a,fi_exact(b));}
// Division and square root may use approximate GPU builtins. Enclose4 ULPs.
fn fi_builtin(lo:f32,hi:f32)->FInterval {
  return fi_round(lo-4.0*FI_U*abs(lo),hi+4.0*FI_U*abs(hi));
}
fn fi_div(a:FInterval,b:FInterval)->FInterval {
  if(!a.valid||!b.valid||fi_contains_zero(b)){return fi_invalid();}
  if(fi_zero(a)){return fi_exact(0.0);}
  if(all(b.v==vec2f(1.0))){return a;}if(all(b.v==vec2f(-1.0))){return fi_neg(a);}
  if(min(abs(b.v.x),abs(b.v.y))<1.0/FI_LIMIT){return fi_invalid();}
  let bits=bitcast<u32>(b.v.x);
  if(b.v.x==b.v.y&&(bits&0x007fffffu)==0u){
    let inverse=bitcast<f32>((bits&0x80000000u)|((254u-((bits>>23u)&255u))<<23u));
    return fi_mul(a,fi_exact(inverse));
  }
  return fi_mul(a,fi_builtin(1.0/b.v.y,1.0/b.v.x));
}
fn fi_square(a:FInterval)->FInterval {
  if(!a.valid){return fi_invalid();}if(fi_zero(a)){return a;}
  let maximum=max(abs(a.v.x),abs(a.v.y));
  let high=fi_mul(fi_exact(maximum),fi_exact(maximum));
  var low=fi_exact(0.0);
  if(!fi_contains_zero(a)){
    let minimum=min(abs(a.v.x),abs(a.v.y));low=fi_mul(fi_exact(minimum),fi_exact(minimum));
  }
  return FInterval(vec2f(max(0.0,low.v.x),high.v.y),low.valid&&high.valid);
}
fn fi_sqrt(a:FInterval)->FInterval {
  if(!a.valid||a.v.y<0.0){return fi_invalid();}if(fi_zero(a)){return a;}
  let result=fi_builtin(sqrt(max(0.0,a.v.x)),sqrt(a.v.y));
  return FInterval(vec2f(max(0.0,result.v.x),result.v.y),result.valid);
}
fn fi_distance(a:vec3f,b:vec3f)->FInterval {
  var squared=fi_exact(0.0);
  for(var axis=0u;axis<3u;axis++){
    squared=fi_add(squared,fi_square(fi_sub(fi_exact(a[axis]),fi_exact(b[axis]))));
  }
  return fi_sqrt(squared);
}
