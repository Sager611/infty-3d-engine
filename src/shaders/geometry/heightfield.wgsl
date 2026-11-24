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
struct LeafRoots {times:vec2f,count:u32};
fn leaf_roots(o:vec3f,d:vec3f)->LeafRoots {
  let delta=o.xy+CUP_STEM;
  let a=dot(CUP_SCALE,d.xy*d.xy);
  let b=fma(2.0*CUP_SCALE.x*delta.x,d.x,fma(2.0*CUP_SCALE.y*delta.y,d.y,-d.z));
  let c=fma(CUP_SCALE.x,delta.x*delta.x,fma(CUP_SCALE.y,delta.y*delta.y,-CUP_FLOOR-o.z));
  if(a==0.0){return LeafRoots(vec2f(-c/b),1u);}
  let disc=fma(b,b,-4.0*a*c);
  if(disc<0.0){return LeafRoots(vec2f(0.0),0u);}
  let root=sqrt(disc);let q=-0.5*(b+select(-root,root,b>=0.0));
  if(q==0.0){return LeafRoots(vec2f(-b/(2.0*a)),1u);}
  return LeafRoots(vec2f(min(q/a,c/q),max(q/a,c/q)),2u);
}
fn leaf_slope(p:vec2f)->vec2f {return -2.0*CUP_SCALE*(p+CUP_STEM);}
fn blade_normal(p:vec2f,uv:vec2f)->vec3f {
  // A wider derivative suppresses 8-bit height terraces without flattening veins.
  let step=3.0/f32(height_data.info.x);
  let dx=vec2f(step,0.0);let dy=dx.yx;
  let slope=vec2f(
    hf_bilinear(0u,max(uv-dx,vec2f(0.0)),1.0)-hf_bilinear(0u,min(uv+dx,vec2f(1.0)),1.0),
    hf_bilinear(0u,max(uv-dy,vec2f(0.0)),1.0)-hf_bilinear(0u,min(uv+dy,vec2f(1.0)),1.0)
  )*(height_data.amplitudes.x/(1020.0*step));
  return vec3f(mix(leaf_slope(p),slope,0.7),1.0);
}
fn height_step(delta:f32,slope:f32,limit:f32)->f32 {
  return clamp(delta*select(-1.0,1.0,slope>=0.0)/max(abs(slope),0x1p-20),-limit,limit);
}
fn leaf_project(d:vec3f,base:Query,axis:u32,cup:bool)->Query {
  let slope=select(vec2f(0.0),leaf_slope(base.p.xy),cup);
  var normal=vec3f(slope,1.0);
  let difference=hf_height(base.uv)-base.p.z;
  var step=0.0;
  if(cup){step=height_step(difference,dot(d,normal),0.05);}
  else if(difference!=0.0){step=difference/dot(d,normal);}
  let t=base.t+step;let p=fma(vec3f(step),d,base.p);
  var hit=plane_sample(p,t,false);
  hit.hit=leaf_domain(p,t)&&leaf_covered(hit.uv);
  if(hit.hit&&cup){normal=blade_normal(p.xy,hit.uv);}
  hit.p=plane_unframe(p,axis);hit.n=plane_unframe(normalize(normal),axis);
  return hit;
}
fn leaf_seed(o:vec3f,d:vec3f,offset:f32,root:f32)->Query {
  var seed=plane_sample(fma(vec3f(root),d,o),offset+root,false);
  seed.hit=leaf_domain(seed.p,seed.t)&&leaf_covered(seed.uv);
  return seed;
}
fn finite_leaf(o:vec3f,d:vec3f,axis:u32)->GeometryQuery {
  let local_d=plane_frame(d,axis);
  let cup=height_data.info.w!=0u;
  var seeded=false;
  if(cup){
    let local_o=plane_frame(o,axis);
    let offset=max(0.0,-dot(local_o,local_d));
    let origin=fma(vec3f(offset),local_d,local_o);
    let roots=leaf_roots(origin,local_d);
    // Explicit candidates: the divergent root loop corrupted the browser draw.
    if(roots.count!=0u){
      let near=leaf_seed(origin,local_d,offset,roots.times.x);
      if(near.hit){
        seeded=true;
        let hit=leaf_project(local_d,near,axis,true);
        if(hit.hit){return GeometryQuery(hit,0.0,plane_rounding(o,hit.t),0u);}
      }
      if(roots.count==2u){
        let far=leaf_seed(origin,local_d,offset,roots.times.y);
        if(far.hit){
          seeded=true;
          let hit=leaf_project(local_d,far,axis,true);
          if(hit.hit){return GeometryQuery(hit,0.0,plane_rounding(o,hit.t),0u);}
        }
      }
    }
  }
  var base=finite_plane(o,d,axis);
  if(seeded){base.query.hit=false;}
  else if(base.query.hit){
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
