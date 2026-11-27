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
    source_for(
        &crate::material::presets_with_leaf(true),
        crate::heightfield::LEAF.orders(),
    )
}
/// Any material level list and heightfield order count; code is unrolled from the data.
pub fn source_for(levels: &[crate::material::Level], orders: usize) -> String {
    use std::fmt::Write as _;
    // Levels are contiguous per material: unroll only the longest material, from its first level.
    // Unrolling every level made Metal compile 2.6x slower.
    let (mut first, mut run, mut longest) = (String::from("0u"), 0, 0);
    for (index, level) in levels.iter().enumerate() {
        run = if index > 0 && levels[index - 1].material.tag() == level.material.tag() {
            run + 1
        } else {
            first = format!(
                "select({first},{index}u,material=={}u)",
                level.material.tag()
            );
            1
        };
        longest = longest.max(run);
    }
    let mut height = String::from("let first=material_first(u32(context.object.params.w));\n");
    let mut material = String::from("let first=material_first(material);\n");
    let mut leaf = material.clone();
    for k in 0..longest {
        let _ = writeln!(
            height,
            "sum=texh_merge(sum,texh_level(first+{k}u,foot,context,typical));"
        );
        let _ = writeln!(
            material,
            "state=material_layer(state,first+{k}u,material,delta,weights,footprint,normal_filter);"
        );
        let _ = writeln!(
            leaf,
            "color=leaf_level(color,first+{k}u,material,p,size,footprint);"
        );
    }
    let (mut specs, mut carve, mut field) = (String::new(), 0_u32, String::from("false"));
    for m in crate::material::Material::ALL {
        let s = m.spec();
        // Carved bases never rise above the carrier; other orders rise at most their gain; field relief keeps its own rule.
        let rise: f32 = levels
            .iter()
            .filter(|l| l.material == m && !s.field && !(s.carve && l.order == 0))
            .map(|l| l.relief)
            .sum();
        let _ = writeln!(
            specs,
            "case {}u:{{return MaterialSpec({:?},{:?},{:?},{rise:?});}}",
            m.tag(),
            s.roughness,
            s.metallic,
            s.occlusion
        );
        carve |= u32::from(s.carve) << m.tag();
        // Float equality keeps Metal compile time flat; bit tests here cost ~9 s.
        if s.field {
            let _ = write!(field, "||material=={}.0", m.tag());
        }
    }
    let source = SOURCE
        .replace("// @material-specs", &specs)
        .replace("MATERIAL_CARVE=0u", &format!("MATERIAL_CARVE={carve}u"))
        .replace("false/*@material-field*/", &field)
        .replace("0u/*@material-first*/", &first)
        .replace("// @texh-levels", &height)
        .replace("// @material-levels", &material)
        .replace("// @leaf-levels", &leaf);
    field_orders(&source, orders)
}

// Unroll the heightfield's data-defined order count; no per-hit traversal over orders.
fn field_orders(source: &str, orders: usize) -> String {
    use std::fmt::Write as _;
    let join = |items: Vec<String>, empty: &str| {
        if items.is_empty() {
            empty.to_owned()
        } else {
            items.join("+")
        }
    };
    let repeat = |k: usize| format!("{:?}", 10_f64.powi(i32::try_from(k).unwrap_or(0)));
    let chunks = orders.div_ceil(4);
    let term = |k: usize| match k {
        _ if k >= orders => "128.0".to_owned(),
        0 | 1 => format!("hf_bilinear({k}u,uv,{})", repeat(k)),
        _ => format!("hf_gradient_value({k}u,uv,{})", repeat(k)),
    };
    let dots = |c: usize| format!("dot(abs(height_data.amplitudes[{c}]),vec4f(128.0/255.0))");
    let height = (0..chunks).map(|c| {
        let terms: Vec<_> = (0..4).map(|j| term(4 * c + j)).collect();
        format!(
            "dot(vec4f({})-vec4f(128.0),height_data.amplitudes[{c}]/255.0)",
            terms.join(",")
        )
    });
    let lanes = &"yzw"[..orders.clamp(1, 4) - 1];
    let first = match lanes.len() {
        0 => vec![],
        1 => vec!["abs(height_data.amplitudes[0].y)*(128.0/255.0)".to_owned()],
        n => vec![format!(
            "dot(abs(height_data.amplitudes[0].{lanes}),vec{n}f(128.0/255.0))"
        )],
    };
    let detail = first.into_iter().chain((1..chunks).map(dots)).collect();
    let details = (1..orders)
        .map(|k| format!("hf_render_level({k}u,uv,span)"))
        .collect();
    let slopes: Vec<_> = (0..orders)
        .map(|k| {
            let lane = ["x", "y", "z", "w"][k % 4];
            format!("hf_slope({k}u,uv,{},footprint,span,quantum_axes)*height_data.amplitudes[{}].{lane}", repeat(k), k / 4)
        })
        .collect();
    let repeats: Vec<_> = (0..orders).map(repeat).chain(["1.0".to_owned()]).collect();
    // Clip a..b against each outline side n·p <= c (Cyrus–Beck), unrolled from the coverage data.
    let outline = crate::heightfield::LEAF.outline(16).unwrap_or_default();
    let mut clips = String::new();
    for (a, b) in outline.iter().zip(outline.iter().cycle().skip(1)) {
        let n = [b[1] - a[1], a[0] - b[0]];
        let c = n[0].mul_add(a[0], n[1] * a[1]);
        let _ = writeln!(
            clips,
            "{{let n=vec2f({:.9},{:.9});let num={c:.9}-dot(a,n);let den=dot(e,n);let r=num/select(den,1.0,den==0.0);hi=select(hi,min(hi,r),den>0.0);lo=select(lo,max(lo,r),den<0.0);inside=inside&&(den!=0.0||num>=0.0);}}",
            n[0], n[1]
        );
    }
    source
        .replace(
            "const FIELD_ORDERS=4u;",
            &format!("const FIELD_ORDERS={orders}u;"),
        )
        .replace(
            "const FIELD_CHUNKS=1u;",
            &format!("const FIELD_CHUNKS={chunks}u;"),
        )
        .replace(
            "array<f32,5>(1.0,10.0,100.0,1000.0,1.0)",
            &format!("array<f32,{}>({})", orders + 1, repeats.join(",")),
        )
        .replace("0.0/*@field-height*/", &join(height.collect(), "0.0"))
        .replace(
            "0.0/*@field-reach*/",
            &join((0..chunks).map(dots).collect(), "0.0"),
        )
        .replace("0.0/*@field-detail-bound*/", &join(detail, "0.0"))
        .replace("0.0/*@field-details*/", &join(details, "0.0"))
        .replace("/*@field-outline*/", &clips)
        .replace(
            "vec2f(0.0)/*@field-slopes*/",
            &format!("({})/255.0", slopes.join("+")),
        )
}
