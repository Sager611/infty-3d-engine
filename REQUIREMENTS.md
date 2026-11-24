# Requirements

- Thousands of mixed, instanced objects, with a web selector to load a showcase and return to the default scene.
- Extensible geometry; no ray marching.
- Three intersecting ridged cones with genuine multiscale geometry, CPU/GPU queries and web creation. New cluster kernels use fixed direct analytic work: no iteration, recursion or traversal.
- Materials view with selectable brushed steel and red brickwork/mortar. Each begins with four repeating512×512 albedo/relief levels,10× apart; finer levels reveal grooves, pores and mineral grains, not miniature bricks. Support additional coarser/finer scales; visually check sphere, plane and Goursat up close through microscopic distances. Ground appearance in inspected references; preserve geometry, silhouettes, queries and navigation.
- Intuitive web controls to add and delete geometry instances.
- Modular geometry definitions, instances and query kernels, including closest-point queries with explicit accuracy and unresolved results.
- Free-camera WASD movement and drag look; wheel changes travel speed. Only straight W approaches to nearby center-ray hits ease down to a minimum speed; reverse, strafe and diagonal movement retain normal speed. Minimum travel speed and the approach floor are0.000001 units/s for close inspection.
- Base ray queries always return a gap and matching surface witness with UV/normal; preserve explicit uncertainty and independent point queries.
- Texh: on a base miss, bilinearly sample height once at the witness UV. Accept if signed height reaches the gap toward the ray. Preserve unmodified base hits and cutouts. Expand culling by texture reach; no displaced-ray traversal.
- Scene → Leaf · deep cup opens an angled view. Keep original RGB, four bands (broadest gain1), deep stem and raised edges. Solve the broad cup analytically for candidate UVs; one branchless bilinear height sample corrects hit depth, then the cutout clips the projected UV. Point queries stay independent. Compare warm displayed FPS at1265×791; target60FPS.

## Changelog

| When | Who | What | Why |
| --- | --- | --- | --- |
| 2026-11-08 | Assistant | Recorded the complete two-item requirements | Keep project scope explicit |
| 2026-11-08 | User | Required intuitive add/delete controls | Create and remove instances in the visualization |
| 2026-11-08 | User | Required modular geometry queries and free-camera surface inspection | Keep implementation scope explicit |
| 2026-11-09 | User | Required a localhost showcase scene selector | Make thousands of instances directly viewable in the app |
| 2026-11-09 | User | Required slower minimum travel near geometry | Allow much closer surface inspection |
| 2026-11-09 | User | Required a directly addressed multiscale crystalline cluster | Add geometric detail without iterative kernels |
| 2026-11-09 | User | Required layered steel/brick materials and microscopic visual checks | Reveal physical surface detail across scales |
| 2026-11-09 | Assistant | Clarified reference-grounded, appearance-only materials | Preserve geometry/query/navigation behavior |
| 2026-11-23 | User | Doubled broadest Leaf height | Exaggerate largest-scale geometry |
| 2026-11-23 | User | Set broadest Leaf height to32× original | Make broad folds obvious |
| 2026-11-24 | User | Replace displaced rays with generic texh | One sample instead of traversal |
| 2026-11-24 | User | Remove hit height work and cup the Leaf heightmap | Faster close-ups and a lower stem |
| 2026-11-24 | User | Require branchless bilinear height sampling | Smooth subtexel gaps without search |
| 2026-11-24 | User | Expose the deeper Leaf and displace ray hits | Show true relief from the Scene menu |
