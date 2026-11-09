//! Conservative threaded hierarchy; planes remain unbounded leaves.
use super::model::Object;

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Bounds {
    #[must_use]
    pub fn object(object: Object) -> Option<Self> {
        let radius = object.geometry().radius()?;
        let transform = object.transform();
        Some(Self {
            min: transform.world_point([-radius; 3]),
            max: transform.world_point([radius; 3]),
        })
    }

    #[must_use]
    pub fn intersects(self, o: [f64; 3], d: [f64; 3]) -> bool {
        let mut near: f64 = 0.0;
        let mut far = f64::INFINITY;
        for axis in 0..3 {
            // Inflate against roundoff, including near-parallel/tangent rays.
            let pad = 1e-10 * self.min[axis].abs().max(self.max[axis].abs()).max(1.0);
            let lo = self.min[axis] - pad;
            let hi = self.max[axis] + pad;
            if d[axis] == 0.0 {
                if o[axis] < lo || o[axis] > hi {
                    return false;
                }
            } else {
                let a = (lo - o[axis]) / d[axis];
                let b = (hi - o[axis]) / d[axis];
                near = near.max(a.min(b));
                far = far.min(a.max(b));
            }
        }
        near <= far
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Node {
    Branch { bounds: Bounds, escape: usize },
    Leaf(usize),
}

#[derive(Debug)]
pub struct Spatial {
    pub nodes: Vec<Node>,
}

impl Spatial {
    #[must_use]
    pub fn new(objects: &[Object]) -> Self {
        let (mut finite, planes): (Vec<_>, Vec<_>) =
            (0..objects.len()).partition(|&i| Bounds::object(objects[i]).is_some());
        let mut nodes = Vec::with_capacity(objects.len() * 2);
        build(&mut finite, objects, &mut nodes);
        nodes.extend(planes.into_iter().map(Node::Leaf));
        Self { nodes }
    }

    #[must_use]
    pub fn candidates(&self, o: [f64; 3], d: [f64; 3]) -> Vec<usize> {
        let mut result = Vec::new();
        let mut index = 0;
        while let Some(node) = self.nodes.get(index) {
            match *node {
                Node::Branch { bounds, escape } if !bounds.intersects(o, d) => {
                    index = escape;
                    continue;
                }
                Node::Leaf(slot) => result.push(slot),
                Node::Branch { .. } => {}
            }
            index += 1;
        }
        result
    }
}

fn build(indices: &mut [usize], objects: &[Object], nodes: &mut Vec<Node>) {
    if indices.is_empty() {
        return;
    }
    if indices.len() == 1 {
        nodes.push(Node::Leaf(indices[0]));
        return;
    }
    let mut bounds = Bounds {
        min: [f64::INFINITY; 3],
        max: [f64::NEG_INFINITY; 3],
    };
    for &index in indices.iter() {
        if let Some(b) = Bounds::object(objects[index]) {
            for axis in 0..3 {
                bounds.min[axis] = bounds.min[axis].min(b.min[axis]);
                bounds.max[axis] = bounds.max[axis].max(b.max[axis]);
            }
        }
    }
    let axis = (0..3)
        .max_by(|&a, &b| {
            (bounds.max[a] - bounds.min[a]).total_cmp(&(bounds.max[b] - bounds.min[b]))
        })
        .unwrap_or(0);
    indices.sort_unstable_by(|&a, &b| {
        objects[a].position()[axis]
            .total_cmp(&objects[b].position()[axis])
            .then(objects[a].id().cmp(&objects[b].id()))
    });
    let start = nodes.len();
    nodes.push(Node::Branch { bounds, escape: 0 });
    let middle = indices.len() / 2;
    let (left, right) = indices.split_at_mut(middle);
    build(left, objects, nodes);
    build(right, objects, nodes);
    nodes[start] = Node::Branch {
        bounds,
        escape: nodes.len(),
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::model::Kind;
    use crate::{
        math::Ray,
        scene::query::{QueryMode, query_object, query_objects},
    };

    #[test]
    fn accelerated_edges_match_exhaustive_ties_inside_parallel_and_tangent_queries()
    -> Result<(), Box<dyn std::error::Error>> {
        let objects = [
            Object::new(9, Kind::Sphere, [0.0; 3], 1.0, 0.0)?,
            Object::new(2, Kind::Sphere, [0.0; 3], 1.0, 0.0)?,
            Object::new(7, Kind::Goursat, [2.0, -1.0, 0.0], 2.0, 1.5625)?,
            Object::new(8, Kind::Goursat, [-2.0, 1.0, 0.0], 0.01, 1.5)?,
            Object::new(u32::MAX, Kind::Plane, [0.0, -2.0, 0.0], 4.0, 0.0)?,
        ];
        let rays = [
            Ray {
                o: [1.0, 0.0, -3.0],
                d: [0.0, 0.0, 1.0],
            },
            Ray {
                o: [0.0; 3],
                d: [1.0, 0.0, 0.0],
            },
            Ray {
                o: [3.0, 0.0, 0.0],
                d: [-1.0, 0.0, 0.0],
            },
            Ray {
                o: [0.0, 1.0, 0.0],
                d: [1.0, -1e-12, 0.0],
            },
            Ray {
                o: [-1e-12, -2.0, 0.0],
                d: [0.0, 0.0, -1.0],
            },
            Ray {
                o: [20.0; 3],
                d: [0.6, 0.8, 0.0],
            },
            Ray {
                o: [-3.0, 1.0, 2.0],
                d: [2.0, -0.25, -1.0],
            },
        ];
        for ray in rays {
            for mode in [QueryMode::First, QueryMode::All, QueryMode::Inspect] {
                let mut brute = objects
                    .iter()
                    .map(|object| query_object(object, ray, mode == QueryMode::Inspect))
                    .collect::<Result<Vec<_>, _>>()?;
                if mode != QueryMode::Inspect {
                    brute.retain(|sample| sample.sample.hit);
                }
                brute.sort_by(|a, b| {
                    (if mode == QueryMode::Inspect {
                        b.sample.hit.cmp(&a.sample.hit)
                    } else {
                        std::cmp::Ordering::Equal
                    })
                    .then_with(|| a.sample.t.total_cmp(&b.sample.t))
                    .then(a.id.cmp(&b.id))
                });
                if mode == QueryMode::First {
                    brute.truncate(1);
                }
                let actual = query_objects(&objects, ray, mode)?;
                assert_eq!(
                    actual
                        .iter()
                        .map(|sample| sample.packed(ray).map(f64::to_bits))
                        .collect::<Vec<_>>(),
                    brute
                        .iter()
                        .map(|sample| sample.packed(ray).map(f64::to_bits))
                        .collect::<Vec<_>>()
                );
            }
        }
        Ok(())
    }

    #[test]
    fn mixed_hierarchy_matches_brute_force_and_keeps_planes()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut objects = Vec::new();
        for id in 1..=4096_u32 {
            objects.push(Object::new(
                id,
                if id % 2 == 0 {
                    Kind::Goursat
                } else {
                    Kind::Sphere
                },
                [
                    f64::from(id % 16) - 8.0,
                    f64::from(id / 16 % 16) - 8.0,
                    f64::from(id / 256) - 8.0,
                ],
                0.12,
                if id % 2 == 0 { 1.5 } else { 0.0 },
            )?);
        }
        objects.push(Object::new(u32::MAX, Kind::Plane, [0.0; 3], 1.0, 0.0)?);
        let tree = Spatial::new(&objects);
        for x in [-20.0, -7.0, 0.0, 7.0, 20.0] {
            let ray = Ray {
                o: [x, 10.0, -20.0],
                d: [0.0, -0.2, 1.0],
            };
            let candidates = tree.candidates(ray.o, ray.d);
            assert!(candidates.len() < objects.len() / 8);
            assert!(candidates.contains(&4096));
            let mut brute = objects
                .iter()
                .map(|object| query_object(object, ray, false))
                .collect::<Result<Vec<_>, _>>()?;
            brute.retain(|s| s.sample.hit);
            brute.sort_by(|a, b| a.sample.t.total_cmp(&b.sample.t).then(a.id.cmp(&b.id)));
            let actual = query_objects(&objects, ray, QueryMode::All)?;
            assert_eq!(
                actual.iter().map(|s| s.packed(ray)).collect::<Vec<_>>(),
                brute.iter().map(|s| s.packed(ray)).collect::<Vec<_>>()
            );
        }
        Ok(())
    }
    #[test]
    fn all_returns_thousands_and_inspection_retains_every_miss()
    -> Result<(), Box<dyn std::error::Error>> {
        let objects = (1..=4096)
            .map(|id| {
                Object::new(
                    id,
                    if id % 2 == 0 {
                        Kind::Goursat
                    } else {
                        Kind::Sphere
                    },
                    [0.0; 3],
                    0.1,
                    if id % 2 == 0 { 1.5 } else { 0.0 },
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let ray = Ray {
            o: [0.05, 0.05, -1.0],
            d: [0.0, 0.0, 1.0],
        };
        let all = query_objects(&objects, ray, QueryMode::All)?;
        assert_eq!(all.len(), 4096);
        // Independent substitution into sphere and quartic equations.
        let sphere_t = 0.1f64.mul_add(-0.5_f64.sqrt(), 1.0);
        let quartic_t = 0.1f64.mul_add(-(1.0 + 0.375_f64.sqrt()).sqrt(), 1.0);
        for hit in &all {
            let expected = if hit.id % 2 == 0 { quartic_t } else { sphere_t };
            assert!((hit.sample.t - expected).abs() < 1e-10);
        }
        assert_eq!(all[0].id, 2);
        assert_eq!(all[2048].id, 1);
        assert_eq!(query_objects(&objects, ray, QueryMode::First)?[0].id, 2);
        let miss = Ray {
            o: [5.0, 0.0, -1.0],
            d: ray.d,
        };
        let inspect = query_objects(&objects, miss, QueryMode::Inspect)?;
        assert_eq!(inspect.len(), 4096);
        assert!(inspect.iter().all(|s| !s.sample.hit));
        Ok(())
    }
}
