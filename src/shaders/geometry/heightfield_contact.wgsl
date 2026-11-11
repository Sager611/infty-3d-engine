// Analytic candidates are contacts only after continuous-field existence/support proof.
struct HFContact {found:bool,time:FInterval,p:vec3f,n:vec3f,error:f32};
struct HFTraversal {contact:HFContact,visits:u32,pending:f32,pending_valid:bool,complete:bool};
fn hf_empty_contact()->HFContact {
  return HFContact(false,fi_invalid(),vec3f(0.0),vec3f(0.0,0.0,1.0),0.0);
}
fn hf_empty()->HFTraversal {return HFTraversal(hf_empty_contact(),0u,0.0,false,true);}
fn hf_retain(input:HFTraversal,lower:f32)->HFTraversal {
  var result=input;
  if(!result.pending_valid){result.pending=lower;}else{result.pending=min(result.pending,lower);}
  result.pending_valid=true;result.complete=false;return result;
}
fn hf_choose(input:HFTraversal,candidate:HFContact)->HFTraversal {
  var result=input;if(!candidate.found){return result;}
  if(!result.contact.found){result.contact=candidate;return result;}
  let time=FInterval(vec2f(min(candidate.time.v.x,result.contact.time.v.x),
    min(candidate.time.v.y,result.contact.time.v.y)),true);
  if(fi_middle(candidate.time)<fi_middle(result.contact.time)){result.contact=candidate;}
  // Preserve a representative on the same surface; bound its shifted ray parameter.
  let shift=abs(fi_middle(result.contact.time)-fi_middle(time));
  result.contact.error=fi_add(fi_exact(result.contact.error),fi_exact(shift*1.00001)).v.y;
  result.contact.time=time;return result;
}
fn hf_ray_coordinate(o:vec3f,d:vec3f,time:FInterval,axis:u32)->FInterval {
  return fi_add(fi_exact(o[axis]),fi_scale(time,d[axis]));
}
fn hf_spread(o:vec3f,d:vec3f,time:FInterval,point:vec2f)->FInterval {
  var result=fi_exact(0.0);
  for(var axis=0u;axis<2u;axis++){
    let delta=fi_sub(hf_ray_coordinate(o,d,time,axis),fi_exact(point[axis]));
    result=fi_add(result,FInterval(vec2f(0.0,max(abs(delta.v.x),abs(delta.v.y))),delta.valid));
  }
  return result;
}
fn hf_mask_range(o:vec3f,d:vec3f,time:FInterval)->FInterval {
  let point=clamp((o+fi_middle(time)*d).xy,vec2f(-1.0),vec2f(1.0));
  let value=hf_sample(point);let spread=hf_spread(o,d,time,point);
  let error=fi_scale(spread,127.5*f32(height_data.info.x));
  return fi_add(value.mask,FInterval(vec2f(-error.v.y,error.v.y),error.valid));
}
fn hf_slope()->FInterval {
  var value=fi_exact(0.0);
  for(var order=0u;order<4u;order++){
    value=fi_add(value,fi_scale(fi_exact(abs(height_data.amplitudes[order])),
      0.5*f32(height_data.info.x*hf_repeat(order))));
  }
  return value;
}
fn hf_actual_sign(o:vec3f,d:vec3f,time:f32)->FInterval {
  let at=fi_exact(time);
  for(var axis=0u;axis<2u;axis++){
    let coordinate=hf_ray_coordinate(o,d,at,axis);
    if(!coordinate.valid||coordinate.v.x< -1.0||coordinate.v.y>1.0){return fi_invalid();}
  }
  let point=clamp((o+time*d).xy,vec2f(-1.0),vec2f(1.0));
  let value=hf_sample(point);let residual=fi_sub(hf_ray_coordinate(o,d,at,2u),value.height);
  let error=fi_mul(hf_spread(o,d,at,point),hf_slope());
  return fi_add(residual,FInterval(vec2f(-error.v.y,error.v.y),error.valid));
}
fn hf_contact(o:vec3f,d:vec3f,time:FInterval)->HFContact {
  var contact=hf_empty_contact();let nominal=o+fi_middle(time)*d;
  let xy=clamp(nominal.xy,vec2f(-1.0),vec2f(1.0));let value=hf_sample(xy);
  if(!time.valid||!value.height.valid||!value.mask.valid){return contact;}
  let point=vec3f(xy,value.scalar);var squared=fi_exact(0.0);
  for(var axis=0u;axis<3u;axis++){
    squared=fi_add(squared,fi_square(fi_sub(hf_ray_coordinate(o,d,time,axis),fi_exact(point[axis]))));
  }
  let distance=fi_sqrt(squared);
  if(!distance.valid){return contact;}
  return HFContact(true,time,point,normalize(vec3f(-value.gradient,1.0)),distance.v.y);
}
fn hf_inside(piece:HFPiece,o:vec3f,d:vec3f,time:FInterval)->bool {
  let x=hf_coordinate(piece,0u,o,d,time);let y=hf_coordinate(piece,1u,o,d,time);
  return x.valid&&y.valid&&x.v.x>=0.0&&x.v.y<=1.0&&y.v.x>=0.0&&y.v.y<=1.0&&time.v.x>=0.0;
}
fn hf_entry(input:HFTraversal,piece:HFPiece,o:vec3f,d:vec3f,span:PlaneRange,mask_coincident:bool)->HFTraversal {
  let entry=max(span.near,span.near_upper);
  if(entry>span.far_lower){
    let coverage=hf_mask_range(o,d,FInterval(vec2f(span.near,span.far),true));
    if(!coverage.valid||coverage.v.y>=127.5){return hf_retain(input,span.near);}return input;
  }
  let coverage=hf_mask_range(o,d,fi_exact(entry));
  if(!mask_coincident&&coverage.valid&&coverage.v.y<127.5){return input;}
  if((mask_coincident||(coverage.valid&&coverage.v.x>=127.5))&&hf_inside(piece,o,d,fi_exact(entry))){
    let candidate=hf_contact(o,d,FInterval(vec2f(span.near,entry),true));
    if(candidate.found){return hf_choose(input,candidate);}
  }
  return hf_retain(input,span.near);
}
fn hf_certified(piece:HFPiece,o:vec3f,d:vec3f,time:FInterval,exists:bool,mask_root:bool)->FInterval {
  if(!time.valid||time.v.y<0.0){return fi_invalid();}
  let inside=hf_inside(piece,o,d,time);
  if(mask_root){if(inside&&exists){return time;}return fi_invalid();}
  let mask=hf_mask_range(o,d,time);
  if(inside&&exists&&mask.valid&&mask.v.x>=127.5){return time;}
  // Fixed two-endpoint certificate around one analytic proposal, never stepping/refining.
  let middle=fi_middle(time);let residual=hf_actual_sign(o,d,middle);
  if(!residual.valid){return fi_invalid();}
  let pad=4.0*max(abs(residual.v.x),abs(residual.v.y))/max(abs(d.z),0.125)+4.0*FI_U*abs(middle);
  var bracket=fi_round(max(0.0,time.v.x-pad),time.v.y+pad);bracket.v.x=max(0.0,bracket.v.x);
  if(!bracket.valid){return fi_invalid();}
  let coverage=hf_mask_range(o,d,bracket);
  if(!coverage.valid||coverage.v.x<127.5){return fi_invalid();}
  let a=hf_actual_sign(o,d,bracket.v.x);let b=hf_actual_sign(o,d,bracket.v.y);
  if(a.valid&&b.valid&&((a.v.y<0.0&&b.v.x>0.0)||(a.v.x>0.0&&b.v.y<0.0))){return bracket;}
  return fi_invalid();
}
fn hf_cell(input:HFTraversal,piece:HFPiece,o:vec3f,d:vec3f,parent:PlaneRange)->HFTraversal {
  var result=input;var span=parent;let resolution=fi_exact(f32(height_data.info.z));
  for(var axis=0u;axis<2u;axis++){
    let lo=fi_sub(fi_div(fi_exact(2.0*f32(piece.ij[axis])),resolution),fi_exact(1.0));
    let hi=fi_sub(fi_div(fi_exact(2.0*f32(piece.ij[axis]+1u)),resolution),fi_exact(1.0));
    if(!lo.valid||!hi.valid){return hf_retain(result,parent.near);}
    span=plane_axis(span,o[axis],d[axis],lo.v.x,hi.v.y);
  }
  if(!span.valid){return result;}
  if(span.unbounded||span.uncertain){return hf_retain(result,span.near);}
  let start=max(0.0,fi_down(span.near));let length=fi_up(fi_up(span.far)-start);
  if(!fi_finite(length)||length<=0.0){return hf_retain(result,span.near);}
  let x=hf_coordinate(piece,0u,o,d,fi_exact(start));let y=hf_coordinate(piece,1u,o,d,fi_exact(start));
  let scale=fi_scale(fi_exact(length),0.5*f32(height_data.info.z));
  let dx=fi_scale(scale,d.x);let dy=fi_scale(scale,d.y);
  var geometry=hf_compose(piece.height,x,dx,y,dy);
  geometry[0]=fi_neg(geometry[0]);
  geometry[1]=fi_sub(fi_scale(fi_exact(length),d.z),geometry[1]);
  geometry[2]=fi_sub(hf_ray_coordinate(o,d,fi_exact(start),2u),geometry[2]);
  var roots=hf_roots(geometry);let coincident=roots.coincident;
  if(roots.unknown){result=hf_retain(result,span.near);}
  if(coincident){
    var mask=hf_compose(piece.mask,x,dx,y,dy);mask[2]=fi_sub(mask[2],fi_exact(127.5));
    roots=hf_roots(mask);if(roots.unknown){result=hf_retain(result,span.near);}
    result=hf_entry(result,piece,o,d,span,roots.coincident);
  }
  for(var index=0u;index<2u;index++){
    let root=roots.roots[index];if(!root.present){continue;}
    let time=fi_add(fi_scale(root.time,length),fi_exact(start));
    if(time.valid&&time.v.y<0.0){continue;}
    let coverage=hf_mask_range(o,d,time);
    if(!coincident&&coverage.valid&&coverage.v.y<127.5){continue;}
    let certified=hf_certified(piece,o,d,time,root.exists,coincident);
    if(!certified.valid){result=hf_retain(result,span.near);continue;}
    let candidate=hf_contact(o,d,certified);
    if(candidate.found){result=hf_choose(result,candidate);}else{result=hf_retain(result,span.near);}
  }
  return result;
}
