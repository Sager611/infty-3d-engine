// Filtered shading from the same immutable height bands.
@group(0) @binding(10) var leaf_repeat:sampler;
@group(0) @binding(11) var leaf_clamp:sampler;
fn leaf_uv(p:vec2f,order:u32)->vec2f {
  let x=hf_axis(order,p.x);let y=hf_axis(order,p.y);let edge=height_data.info.x;
  var cell=vec2u(x.cell,y.cell);
  if(order!=0u){cell%=vec2u(edge);}
  var uv=(vec2f(cell)+vec2f(fi_middle(x.part),fi_middle(y.part))+vec2f(0.5))/f32(edge);
  if(order==0u){uv=clamp(uv,vec2f(0.5/f32(edge)),vec2f(1.0-0.5/f32(edge)));}
  return uv;
}
fn leaf_fetch(uv:vec2f,order:u32,layer:i32,lod:f32)->vec4f {
  if(order==0u){return textureSampleLevel(material_tiles,leaf_clamp,uv,layer,lod);}
  return textureSampleLevel(material_tiles,leaf_repeat,uv,layer,lod);
}
fn leaf_tile(p:vec2f,order:u32,layer:i32,lod:f32)->MaterialTile {
  let uv=leaf_uv(p,order);let center=leaf_fetch(uv,order,layer,lod);
  let x=hf_axis(order,p.x);let y=hf_axis(order,p.y);
  let raw=hf_gradient(order,hf_coeff(order,vec2u(x.cell,y.cell)),vec2f(fi_middle(x.part),fi_middle(y.part)));
  if(lod==0.0){return MaterialTile(center.rgb,raw,center.a);}
  let step=0.5*exp2(lod)/512.0;
  let right=leaf_fetch(uv+vec2f(step,0.0),order,layer,lod).a;
  let left=leaf_fetch(uv-vec2f(step,0.0),order,layer,lod).a;
  let up=leaf_fetch(uv+vec2f(0.0,step),order,layer,lod).a;
  let down=leaf_fetch(uv-vec2f(0.0,step),order,layer,lod).a;
  let filtered=vec2f(right-left,up-down)/(2.0*step)*height_data.amplitudes[order]*0.5*f32(hf_repeat(order));
  return MaterialTile(center.rgb,mix(raw,filtered,smoothstep(0.0,1.0,lod)),center.a);
}
fn leaf_material(local:vec3f,axis:u32,ray:vec3f,size:f32,footprint:f32)->MaterialSurface {
  let p=plane_frame(local,axis).xy;
  var base=vec3f(0.1,0.2,0.04);var modulation=vec3f(1.0);var gradient=vec2f(0.0);
  var variance=0.45*0.45;
  for(var i=0u;i<min(64u,arrayLength(&material_levels));i++){
    let level=material_levels[i];if(level.identity.x!=2u){continue;}
    let order=level.identity.z>>8u;if(order>=4u){continue;}
    let extent=2.0*size/f32(hf_repeat(order));
    let lod=clamp(log2(max(1e-30,footprint)*512.0/extent),0.0,9.0);
    let visible=1.0-smoothstep(4.0,8.0,lod);variance+=level.surface.w*(1.0-visible);
    if(visible==0.0){if(order==0u){base=level.mean.rgb;}continue;}
    let tile=leaf_tile(p,order,i32(level.identity.y),lod);
    if(order==0u){base=tile.color;}
    else{modulation*=mix(vec3f(1.0),tile.color/max(level.mean.rgb,vec3f(1.0/255.0)),level.surface.z*visible);}
    gradient+=tile.gradient*visible;
  }
  let n=plane_unframe(material_unit(vec3f(-gradient,1.0),vec3f(0.0,0.0,1.0)),axis);
  return MaterialSurface(clamp(base*modulation,vec3f(0.004),vec3f(0.98)),
    surface_normal(n,ray,5u),clamp(sqrt(variance),0.05,0.98),0.0,1.0,0.0);
}
