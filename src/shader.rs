pub const SOURCE: &str = concat!(
    include_str!("shaders/types.wgsl"),
    '\n',
    include_str!("shaders/geometry/sphere.wgsl"),
    '\n',
    include_str!("shaders/geometry/plane.wgsl"),
    '\n',
    include_str!("shaders/materials/detail.wgsl"),
    '\n',
    include_str!("shaders/render.wgsl"),
    '\n',
    include_str!("shaders/verify.wgsl"),
);
