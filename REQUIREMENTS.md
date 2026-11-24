# Requirements

- Thousands of mixed, instanced objects, with a web selector to load a showcase and return to the default scene.
- Extensible geometry; no ray marching.
- Three intersecting ridged cones with genuine multiscale geometry, CPU/GPU queries and web creation. New cluster kernels use fixed direct analytic work: no iteration, recursion or traversal.
- Materials view with selectable brushed steel and red brickwork/mortar. Each begins with four repeating512×512 albedo/relief levels,10× apart; finer levels reveal grooves, pores and mineral grains, not miniature bricks. Support additional coarser/finer scales; visually check sphere, plane and Goursat up close through microscopic distances. Ground appearance in inspected references; preserve carrier geometry, picking and navigation.
- Intuitive web controls to add and delete geometry instances.
- Modular geometry definitions, instances and query kernels, including closest-point queries with explicit accuracy and unresolved results.
- Free-camera WASD movement and drag look; wheel changes travel speed. Only straight W approaches to nearby center-ray hits ease down to a minimum speed; reverse, strafe and diagonal movement retain normal speed. Minimum travel speed and the approach floor are0.000001 units/s for close inspection.
- Base ray queries always return a gap and matching surface witness with UV/normal; preserve explicit uncertainty and independent point queries.
- Texh rendered hits: one shared, geometry-independent direct calculation. NO LOOPS or recursion; always prefer fast approximation over accurate displacement. No root search, traversal, ray marching or fitted geometry. Sample carrier height once for depth; update UV/RGB and finite-difference normals with a frozen carrier footprint/frame. Bound grazing offsets. No exact first-crossing or displaced-silhouette guarantee.
- Preserve carrier queries/picking, independent point queries and the existing miss rule: one bilinear height sample at the witness accepts signed height reaching the gap. No new gap solver.
- Scene → Leaf · relief and Red brick · relief: share texh, preserve matching source assets, cutouts, all four height bands and carrier cast-shadow behavior. Both localhost previews target60 warm FPS at1265×791; report measured limits.

## Changelog

| When | Who | What | Why |
| --- | --- | --- | --- |
| 2026-11-24 | User | Require shared direct, loop-free/nonrecursive displacement; speed over accuracy | Replace iterative search without changing carrier queries, gaps or assets |
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
| 2026-11-25 | User | Require veined Leaf geometry and recessed brick hits | Natural multiscale relief |
| 2026-11-24 | User | Reduce Leaf curl and vein relief | Avoid exaggerated folds |
| 2026-11-24 | User | Ground Leaf depth in real references | Preserve irregular blade and vein contours |
