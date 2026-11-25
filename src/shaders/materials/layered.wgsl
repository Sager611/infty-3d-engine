// RGBA8 linear reflectance + relief. Explicit mip footprints for compute shading.
struct MaterialLevel {
  identity: vec4u, // material, layer, base bit + leaf order<<8, optional atlas columns/rows
  surface: vec4f, // metres/tile, relief metres, colour gain, unresolved roughness
  phase: vec4f,   // f64-origin fractions + exact packed 8-bit XYZ tile identities
  mean: vec4f,    // linear RGB/height mean; fine layers modulate around one
  border: vec4f, // atlas mortar RGB/height; absent for ordinary mirrored tiles
};
struct MaterialSurface {
  albedo: vec3f,
  normal: vec3f,
  roughness: f32,
  metallic: f32,
  occlusion: f32,
  brush_blend: f32,
};
struct MaterialTile {
  color: vec3f,
  gradient: vec2f,
  height: f32,
};
@group(0) @binding(6) var material_tiles: texture_2d_array<f32>;
@group(0) @binding(7) var material_sampler: sampler;
@group(0) @binding(8) var<storage, read> material_levels: array<MaterialLevel>;

fn material_diffuse_light(n: vec3f) -> vec3f {
  return vec3f(0.32+dot(n,vec3f(0.16,0.2,0.08)));
}
fn material_display(linear: vec3f) -> vec3f {
  let c=clamp(linear,vec3f(0.0),vec3f(1.0));
  return select(12.92*c,1.055*pow(c,vec3f(1.0/2.4))-vec3f(0.055),c>vec3f(0.0031308));
}
fn material_compensate(a: vec4f, b: vec4f, mean: vec4f, t: f32) -> vec4f {
  let scale=inverseSqrt(t*t+(1.0-t)*(1.0-t));
  let value=mean+mix(a-mean,b-mean,t)*scale;
  return vec4f(clamp(value.rgb,vec3f(0.0),vec3f(1.0)),value.a);
}
fn material_unit(v: vec3f, fallback: vec3f) -> vec3f {
  let square=dot(v,v);
  if (square<1e-20) { return fallback; }
  return v*inverseSqrt(square);
}
fn material_hash(cell: vec2f, seed: u32) -> u32 {
  let c=vec2u(fract(cell/256.0)*256.0);
  var h=(c.x*0x8da6b343u)^(c.y*0xd8163841u)^seed;
  h=(h^(h>>16u))*0x7feb352du;
  h=(h^(h>>15u))*0x846ca68bu;
  return h^(h>>16u);
}
fn material_random(cell: vec2f, seed: u32) -> f32 {
  return f32(material_hash(cell,seed)>>8u)/16777216.0;
}
fn material_noise(p: vec2f, seed: u32) -> f32 {
  let cell=floor(p*0.25)*4.0;
  let f=fract(p*0.25);let t=f*f*(3.0-2.0*f);
  return mix(mix(material_random(cell,seed),material_random(cell+vec2f(4.0,0.0),seed),t.x),
    mix(material_random(cell+vec2f(0.0,4.0),seed),material_random(cell+vec2f(4.0),seed),t.x),t.y);
}
fn material_sample(uv:vec2f,layer:i32,level:f32)->vec4f {
  return textureSampleLevel(material_tiles,material_sampler,uv,layer,clamp(level,0.0,9.0));
}
fn material_mirror_uv(uv:vec2f,origin:vec2f)->vec2f {
  // Keep precise fractions separate from cell identity; address mode supplies reflection.
  return fract(uv)+2.0*fract((floor(uv)+origin)*0.5);
}
fn material_mirrored(uv: vec2f, origin: vec2f, layer: i32, lod: f32) -> vec4f {
  return material_sample(material_mirror_uv(uv,origin),layer,lod);
}
struct MaterialAtlas {uv:vec2f,local:vec2f,grid:vec2f,cell:vec2f};
fn material_atlas(uv:vec2f,origin:vec2f,level:MaterialLevel)->MaterialAtlas {
  let grid=vec2f(f32(level.identity.w&65535u),f32(level.identity.w>>16u));
  let world=(uv+origin)*grid;let row=floor(world.y);
  let bond=vec2f(world.x+fract(row*0.5),world.y);
  let cell=floor(bond);let local=fract(bond);
  let seed=material_hash(cell,level.identity.y+571u);
  let chosen=seed%u32(grid.x*grid.y);
  let face=vec2f(f32(chosen%u32(grid.x)),f32(chosen/u32(grid.x)));
  let mirrored=select(local,1.0-local,vec2<bool>((seed&256u)!=0u,(seed&512u)!=0u));
  return MaterialAtlas((face+mirrored)/grid,local,grid,cell);
}
fn material_value(uv: vec2f, origin: vec2f, level: MaterialLevel, lod: f32) -> vec4f {
  let layer=i32(level.identity.y);
  if (level.identity.w!=0u) {
    let mapped=material_atlas(uv,origin,level);
    var value=material_sample(mapped.uv,layer,lod);
    value=vec4f(value.rgb*(1.0+(material_random(mapped.cell,91u)-0.5)*0.24*smoothstep(0.2,0.6,value.a)),value.a);
    let distance=min(mapped.local,1.0-mapped.local)/(exp2(lod)*mapped.grid/512.0);
    value=mix(level.border,value,smoothstep(0.0,1.0,min(distance.x,distance.y)));
    return mix(value,level.mean,smoothstep(4.0,8.0,lod));
  }
  let a=material_mirrored(uv,origin,layer,lod);
  let b=material_mirrored(uv+vec2f(0.371,0.613),origin,layer,lod);
  let t=0.3+0.4*material_noise(uv+origin,level.identity.y+317u);
  return material_compensate(a,b,level.mean,t);
}
// Zero keeps raw height differentials; one limits byte noise in shading normals.
fn material_tile(uv: vec2f, origin: vec2f, level: MaterialLevel, lod: f32, normal_filter:f32) -> MaterialTile {
  let edge=f32(textureDimensions(material_tiles,0).x);
  let support=surface_slope_step(exp2(lod)/edge,normal_filter*sqrt(2.0)*level.surface.y/(255.0*level.surface.x));
  let gradient_lod=select(lod,max(lod,min(9.0,log2(support*edge))),normal_filter>0.0);
  let step=exp2(gradient_lod)/edge;
  let center=material_value(uv,origin,level,lod);
  let right=material_value(uv+vec2f(step,0.0),origin,level,gradient_lod).a;
  let left=material_value(uv-vec2f(step,0.0),origin,level,gradient_lod).a;
  let up=material_value(uv+vec2f(0.0,step),origin,level,gradient_lod).a;
  let down=material_value(uv-vec2f(0.0,step),origin,level,gradient_lod).a;
  return MaterialTile(center.rgb,vec2f(right-left,up-down)/(2.0*step),center.a);
}
struct MaterialCoords {uv:vec3f,cells:vec3f};
fn material_coords(level:MaterialLevel,delta:vec3f)->MaterialCoords {
  // delta is already object-local, not a camera-to-hit offset.
  return MaterialCoords(delta/level.surface.x,vec3f(0.0));
}
fn material_weights(n:vec3f)->vec3f {
  let projection=pow(abs(n),vec3f(8.0));
  return projection/(projection.x+projection.y+projection.z);
}
fn material_lod(footprint:f32,extent:f32)->f32 {
  return clamp(log2(max(1e-30,footprint)*512.0/extent),0.0,9.0);
}
fn material_visible(lod:f32)->f32 {return 1.0-smoothstep(4.0,8.0,lod);}
struct MaterialLayers {base:vec3f,modulation:vec3f,gradient:vec3f,variance:f32,occlusion:f32};
fn material_layer(input:MaterialLayers,index:u32,material:u32,delta:vec3f,weights:vec3f,footprint:f32,normal_filter:f32)->MaterialLayers {
  var state=input;
  if(index>=arrayLength(&material_levels)){return state;}
  let level=material_levels[index];if(level.identity.x!=material){return state;}
  let extent=level.surface.x;let lod=material_lod(footprint,extent);
  let base_order=(level.identity.z&1u)!=0u;let visible=material_visible(lod);
  state.variance+=level.surface.w*(1.0-visible);
  if(base_order&&lod>=9.0){state.base=level.mean.rgb;return state;}
  if(!base_order&&visible==0.0){return state;}
  let coords=material_coords(level,delta);let p=coords.uv;let cells=coords.cells;
  var color=vec3f(0.0);var height=0.0;var slope=vec3f(0.0);
  if(weights.x>0.0){
    let q=material_tile(p.yz,cells.yz,level,lod,normal_filter);
    color+=q.color*weights.x;height+=q.height*weights.x;slope+=vec3f(0.0,q.gradient)*weights.x;
  }
  if(weights.y>0.0){
    let q=material_tile(p.xz,cells.xz,level,lod,normal_filter);
    color+=q.color*weights.y;height+=q.height*weights.y;slope+=vec3f(q.gradient.x,0.0,q.gradient.y)*weights.y;
  }
  if(weights.z>0.0){
    let q=material_tile(p.xy,cells.xy,level,lod,normal_filter);
    color+=q.color*weights.z;height+=q.height*weights.z;slope+=vec3f(q.gradient,0.0)*weights.z;
  }
  if(base_order){state.base=color;}
  else{state.modulation*=mix(vec3f(1.0),color/max(level.mean.rgb,vec3f(1.0/255.0)),level.surface.z*visible);}
  state.gradient+=slope*(level.surface.y/extent)*visible;
  if(material==1u&&!base_order){state.occlusion*=mix(1.0,0.08+0.92*smoothstep(0.12,0.56,height),visible);}
  return state;
}
fn material_layers(material:u32,delta:vec3f,n:vec3f,footprint:f32,normal_filter:f32)->MaterialLayers {
  let weights=material_weights(n);let roughness=select(0.26,0.86,material==1u);
  var state=MaterialLayers(vec3f(0.5),vec3f(1.0),vec3f(0.0),roughness*roughness,1.0);
  // @material-levels
  return state;
}
fn layered_material(material:u32,delta:vec3f,normal:vec3f,footprint:f32)->MaterialSurface {
  let n=material_unit(normal,vec3f(0.0,1.0,0.0));
  let state=material_layers(material,delta,n,footprint,1.0);
  let tangent=state.gradient-n*dot(n,state.gradient);
  let bumped=material_unit(n-tangent/max(1.0,length(tangent)/2.0),n);
  return MaterialSurface(clamp(state.base*state.modulation,vec3f(0.004),vec3f(0.98)),bumped,
    clamp(sqrt(state.variance),0.05,0.98),select(0.0,1.0,material==0u),state.occlusion,material_weights(n).z);
}

// Analytic studio lobes/ground, not a photographed environment or area-light integral.
fn material_environment(surface: MaterialSurface, v: vec3f) -> vec3f {
  let n=surface.normal;let r=reflect(-v,n);
  let a=normalize(vec3f(-0.6,0.7,1.0));let b=normalize(vec3f(1.0,0.2,0.3));
  let c=normalize(vec3f(-0.2,1.0,-0.7));
  let studio=0.7*max(0.0,dot(n,a))*material_response(surface,v,a,0.03)
    +0.42*max(0.0,dot(n,b))*material_response(surface,v,b,0.03)
    +0.35*max(0.0,dot(n,c))*material_response(surface,v,c,0.03);
  return mix(vec3f(0.06,0.055,0.045),vec3f(0.28,0.32,0.39),smoothstep(-0.3,0.8,r.y))
    +vec3f(studio);
}
fn material_lobe(n: vec3f, v: vec3f, l: vec3f, h: vec3f, roughness: f32, anisotropy: f32, axis: vec3f, spread: f32) -> f32 {
  let projected=axis-n*dot(axis,n);
  let strength=anisotropy*smoothstep(0.0,0.01,dot(projected,projected));
  let tangent=material_unit(projected,vec3f(1.0,0.0,0.0));
  let bitangent=cross(n,tangent);
  let aspect=sqrt(1.0-0.9*strength);
  let intrinsic=roughness*roughness*vec2f(aspect,1.0/aspect);
  let alpha=max(vec2f(0.025),sqrt(intrinsic*intrinsic+vec2f(spread*spread)));
  let nh=max(0.0,dot(n,h));let nv=max(0.0001,dot(n,v));let nl=max(0.0,dot(n,l));
  let ellipse=vec3f(dot(h,tangent)/alpha.x,dot(h,bitangent)/alpha.y,nh);
  let denominator=dot(ellipse,ellipse);
  let distribution=1.0/(PI*alpha.x*alpha.y*denominator*denominator);
  let view=nl*length(vec3f(alpha.x*dot(v,tangent),alpha.y*dot(v,bitangent),nv));
  let light=nv*length(vec3f(alpha.x*dot(l,tangent),alpha.y*dot(l,bitangent),nl));
  return 0.5*distribution/max(0.0001,view+light);
}
fn material_response(surface: MaterialSurface, v: vec3f, l: vec3f, spread: f32) -> f32 {
  let n=surface.normal;let h=material_unit(v+l,n);
  let a=material_lobe(n,v,l,h,surface.roughness,0.6*surface.metallic,vec3f(0.0,0.0,1.0),spread);
  if (surface.metallic<0.5) { return a; }
  return mix(a,material_lobe(n,v,l,h,surface.roughness,0.6,vec3f(0.0,1.0,0.0),spread),surface.brush_blend);
}
fn material_shade(surface: MaterialSurface, p: vec3f, ray: vec3f, light: vec3f, shadow: f32) -> vec3f {
  let n=surface.normal;
  let v=-ray;
  let l=material_unit(light-p,n);
  let h=material_unit(v+l,n);
  let nv=max(0.0001,dot(n,v));
  let nl=max(0.0,dot(n,l));
  let vh=max(0.0,dot(v,h));
  let f0=mix(vec3f(0.04),surface.albedo,surface.metallic);
  let fresnel=f0+(vec3f(1.0)-f0)*pow(1.0-vh,5.0);
  let specular=fresnel*material_response(surface,v,l,0.0);
  let diffuse=(vec3f(1.0)-fresnel)*(1.0-surface.metallic)*surface.albedo/PI;
  let direct=(diffuse+specular)*vec3f(3.2,3.05,2.85)*nl*shadow*surface.occlusion;
  let reflected=material_environment(surface,v);
  let environment_fresnel=f0+(vec3f(1.0)-f0)*pow(1.0-nv,5.0);
  let ambient=(reflected*environment_fresnel
    + material_diffuse_light(n)*(1.0-surface.metallic)*surface.albedo)*surface.occlusion;
  let color=direct+ambient;
  return material_display((color*(2.51*color+vec3f(0.03)))/
    (color*(2.43*color+vec3f(0.59))+vec3f(0.14)));
}
