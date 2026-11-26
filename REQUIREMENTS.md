# Requirements

- Thousands of mixed, instanced objects, with a web selector to load a showcase and return to the default scene.
- Extensible geometry; no ray marching.
- Three intersecting ridged cones with genuine multiscale geometry, CPU/GPU queries and web creation. New cluster kernels use fixed direct analytic work: no iteration, recursion or traversal.
- Materials view with selectable brushed steel and red brickwork/mortar. Each begins with four repeating512×512 albedo/relief levels,10× apart; finer levels reveal grooves, pores and mineral grains, not miniature bricks. Support additional coarser/finer scales; visually check sphere, plane and Goursat up close through microscopic distances. Ground appearance in inspected references; keep height/colour fixed in object space during camera movement; preserve carrier geometry, picking and navigation.
- Intuitive add/duplicate/delete controls. Duplicate selected instances in place with fresh IDs, matching geometry/pose/material, selected copies and one-step undo. XYZ positions use numeric0.001 increments (10× finer), direct typing and wheel nudging.
- Selected geometry: centered XYZ translation arrows and between-axis rotation arcs, Unreal-style; Shift doubles either drag. Persist normalized quaternion poses without breaking legacy scenes/URLs; keep CPU/GPU queries, bounds, cutouts, normals, materials and shadows in the same instance frame.
- Modular geometry definitions, instances and query kernels, including closest-point queries with explicit accuracy and unresolved results.
- Free-camera WASD and drag look; wheel sets travel speed, held Shift temporarily doubles translation. Straight W eases within0.25 units (4× previous approach gain); reverse/strafe/diagonals keep base speed. Minimum speed/approach floor0.000001 units/s. Far-left vertical logarithmic slider: symbol-only min/max; sync wheel/keys/reset, Shift-expanded range and applied approach rate; idle shows available speed, without polling.
- Base ray queries always return a gap and matching surface witness with UV/normal; preserve explicit uncertainty and independent point queries.
- Texh rendered hits: one shared, geometry-independent direct calculation. NO LOOPS or recursion; always prefer fast approximation over accurate displacement. No root search, traversal, ray marching or fitted geometry. Sample height once, with a frozen footprint/frame, where the ray meets the median-height plane (brick face, not tops); filter shifted RGB/normals by the direct warp's pixel span; bound byte-height slope noise without weakening relief; orient bumps by the carrier side. Bound grazing offsets. No exact first-crossing or displaced-silhouette guarantee.
- Preserve carrier queries/picking, independent point queries and the existing miss rule: one bilinear height sample at the witness accepts signed height reaching the gap. No new gap solver.
- Scene → Leaf · relief and Red brick · relief: share texh, preserve matching source assets, cutouts, all four height bands and carrier cast-shadow behavior. Brick fine height gains are8× stronger; mortar and Leaf depth stay unchanged. Admit finer Leaf height/normal orders only when their bounded height exceeds¼pixel, fading fully over2× approach; preserve full close-up gains. Bound-cull irrelevant Leaf render/shadow work without losing selection outlines or changing raw queries. Break up visible repeats in the two finest Leaf bands without flattening them; manually approach/orbit both textures from normal through very close distances and several angles. Both localhost previews target60 warm FPS at1265×791; report measured limits.

## Changelog

| When | Who | What | Why |
| --- | --- | --- | --- |
| 2026-11-26 | User | Require natural close-up brick relief motion | Stop pores borrowing depth from spots ~1 mm away |
| 2026-11-25 | User | Require live logarithmic velocity slider at screen left | Show speed and symbolic limits without numbers or idle work |
| 2026-11-25 | User | Remove close-up Leaf/brick noise | Filter displaced pixels and byte-slope noise; stabilize bump orientation |
| 2026-11-25 | User | Delay bounded Leaf detail; reduce crowded close-up work | Avoid premature height sampling and GPU stalls |
| 2026-11-25 | User | Require camera-stable brickwork | Stop texture sliding during movement |
| 2026-11-25 | User | Require selected-object duplication | Reuse geometry with pose/material intact |
| 2026-11-25 | User | Require varied fine Leaf detail and approach/orbit QA | Remove visible micro-tiling and inspect moving close-ups |
| 2026-11-25 | User | Require translation/rotation gizmo with Shift2× | Edit selected geometry directly |
| 2026-11-25 | User | Require Shift boost, faster approach, stronger pores, smooth normals and precise wheelable positions | Improve close inspection and editing |
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
