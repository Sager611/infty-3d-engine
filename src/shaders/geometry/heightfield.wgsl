// Immutable five-field storage. Query adapters validate the header once.
struct HeightData {info:vec4u,amplitudes:vec4f,words:array<u32>};
@group(0) @binding(9) var<storage,read> height_data:HeightData;
struct HFAxis {cell:u32,part:FInterval};
struct HFValue {height:FInterval,scalar:f32,gradient:vec2f,mask:FInterval};
struct HFBounds {lo:vec3f,hi:vec3f,covered:bool,valid:bool};
fn hf_repeat(order:u32)->u32 {
  if(order==1u){return 10u;}if(order==2u){return 100u;}if(order==3u){return 1000u;}return 1u;
}
fn hf_header_ok()->bool {
  let edge=height_data.info.x;
  if(height_data.info.w!=1u||edge<2u||edge>512u||(edge&(edge-1u))!=0u){return false;}
  let count=edge*edge;let stride=(count+2u*(count-1u)/3u+3u)&~3u;
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
fn hf_read(order:u32,level:u32,input:vec2u)->vec2f {
  let edge=height_data.info.x;let side=edge>>level;
  var ij=input%vec2u(side);
  if(order==0u||order==4u){ij=min(input,vec2u(side-1u));}
  let pixel=ij.y*side+ij.x;let base=order*height_data.info.y;
  if(level==0u){return vec2f(hf_byte(base+pixel));}
  let previous=edge>>(level-1u);
  let offset=edge*edge+2u*(edge*edge-previous*previous)/3u+2u*pixel;
  return vec2f(hf_byte(base+offset),hf_byte(base+offset+1u));
}
fn hf_coeff(order:u32,ij:vec2u)->vec4f {
  let a=hf_read(order,0u,ij).x;let b=hf_read(order,0u,ij+vec2u(1u,0u)).x;
  let c=hf_read(order,0u,ij+vec2u(0u,1u)).x;let d=hf_read(order,0u,ij+vec2u(1u)).x;
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
// Dyadic domain coordinates stay exact integers even at depth19.
fn hf_address(index:u32,depth:u32,repeat:u32)->HFAxis {
  let edge_log=firstLeadingBit(height_data.info.x);let product=index*repeat;
  if(depth<=edge_log){return HFAxis(product<<(edge_log-depth),fi_exact(0.0));}
  let shift=depth-edge_log;let remainder=product&((1u<<shift)-1u);
  return HFAxis(product>>shift,fi_exact(f32(remainder)/f32(1u<<shift)));
}
fn hf_alpha_range(order:u32,node:vec3u)->FInterval {
  let repeat=hf_repeat(order);
  let ax=hf_address(node.x,node.z,repeat);let ay=hf_address(node.y,node.z,repeat);
  let bx=hf_address(node.x+1u,node.z,repeat);let by=hf_address(node.y+1u,node.z,repeat);
  let first=vec2u(ax.cell,ay.cell);let last=vec2u(bx.cell,by.cell);
  var result=fi_invalid();var found=false;
  if(all(last-first<=vec2u(1u))){
    for(var dy=0u;dy<2u;dy++){
      for(var dx=0u;dx<2u;dx++){
        let ij=first+vec2u(dx,dy);if(any(ij>last)){continue;}
        let c=hf_coeff(order,ij);
        let low=vec2f(select(0.0,ax.part.v.x,ij.x==first.x),select(0.0,ay.part.v.x,ij.y==first.y));
        let high=vec2f(select(1.0,bx.part.v.y,ij.x==last.x),select(1.0,by.part.v.y,ij.y==last.y));
        for(var corner=0u;corner<4u;corner++){
          let value=hf_polynomial(c,fi_exact(select(low.x,high.x,(corner&1u)!=0u)),
            fi_exact(select(low.y,high.y,(corner&2u)!=0u)));
          if(!found){result=value;found=true;}else{result.v=vec2f(min(result.v.x,value.v.x),max(result.v.y,value.v.y));result.valid=result.valid&&value.valid;}
        }
      }
    }
  }else{
    let span=max(last.x-first.x+1u,last.y-first.y+1u);
    var level=0u;if(span>1u){level=firstLeadingBit(span-1u)+1u;}
    level=min(level,firstLeadingBit(height_data.info.x));
    let start=first>>vec2u(level);let end=last>>vec2u(level);
    for(var dy=0u;dy<2u;dy++){
      for(var dx=0u;dx<2u;dx++){
        let ij=start+vec2u(dx,dy);if(any(ij>end)){continue;}
        let value=FInterval(hf_read(order,level,ij),true);
        if(!found){result=value;found=true;}else{result.v=vec2f(min(result.v.x,value.v.x),max(result.v.y,value.v.y));}
      }
    }
  }
  result.v=clamp(result.v,vec2f(0.0),vec2f(255.0));return result;
}
fn hf_domain(node:vec3u)->vec4f {
  let step=2.0/f32(1u<<node.z);
  return vec4f(vec2f(node.xy)*step-vec2f(1.0),vec2f(node.xy+vec2u(1u))*step-vec2f(1.0));
}
fn hf_bounds(node:vec3u)->HFBounds {
  var result=HFBounds(vec3f(0.0),vec3f(0.0),true,false);
  if(node.z>19u||any(node.xy>=vec2u(1u<<node.z))){return result;}
  let mask=hf_alpha_range(4u,node);
  if(!mask.valid){return result;}
  if(mask.v.y<127.5){result.covered=false;result.valid=true;return result;}
  var height=fi_exact(0.0);
  for(var order=0u;order<4u;order++){
    let range=fi_sub(hf_alpha_range(order,node),fi_exact(128.0));
    height=fi_add(height,fi_mul(range,fi_div(fi_exact(height_data.amplitudes[order]),fi_exact(255.0))));
  }
  let domain=hf_domain(node);
  result.lo=vec3f(domain.xy,height.v.x);result.hi=vec3f(domain.zw,height.v.y);
  result.valid=height.valid;return result;
}
