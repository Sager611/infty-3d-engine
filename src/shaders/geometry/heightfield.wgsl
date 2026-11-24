// Immutable five-field storage. Query adapters validate the header once.
struct HeightData {info:vec4u,amplitudes:vec4f,words:array<u32>};
@group(0) @binding(9) var<storage,read> height_data:HeightData;
struct HFAxis {cell:u32,part:FInterval};
struct HFValue {height:FInterval,scalar:f32,gradient:vec2f,mask:FInterval};
fn hf_repeat(order:u32)->u32 {
  if(order==1u){return 10u;}if(order==2u){return 100u;}if(order==3u){return 1000u;}return 1u;
}
fn hf_header_ok()->bool {
  let edge=height_data.info.x;
  if(height_data.info.w!=1u||edge<2u||edge>512u||(edge&(edge-1u))!=0u){return false;}
  let stride=edge*edge;
  if(height_data.info.y!=stride||arrayLength(&height_data.words)*4u!=5u*stride){return false;}
  var resolution=edge;
  for(var order=0u;order<4u;order++){
    let amplitude=height_data.amplitudes[order];
    if(!fi_finite(amplitude)||fi_subnormal(amplitude)||abs(amplitude)>1.0){return false;}
    if(amplitude!=0.0){resolution=edge*hf_repeat(order);}
  }
  return height_data.info.z==resolution;
}
fn hf_byte(index:u32)->f32 {
  return f32((height_data.words[index>>2u]>>((index&3u)*8u))&255u);
}
fn hf_read(order:u32,input:vec2u)->f32 {
  let edge=height_data.info.x;
  let ij=select(input%vec2u(edge),min(input,vec2u(edge-1u)),order==0u||order==4u);
  return hf_byte(order*height_data.info.y+ij.y*edge+ij.x);
}
fn hf_coeff(order:u32,ij:vec2u)->vec4f {
  let a=hf_read(order,ij);let b=hf_read(order,ij+vec2u(1u,0u));
  let c=hf_read(order,ij+vec2u(0u,1u));let d=hf_read(order,ij+vec2u(1u));
  return vec4f(a,b-a,c-a,d-b-c+a);
}
fn hf_unit(value:FInterval)->FInterval {
  return FInterval(clamp(value.v,vec2f(0.0),vec2f(1.0)),value.valid);
}
// Exact IEEE addressing: mantissa times the odd repeat factor fits31 bits.
// Avoid a rounded 512000-wide float phase; no fused-multiply assumption.
fn hf_axis(order:u32,p:f32)->HFAxis {
  let rank=select(0u,order,order<4u);
  let odd=array<u32,4>(1u,5u,25u,125u)[rank];
  let power=firstLeadingBit(height_data.info.x)-1u+rank;
  let scale=odd<<power;let bits=bitcast<u32>(p);let magnitude=bits&0x7fffffffu;
  if(magnitude==0u){return HFAxis(scale,fi_exact(0.0));}
  let exponent=i32((magnitude>>23u)&255u)-127;
  let shift=u32(23-exponent-i32(power));
  let product=((magnitude&0x007fffffu)|0x00800000u)*odd;
  var whole=0u;var remainder=product;
  if(shift<32u){whole=product>>shift;remainder=product&((1u<<shift)-1u);}
  var fraction=fi_exact(0.0);
  if(remainder!=0u){
    let value=ldexp(f32(remainder),-i32(shift));fraction=fi_round(value,value);
  }
  if((bits&0x80000000u)!=0u){
    if(remainder==0u){return HFAxis(scale-whole,fraction);}
    return HFAxis(scale-whole-1u,hf_unit(fi_sub(fi_exact(1.0),fraction)));
  }
  return HFAxis(scale+whole,hf_unit(fraction));
}
fn hf_polynomial(c:vec4f,x:FInterval,y:FInterval)->FInterval {
  return fi_add(fi_add(fi_exact(c.x),fi_scale(x,c.y)),
    fi_add(fi_scale(y,c.z),fi_scale(fi_mul(x,y),c.w)));
}
fn hf_gradient(order:u32,c:vec4f,phase:vec2f)->vec2f {
  let amplitude=height_data.amplitudes[order]/255.0;
  let scale=0.5*f32(height_data.info.x*hf_repeat(order));
  return amplitude*scale*vec2f(c.y+c.w*phase.y,c.z+c.w*phase.x);
}
fn hf_sample(p:vec2f)->HFValue {
  var result=HFValue(fi_exact(0.0),0.0,vec2f(0.0),fi_invalid());
  if(any(abs(p)>vec2f(1.0))||!fi_finite(p.x)||!fi_finite(p.y)||
      fi_subnormal(p.x)||fi_subnormal(p.y)){result.height=fi_invalid();return result;}
  for(var order=0u;order<4u;order++){
    let x=hf_axis(order,p.x);let y=hf_axis(order,p.y);
    var c=hf_coeff(order,vec2u(x.cell,y.cell));c.x-=128.0;
    let gain=fi_div(fi_exact(height_data.amplitudes[order]),fi_exact(255.0));
    result.height=fi_add(result.height,fi_mul(hf_polynomial(c,x.part,y.part),gain));
    let fx=fi_middle(x.part);let fy=fi_middle(y.part);
    let amplitude=height_data.amplitudes[order]/255.0;
    result.scalar+=amplitude*(c.x+c.y*fx+c.z*fy+c.w*fx*fy);
    result.gradient+=hf_gradient(order,c,vec2f(fx,fy));
  }
  let x=hf_axis(4u,p.x);let y=hf_axis(4u,p.y);
  result.mask=hf_polynomial(hf_coeff(4u,vec2u(x.cell,y.cell)),x.part,y.part);
  return result;
}
fn hf_at(order:u32,p:vec2f)->f32 {
  let x=hf_axis(order,p.x);let y=hf_axis(order,p.y);
  let c=hf_coeff(order,vec2u(x.cell,y.cell));
  let f=vec2f(fi_middle(x.part),fi_middle(y.part));
  return c.x+c.y*f.x+c.z*f.y+c.w*f.x*f.y;
}
fn texh(o:vec3f,d:vec3f,base:Query,gap_lower:f32,resolved:bool,cutout:bool)->Query {
  if(!hf_header_ok()||(base.hit&&!cutout)){return base;}
  let reach=fi_up(dot(abs(height_data.amplitudes),vec4f(128.0/255.0)));
  if(!base.hit&&(!resolved||gap_lower>reach)){return base;}
  let uv=select(base.uv,fract(base.uv),base.uv!=clamp(base.uv,vec2f(0.0),vec2f(1.0)));
  let p=fma(uv,vec2f(2.0),vec2f(-1.0));
  var q=base;
  if(cutout&&hf_at(4u,p)<127.5){q.hit=false;return q;}
  if(!q.hit){
    let point=fma(vec3f(q.t),d,o);let side=dot(point-q.p,q.n);
    if(side==0.0){return q;}
    var height=0.0;
    for(var order=0u;order<4u;order++){
      let a=height_data.amplitudes[order];
      if(a!=0.0){height+=(hf_at(order,p)-128.0)*(a/255.0);}
    }
    if(height*sign(side)>=q.gap){q.hit=true;q.p=point;q.gap=0.0;}
  }
  return q;
}
