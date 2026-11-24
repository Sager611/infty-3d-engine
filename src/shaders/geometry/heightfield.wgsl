// Four signed height bands and opacity. Rust validates the upload.
struct HeightData {info:vec4u,amplitudes:vec4f,words:array<u32>};
@group(0) @binding(9) var<storage,read> height_data:HeightData;
const LEAF_REPEATS=vec4f(1.0,10.0,100.0,1000.0);
const CUP_SCALE=vec2f(4.375,0.0625);
const CUP_STEM=vec2f(0.09375,0.0);
const CUP_FLOOR=0.15;
fn hf_read(order:u32,input:vec2u)->f32 {
  let edge=height_data.info.x;let last=vec2u(edge-1u);
  let ij=select(input&last,min(input,last),(order&3u)==0u);
  let index=order*height_data.info.y+ij.y*edge+ij.x;
  return f32((height_data.words[index>>2u]>>((index&3u)*8u))&255u);
}
fn hf_bilinear(order:u32,uv:vec2f,repeats:f32)->f32 {
  let at=uv*(f32(height_data.info.x)*repeats);
  let ij=vec2u(at);let f=fract(at);
  let lo=mix(hf_read(order,ij),hf_read(order,ij+vec2u(1u,0u)),f.x);
  let hi=mix(hf_read(order,ij+vec2u(0u,1u)),hf_read(order,ij+vec2u(1u)),f.x);
  return mix(lo,hi,f.y);
}
fn hf_height(uv:vec2f)->f32 {
  let h=vec4f(hf_bilinear(0u,uv,LEAF_REPEATS.x),hf_bilinear(1u,uv,LEAF_REPEATS.y),
    hf_bilinear(2u,uv,LEAF_REPEATS.z),hf_bilinear(3u,uv,LEAF_REPEATS.w))-vec4f(128.0);
  return dot(h,height_data.amplitudes/255.0);
}
fn leaf_covered(uv:vec2f)->bool {
  return height_data.info.x<2u||hf_bilinear(4u,uv,1.0)>=127.5;
}
fn texh(o:vec3f,d:vec3f,base:Query,gap_lower:f32,cutout:bool)->Query {
  let reach=fi_up(dot(abs(height_data.amplitudes),vec4f(128.0/255.0)));
  if(height_data.info.x<2u||gap_lower>reach||(cutout&&!leaf_covered(base.uv))){return base;}
  let point=fma(vec3f(base.t),d,o);let side=dot(point-base.p,base.n);
  if(side!=0.0&&hf_height(leaf_uv_unit(base.uv))*sign(side)>=base.gap){
    return Query(true,base.t,0.0,point,base.n,base.uv);
  }
  return base;
}
fn leaf_domain(p:vec3f,t:f32)->bool {
  return fi_finite(t)&&t>=0.0&&all(abs(p.xy)<=vec2f(1.0));
}
fn leaf_root(o:vec3f,d:vec3f,offset:f32,root:f32)->Query {
  let p=fma(vec3f(root),d,o);let t=offset+root;
  var q=plane_sample(p,t,false);
  q.hit=leaf_domain(p,t)&&leaf_covered(q.uv);
  return q;
}
fn leaf_seed(o:vec3f,d:vec3f)->Query {
  let offset=max(0.0,-dot(o,d));let origin=fma(vec3f(offset),d,o);
  let delta=origin.xy+CUP_STEM;
  let a=dot(CUP_SCALE,d.xy*d.xy);
  let b=fma(2.0*CUP_SCALE.x*delta.x,d.x,fma(2.0*CUP_SCALE.y*delta.y,d.y,-d.z));
  let c=fma(CUP_SCALE.x,delta.x*delta.x,fma(CUP_SCALE.y,delta.y*delta.y,-CUP_FLOOR-origin.z));
  if(a==0.0){return leaf_root(origin,d,offset,-c/b);}
  let disc=fma(b,b,-4.0*a*c);
  if(disc<0.0){return plane_sample(vec3f(0.0),0.0,false);}
  let root=sqrt(disc);let q=-0.5*(b+select(-root,root,b>=0.0));
  if(q==0.0){return leaf_root(origin,d,offset,-b/(2.0*a));}
  let first=leaf_root(origin,d,offset,min(q/a,c/q));
  if(first.hit){return first;}
  return leaf_root(origin,d,offset,max(q/a,c/q));
}
fn leaf_project(d:vec3f,base:Query,axis:u32,cup:bool)->Query {
  let slope=select(vec2f(0.0),-2.0*CUP_SCALE*(base.p.xy+CUP_STEM),cup);
  let normal=vec3f(slope,1.0);
  let difference=hf_height(base.uv)-base.p.z;
  var step=0.0;
  if(difference!=0.0){
    step=difference/dot(d,normal);
    if(cup){step=clamp(step,-0.05,0.05);}
  }
  let t=base.t+step;let p=fma(vec3f(step),d,base.p);
  var hit=plane_sample(p,t,false);
  hit.hit=leaf_domain(p,t)&&leaf_covered(hit.uv);
  hit.p=plane_unframe(p,axis);hit.n=plane_unframe(normalize(normal),axis);
  return hit;
}
fn finite_leaf(o:vec3f,d:vec3f,axis:u32)->GeometryQuery {
  let local_d=plane_frame(d,axis);
  let cup=height_data.info.w!=0u;
  if(cup){
    let seed=leaf_seed(plane_frame(o,axis),local_d);
    if(seed.hit){
      let hit=leaf_project(local_d,seed,axis,true);
      if(hit.hit){return GeometryQuery(hit,0.0,plane_rounding(o,hit.t),0u);}
      var miss=finite_plane(o,d,axis);miss.query.hit=false;return miss;
    }
  }
  var base=finite_plane(o,d,axis);
  if(base.query.hit){
    if(!cup&&height_data.info.x>=2u){
      let projected=leaf_project(local_d,plane_sample(plane_frame(base.query.p,axis),base.query.t,true),axis,false);
      if(projected.hit){return GeometryQuery(projected,0.0,plane_rounding(o,projected.t),base.status);}
    }
    base.query.hit=false;
  }else if(base.status==0u){
    base.query=texh(o,d,base.query,base.gap_lower,true);
    if(base.query.hit){base.gap_lower=0.0;}
  }
  return base;
}
