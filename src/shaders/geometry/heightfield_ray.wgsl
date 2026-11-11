// Fixed64-node hierarchy. Child culling precedes enqueue; no recursion or marching.
struct HFWork {node:vec3u,lower:f32};
fn hf_pop(queue:ptr<function,array<HFWork,64>>,count:ptr<function,u32>)->HFWork {
  var selected=0u;
  for(var index=1u;index<64u;index++){
    if(index>=*count){break;}
    let a=(*queue)[index];let b=(*queue)[selected];
    if(a.lower<b.lower||(a.lower==b.lower&&a.node.z>b.node.z)){selected=index;}
  }
  let result=(*queue)[selected];*count=*count-1u;(*queue)[selected]=(*queue)[*count];return result;
}
fn hf_perpendicular(o:vec3f,d:vec3f)->HFTraversal {
  var result=hf_empty();result.visits=1u;
  if(any(abs(o.xy)>vec2f(1.0))){return result;}
  let value=hf_sample(o.xy);
  if(!value.height.valid||!value.mask.valid){return hf_retain(result,0.0);}
  if(value.mask.v.y<127.5){return result;}
  let time=fi_div(fi_sub(value.height,fi_exact(o.z)),fi_exact(d.z));
  if(!time.valid){return hf_retain(result,0.0);}
  if(time.v.y<0.0){return result;}
  if(time.v.x<0.0||value.mask.v.x<127.5){return hf_retain(result,max(0.0,time.v.x));}
  result.contact=hf_contact(o,d,time);
  if(!result.contact.found){return hf_retain(result,max(0.0,time.v.x));}return result;
}
fn hf_finish(input:HFTraversal)->HFTraversal {
  var result=input;
  if(result.contact.found&&result.pending_valid&&result.pending>result.contact.time.v.y){result.pending_valid=false;}
  result.complete=!result.pending_valid;return result;
}
fn hf_query(o:vec3f,d:vec3f,budget:u32)->HFTraversal {
  var result=hf_empty();let limit=min(budget,64u);
  if(limit==0u||!plane_supported(o,d)||!hf_header_ok()){return hf_retain(result,0.0);}
  if(all(d.xy==vec2f(0.0))){return hf_perpendicular(o,d);}
  let resolution=height_data.info.z;let depth=min(firstLeadingBit(resolution-1u)+1u,19u);
  var queue:array<HFWork,64>;queue[0]=HFWork(vec3u(0u),0.0);var count=1u;
  for(var iteration=0u;iteration<64u;iteration++){
    if(iteration>=limit||count==0u){break;}
    let item=hf_pop(&queue,&count);
    if(result.contact.found&&item.lower>result.contact.time.v.y){continue;}
    result.visits++;
    let bounds=hf_bounds(item.node);
    if(!bounds.valid){result=hf_retain(result,item.lower);continue;}
    if(!bounds.covered){continue;}
    let span=plane_box(o,d,bounds.lo,bounds.hi);
    if(!span.valid){continue;}
    if(result.contact.found&&span.near>result.contact.time.v.y){continue;}
    if(item.node.z<depth){
      for(var child=0u;child<4u;child++){
        let node=vec3u(item.node.xy*2u+vec2u(child&1u,child>>1u),item.node.z+1u);
        let domain=hf_domain(node);
        let xy=plane_axis(plane_axis(span,o.x,d.x,domain.x,domain.z),o.y,d.y,domain.y,domain.w);
        if(!xy.valid){continue;}
        if(count==64u){result=hf_retain(result,xy.near);continue;}
        queue[count]=HFWork(node,xy.near);count++;
      }
    }else{
      // Cancel the dyadic edge before multiplying: every numerator fits29 bits.
      let shift=item.node.z-firstLeadingBit(height_data.info.x);let factor=resolution/height_data.info.x;
      let first=(item.node.xy*factor)>>vec2u(shift);
      let last=((item.node.xy+vec2u(1u))*factor-vec2u(1u))>>vec2u(shift);
      for(var dy=0u;dy<2u;dy++){
        for(var dx=0u;dx<2u;dx++){
          let ij=first+vec2u(dx,dy);
          if(any(ij>last)||any(ij>=vec2u(resolution))){continue;}
          result=hf_cell(result,hf_piece(ij),o,d,span);
        }
      }
    }
  }
  for(var index=0u;index<64u;index++){
    if(index>=count){break;}result=hf_retain(result,queue[index].lower);
  }
  return hf_finish(result);
}
