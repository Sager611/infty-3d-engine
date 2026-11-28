//! Shader interface every geometry implements. Shared WGSL reaches a geometry only through generated
//! `geometry_<name>` dispatchers and `geometry_<trait>(kind)` tests; each geometry defines
//! `<prefix>_<name>` for every function, so a missing or mistyped one fails shader validation.
use super::{Kind, Spec};
use std::fmt::Write as _;

/// A function every geometry defines; its `SceneObject` parameter (else `kind:u32`) selects the geometry.
pub struct Function {
    pub name: &'static str,
    pub params: &'static str,
    pub returns: &'static str,
}

const fn function(name: &'static str, params: &'static str, returns: &'static str) -> Function {
    Function {
        name,
        params,
        returns,
    }
}

pub const FUNCTIONS: [Function; 7] = [
    // Canonical ray query: hit, or closest-approach witness with bounds.
    function(
        "query",
        "o:vec3f,d:vec3f,object:SceneObject",
        "GeometryQuery",
    ),
    // Carrier point and UV near a sample from its tangent-plane projection; invalid outside the surface.
    function(
        "surface_uv",
        "surface:SurfacePoint,point:vec3f,base:SceneSample,object:SceneObject",
        "SurfacePoint",
    ),
    // Normal curvature along the ray's tangent direction, local units; positive bends away.
    function(
        "curvature",
        "base:SceneSample,d:vec3f,object:SceneObject",
        "f32",
    ),
    // Curvature of a closed surface's smallest enclosing sphere; 0 when open.
    function("enclosing_curvature", "object:SceneObject", "f32"),
    // Conservative: may a field material's relief here meet the ray before `limit`?
    function(
        "field_relevant",
        "o:vec3f,d:vec3f,object:SceneObject,limit:f32,padding:f32",
        "bool",
    ),
    // Conservative: may the ray's chord through the relief slab cross the field's coverage?
    function("field_chord", "o:vec3f,d:vec3f,object:SceneObject", "bool"),
    // Lit display-mode color.
    function(
        "shade",
        "kind:u32,q:Query,n:vec3f,local:vec3f,ray:vec3f,height:f32,size:f32,mode:u32,diffuse:f32,shadow:f32",
        "vec3f",
    ),
];

/// One generated `geometry_<name>(kind)` test per trait.
type Trait = (&'static str, fn(Spec) -> bool);
const TRAITS: [Trait; 3] = [
    ("bounded", |spec| spec.bounded),
    ("card", |spec| spec.card),
    ("wraps_u", |spec| spec.wraps_u),
];

/// Trait tests as tag equality chains, then one single-exit if-chain dispatcher per function over every
/// geometry. Metal compiles bit tests, a `switch` or a `return` per branch 1-4 s slower.
pub fn wgsl() -> String {
    let mut out = String::new();
    for (name, has) in TRAITS {
        let tags: Vec<_> = Kind::ALL
            .into_iter()
            .filter(|kind| has(kind.spec()))
            .map(|kind| format!("kind=={}u", kind.tag()))
            .collect();
        let test = if tags.is_empty() {
            "false".to_owned()
        } else {
            tags.join("||")
        };
        let _ = writeln!(out, "fn geometry_{name}(kind:u32)->bool {{return {test};}}");
    }
    for Function {
        name,
        params,
        returns,
    } in FUNCTIONS
    {
        let args: Vec<_> = params
            .split(',')
            .filter_map(|p| p.split(':').next())
            .collect();
        let selector = params
            .split(',')
            .find_map(|p| p.strip_suffix(":SceneObject"))
            .map_or_else(
                || String::from("kind"),
                |object| format!("{object}.identity.y"),
            );
        let _ = write!(
            out,
            "fn geometry_{name}({params})->{returns} {{let tag={selector};var result:{returns};"
        );
        for (index, kind) in Kind::ALL.into_iter().enumerate() {
            let call = format!(
                "{{result={}_{name}({});}}",
                kind.spec().prefix,
                args.join(",")
            );
            let _ = match index {
                0 => write!(out, "if(tag=={}u){call}", kind.tag()),
                _ if index + 1 == Kind::ALL.len() => write!(out, "else{call}"),
                _ => write!(out, "else if(tag=={}u){call}", kind.tag()),
            };
        }
        out.push_str("return result;}\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{FUNCTIONS, Kind, wgsl};

    #[test]
    fn every_geometry_has_a_unique_tag_and_prefix() {
        for (index, kind) in Kind::ALL.into_iter().enumerate() {
            assert_eq!(Kind::from_tag(kind.tag()), Some(kind));
            for other in &Kind::ALL[index + 1..] {
                assert_ne!(kind.tag(), other.tag());
                assert_ne!(kind.spec().prefix, other.spec().prefix);
            }
        }
    }

    #[test]
    fn every_function_dispatches_to_every_geometry() {
        let wgsl = wgsl();
        for function in FUNCTIONS {
            assert!(wgsl.contains(&format!("fn geometry_{}(", function.name)));
            for kind in Kind::ALL {
                let call = format!("result={}_{}(", kind.spec().prefix, function.name);
                assert!(wgsl.contains(&call), "{call}");
            }
        }
    }

    /// Hand-written WGSL never compares or switches on a geometry tag; test harnesses (`verify*`) may.
    #[test]
    fn shared_shaders_never_branch_on_geometry() {
        let generated = wgsl();
        let mut function = String::new();
        let mut found = Vec::new();
        for line in crate::shader::source().lines() {
            if let Some(name) = line.trim_start().strip_prefix("fn ") {
                function = name.split('(').next().unwrap_or_default().to_owned();
            }
            if function.starts_with("verify") || generated.lines().any(|g| g == line) {
                continue;
            }
            let line: String = line
                .split("//")
                .next()
                .unwrap_or_default()
                .split_whitespace()
                .collect();
            let compares = |name: &str| {
                line.match_indices(name).any(|(at, _)| {
                    let before = line[..at]
                        .trim_end_matches(|c: char| c.is_alphanumeric() || "_.".contains(c));
                    let after = &line[at + name.len()..];
                    ["==", "!="]
                        .iter()
                        .any(|op| before.ends_with(op) || after.starts_with(op))
                })
            };
            if compares("kind")
                || compares("identity.y")
                || (line.contains("switch")
                    && (line.contains("kind") || line.contains("identity.y")))
            {
                found.push(format!("{function}: {line}"));
            }
        }
        assert!(found.is_empty(), "{found:#?}");
    }
}
