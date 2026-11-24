// Color-only Leaf shading with the carrier normal.
@group(0) @binding(10) var leaf_repeat:sampler;
@group(0) @binding(11) var leaf_clamp:sampler;
fn leaf_uv_unit(uv:vec2f)->vec2f {
  return select(uv,fract(uv),uv!=clamp(uv,vec2f(0.0),vec2f(1.0)));
}
fn leaf_uv(uv:vec2f,order:u32)->vec2f {
  return fma(uv,vec2f(LEAF_REPEATS[order]),vec2f(0.5/512.0));
}
fn leaf_fetch(uv:vec2f,order:u32,layer:i32,lod:f32)->vec4f {
  if(order==0u){return textureSampleLevel(material_tiles,leaf_clamp,uv,layer,lod);}
  return textureSampleLevel(material_tiles,leaf_repeat,uv,layer,lod);
}
fn leaf_material(uv:vec2f,normal:vec3f,size:f32,footprint:f32)->MaterialSurface {
  let p=leaf_uv_unit(uv);
  var base=vec3f(0.1,0.2,0.04);var modulation=vec3f(1.0);
  for(var i=0u;i<min(64u,arrayLength(&material_levels));i++){
    let level=material_levels[i];if(level.identity.x!=2u){continue;}
    let order=level.identity.z>>8u;if(order>=4u){continue;}
    let extent=2.0*size/LEAF_REPEATS[order];
    let lod=clamp(log2(max(1e-30,footprint)*512.0/extent),0.0,9.0);
    let visible=1.0-smoothstep(4.0,8.0,lod);
    if(visible==0.0){if(order==0u){base=level.mean.rgb;}continue;}
    let color=leaf_fetch(leaf_uv(p,order),order,i32(level.identity.y),lod).rgb;
    if(order==0u){base=color;}
    else{modulation*=mix(vec3f(1.0),color/max(level.mean.rgb,vec3f(1.0/255.0)),level.surface.z*visible);}
  }
  return MaterialSurface(clamp(base*modulation,vec3f(0.004),vec3f(0.98)),normal,0.45,0.0,1.0,0.0);
}
