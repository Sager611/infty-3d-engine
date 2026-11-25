const SOURCE: &str = concat!(
    include_str!("shaders/types.wgsl"),
    '\n',
    include_str!("shaders/geometry/interval.wgsl"),
    '\n',
    include_str!("shaders/geometry/polynomial.wgsl"),
    '\n',
    include_str!("shaders/geometry/sphere.wgsl"),
    '\n',
    include_str!("shaders/geometry/plane.wgsl"),
    '\n',
    include_str!("shaders/geometry/goursat.wgsl"),
    '\n',
    include_str!("shaders/geometry/crystal.wgsl"),
    '\n',
    include_str!("shaders/geometry/finite.wgsl"),
    '\n',
    include_str!("shaders/geometry/surface.wgsl"),
    '\n',
    include_str!("shaders/materials/heightfield.wgsl"),
    '\n',
    include_str!("shaders/scene.wgsl"),
    '\n',
    include_str!("shaders/materials/detail.wgsl"),
    '\n',
    include_str!("shaders/materials/layered.wgsl"),
    '\n',
    include_str!("shaders/materials/texh.wgsl"),
    '\n',
    include_str!("shaders/materials/leaf.wgsl"),
    "\n",
    include_str!("shaders/materials/surface.wgsl"),
    '\n',
    include_str!("shaders/materials/shadow.wgsl"),
    "\n",
    include_str!("shaders/render.wgsl"),
    '\n',
    include_str!("shaders/verify.wgsl"),
    '\n',
    include_str!("shaders/geometry/finite_verify.wgsl"),
);

// Expand immutable level metadata once when compiling, never per rendered hit.
// Additional material orders require assets/metadata, not a shader traversal.
pub fn source() -> String {
    use std::fmt::Write as _;
    let mut height = String::new();
    let mut material = String::new();
    let mut leaf = String::new();
    for (index, _) in crate::material::presets_with_leaf(true).iter().enumerate() {
        let _ = writeln!(height, "height+=texh_level({index}u,foot,context);");
        let _ = writeln!(
            material,
            "state=material_layer(state,{index}u,material,delta,weights,footprint,normal_filter);"
        );
        let _ = writeln!(leaf, "color=leaf_level(color,{index}u,p,size,footprint);");
    }
    SOURCE
        .replace("// @texh-levels", &height)
        .replace("// @material-levels", &material)
        .replace("// @leaf-levels", &leaf)
}
