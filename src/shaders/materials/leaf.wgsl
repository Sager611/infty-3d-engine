// Color-only Leaf shading; shared texh supplies the displaced normal.
@group(0) @binding(10) var leaf_repeat:sampler;
@group(0) @binding(11) var leaf_clamp:sampler;
fn leaf_uv_unit(uv:vec2f)->vec2f {
  return select(uv,fract(uv),uv!=clamp(uv,vec2f(0.0),vec2f(1.0)));
}
fn leaf_uv(uv:vec2f,order:u32)->vec2f {
  if(order<2u){return fma(uv,vec2f(LEAF_REPEATS[order]),vec2f(0.5/512.0));}
  return hf_phase(uv,order,LEAF_REPEATS[order])+vec2f(0.5/512.0);
}
fn leaf_fetch(uv:vec2f,order:u32,layer:i32,lod:f32)->vec4f {
  if(order==0u){return textureSampleLevel(material_tiles,leaf_clamp,uv,layer,lod);}
  return textureSampleLevel(material_tiles,leaf_repeat,uv,layer,lod);
}
fn leaf_level(input:vec3f,index:u32,material:u32,p:vec2f,size:f32,footprint:f32)->vec3f {
  if(index>=arrayLength(&material_levels)){return input;}
  let level=material_levels[index];if(level.identity.x!=material){return input;}
  let order=(level.identity.z>>8u)&255u;if(order>=4u){return input;}
  let lod=material_lod(footprint,2.0*size/LEAF_REPEATS[order]);let visible=material_visible(lod);
  if(visible==0.0){return select(input,level.mean.rgb,order==0u);}
  let color=leaf_fetch(leaf_uv(p,order),order,i32(level.identity.y),lod).rgb;
  if(order==0u){return color;}
  return input*mix(vec3f(1.0),color/max(level.mean.rgb,vec3f(1.0/255.0)),level.surface.z*visible);
}
fn leaf_material(material:u32,uv:vec2f,normal:vec3f,size:f32,footprint:f32)->MaterialSurface {
  let p=leaf_uv_unit(uv);var color=vec3f(0.5);let spec=material_spec(material);
  // @leaf-levels
  return MaterialSurface(clamp(color,vec3f(0.004),vec3f(0.98)),normal,spec.roughness,spec.metallic,1.0,0.0);
}
