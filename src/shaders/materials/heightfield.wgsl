// Immutable scalar planes; direct material hits and unchanged witness-gap sampling.
struct HeightData {info:vec4u,amplitudes:vec4f,words:array<u32>};
@group(0) @binding(9) var<storage,read> height_data:HeightData;
const LEAF_REPEATS=vec4f(1.0,10.0,100.0,1000.0);
// C1 phase variation, shared by fine height/color. Coarse shape and coverage stay unchanged.
fn hf_phase_hash(ij:vec2u)->vec2f {
  let word=(ij.x*0x9e3779b9u)^(ij.y*0x85ebca6bu);
  var value=vec2u(word,word^0x68bc21ebu);
  value=(value^(value>>vec2u(16u)))*vec2u(0x7feb352du);
  value=(value^(value>>vec2u(15u)))*vec2u(0x846ca68bu);
  return vec2f(value^(value>>vec2u(16u)))*(1.0/4294967296.0);
}
fn hf_phase(uv:vec2f,order:u32,repeats:f32)->vec2f {
  let q=uv*repeats;if(order<2u||order>=4u){return q;}
  let seed=select(vec2f(47.0,113.0),vec2f(13.0,71.0),order==2u);
  let p=fma(q,vec2f(0.25),seed);let period=vec2u(u32(repeats*0.25));
  let ij=vec2u(floor(p))%period;let next=(ij+vec2u(1u))%period;let f=fract(p);
  let w=f*f*(vec2f(3.0)-2.0*f);
  let lo=mix(hf_phase_hash(ij),hf_phase_hash(vec2u(next.x,ij.y)),w.x);
  let hi=mix(hf_phase_hash(vec2u(ij.x,next.y)),hf_phase_hash(next),w.x);
  return fma(mix(lo,hi,w.y),vec2f(1.25),q);
}
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
    hf_gradient_value(2u,uv,100.0),hf_gradient_value(3u,uv,1000.0))-vec4f(128.0);
  return dot(h,height_data.amplitudes/255.0);
}
// Byte heights lie in [-128,127]/255. Ignore sub-quarter-pixel relief;
// fade to full strength over the next 2x approach. Raw queries stay exact.
fn hf_detail_weight(order:u32,span:f32)->f32 {
  let bound=abs(height_data.amplitudes[order])*(128.0/255.0);
  if(bound==0.0){return 0.0;}
  if(order==0u||span<=0.0){return 1.0;}
  return smoothstep(0.25,0.5,bound/span);
}
fn hf_render_level(order:u32,uv:vec2f,span:f32)->f32 {
  let weight=hf_detail_weight(order,span);
  if(weight==0.0){return 0.0;}
  return (hf_gradient_value(order,uv,LEAF_REPEATS[order])-128.0)*(height_data.amplitudes[order]/255.0)*weight;
}
fn hf_render_details(uv:vec2f,span:f32)->f32 {
  return hf_render_level(1u,uv,span)+hf_render_level(2u,uv,span)+hf_render_level(3u,uv,span);
}
fn hf_render_height(uv:vec2f,span:f32)->f32 {return hf_render_level(0u,uv,span)+hf_render_details(uv,span);}
// Bilinearly blend vertex slopes; subtexel differences expose faceted cell gradients.
fn hf_gradient_value(order:u32,uv:vec2f,repeats:f32)->f32 {
  let p=hf_phase(uv,order,repeats);
  return hf_bilinear(order,select(fract(p),clamp(p,vec2f(0.0),vec2f(1.0)),order==0u),1.0);
}
fn hf_slope(order:u32,uv:vec2f,repeats:f32,footprint:f32,span:f32,quantum_axes:f32)->vec2f {
  let detail=hf_detail_weight(order,span);if(detail==0.0){return vec2f(0.0);}
  let edge=f32(height_data.info.x);
  let visible=detail*material_visible(clamp(log2(max(1e-30,footprint)*edge*repeats),0.0,9.0));
  if(visible==0.0){return vec2f(0.0);}
  let step=surface_slope_step(max(1.0/(edge*repeats),0.5*footprint),height_data.amplitudes[order]*quantum_axes/255.0);
  let x=hf_gradient_value(order,uv+vec2f(step,0.0),repeats)-hf_gradient_value(order,uv-vec2f(step,0.0),repeats);
  let y=hf_gradient_value(order,uv+vec2f(0.0,step),repeats)-hf_gradient_value(order,uv-vec2f(0.0,step),repeats);
  return vec2f(x,y)*(visible/(2.0*step));
}
fn hf_gradient_lod(uv:vec2f,footprint:f32,span:f32,quantum_axes:f32)->vec2f {
  if(height_data.info.x<2u){return vec2f(0.0);}
  return (hf_slope(0u,uv,1.0,footprint,span,quantum_axes)*height_data.amplitudes.x
    +hf_slope(1u,uv,10.0,footprint,span,quantum_axes)*height_data.amplitudes.y
    +hf_slope(2u,uv,100.0,footprint,span,quantum_axes)*height_data.amplitudes.z
    +hf_slope(3u,uv,1000.0,footprint,span,quantum_axes)*height_data.amplitudes.w)/255.0;
}
fn hf_gradient(uv:vec2f,footprint:f32)->vec2f {return hf_gradient_lod(uv,footprint,0.0,0.0);}
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
