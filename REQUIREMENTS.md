# Requirements

- Thousands of mixed, instanced objects, with a web selector to load a showcase and return to the default scene.
- Extensible geometry; no ray marching.
- Three intersecting ridged cones with genuine multiscale geometry, CPU/GPU queries and web creation. New cluster kernels use fixed direct analytic work: no iteration, recursion or traversal.
- Intuitive web controls to add and delete geometry instances.
- Modular geometry definitions, instances and query kernels, including closest-point queries with explicit accuracy and unresolved results.
- Free-camera WASD movement and drag look; wheel changes travel speed. Only straight W approaches to nearby center-ray hits ease down to a minimum speed; reverse, strafe and diagonal movement retain normal speed. Minimum travel speed and the approach floor are0.000001 units/s for close inspection.

## Changelog
| When | Who | What | Why |
|---|---|---|---|
| 2026-11-08 | Assistant | Recorded the complete two-item requirements | Keep project scope explicit |
| 2026-11-08 | User | Required intuitive add/delete controls | Create and remove instances in the visualization |
| 2026-11-08 | User | Required modular geometry queries and free-camera surface inspection | Keep implementation scope explicit |
| 2026-11-09 | User | Required a localhost showcase scene selector | Make thousands of instances directly viewable in the app |
| 2026-11-09 | User | Required slower minimum travel near geometry | Allow much closer surface inspection |
| 2026-11-09 | User | Required a directly addressed multiscale crystalline cluster | Add geometric detail without iterative kernels |
