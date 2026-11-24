// Immutable scalar planes; direct material hits and unchanged witness-gap sampling.
struct HeightData {info:vec4u,amplitudes:vec4f,words:array<u32>};
@group(0) @binding(9) var<storage,read> height_data:HeightData;
const LEAF_REPEATS=vec4f(1.0,10.0,100.0,1000.0);
fn hf_byte(index:u32)->f32 {
  return f32((height_data.words[index>>2u]>>((index&3u)*8u))&255u);
}
fn hf_read(order:u32,input:vec2u)->f32 {
  let edge=height_data.info.x;let last=vec2u(edge-1u);
  let ij=select(input&last,min(input,last),(order&3u)==0u);
  return hf_byte(order*height_data.info.y+ij.y*edge+ij.x);
}
fn hf_bilinear(order:u32,uv:vec2f,repeats:f32)->f32 {
  let at=uv*(f32(height_data.info.x)*repeats);
  let ij=vec2u(at);let f=fract(at);
  let lo=mix(hf_read(order,ij),hf_read(order,ij+vec2u(1u,0u)),f.x);
  let hi=mix(hf_read(order,ij+vec2u(0u,1u)),hf_read(order,ij+vec2u(1u)),f.x);
  return mix(lo,hi,f.y);
}
fn hf_height(uv:vec2f)->f32 {
  let h=vec4f(hf_bilinear(0u,uv,1.0),hf_bilinear(1u,uv,10.0),
    hf_bilinear(2u,uv,100.0),hf_bilinear(3u,uv,1000.0))-vec4f(128.0);
  return dot(h,height_data.amplitudes/255.0);
}
fn leaf_covered(uv:vec2f)->bool {
  return height_data.info.x<2u||hf_bilinear(4u,uv,1.0)>=127.5;
}
// Gap behavior intentionally unchanged. Direct hits use material_texh.
fn texh(o:vec3f,d:vec3f,base:Query,gap_lower:f32,cutout:bool)->Query {
  let reach=fi_up(dot(abs(height_data.amplitudes),vec4f(128.0/255.0)));
  if(height_data.info.x<2u||gap_lower>reach||(cutout&&!leaf_covered(base.uv))){return base;}
  let point=fma(vec3f(base.t),d,o);let side=dot(point-base.p,base.n);
  if(side!=0.0&&hf_height(leaf_uv_unit(base.uv))*sign(side)>=base.gap){
    return Query(true,base.t,0.0,point,base.n,base.uv);
  }
  return base;
}
