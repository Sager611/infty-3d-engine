# Domain

| Term | Meaning |
|---|---|
| Geometry definition | Immutable validated canonical shape and parameters, shared by instances |
| Shape kernel | Analytic intersections, surface coordinates and bounded geometry queries in canonical space |
| Instance | Stable scene ID, geometry reference and positive uniform transform |
| Uniform transform | Translation plus one positive scale; distances and errors scale once |
| Prepared scene | Immutable instances, shared definitions and conservative traversal data |
| Scene query | Combines kernel results with deterministic ordering; no browser/GPU state |
| Closest point | Surface witness plus unsigned distance bounds, position error and multiplicity |
| Ray gap | Distance bounds between the surface and a forward ray; not a point-distance query |
| Unresolved | Requested accuracy or work could not be completed; never a fabricated miss |
| Evaluation budget | Explicit bounded kernel/refinement work, not elapsed time |
| Query trace | Optional bounded numerical decisions; no normal-path logging |
| Free camera | Position and view direction; WASD translates, dragging looks, wheel changes speed |
| Detail approach | Only straight W eases near a center-ray hit; other travel keeps its base speed |

## Changelog
| When | Who | What | Why |
|---|---|---|---|
| 2026-11-08 | Assistant | Defined geometry, query and navigation terms | Keep module interfaces and numerical claims consistent |
