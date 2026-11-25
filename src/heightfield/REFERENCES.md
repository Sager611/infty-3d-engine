# Finite leaf

Try Scene controls → Scene → Leaf · relief; Add → Finite plane reuses the carrier.
Axis0/1/2 means normalZ/X/Y. Kind5 is a double-sided square with a Leaf cutout.
Leaf and brick share direct material-hit math; geometry supplies carrier-frame mapping.
At reference half-size70mm, four vertex-aligned512² height fields span140/14/1.4/0.14mm.
The macro field clamps; finer fields repeat. Raw bytes are centered at128, divided by255,
and multiplied by gains2^-3/2^-10/2^-13/2^-15 before summation.
A separate512² opacity field clips coverage at127.5. It is not height or optical albedo.
Uniform instance size scales the entire surface; peers still share their carrier definition.

## Geometry and rendering

Carrier queries and picking stay on the base surface, with the Leaf cutout. The existing signed-height miss rule is unchanged; no new gap solver is provided.
Rendered hits use one carrier-height evaluation and the direct local offset h·c/max(c²,0.0025), where c=N·ray. Below |c|=0.05, relief fades instead of producing an unbounded grazing slide. Shifted UV supplies RGB/cutout. Normals blend vertex slopes at shifted UVs: four fixed samples per band, a texel-sized minimum stencil, per-band footprint fading and the carrier UV Jacobian. Carrier frame/footprint stay frozen.
All four bilinear bands, RGB, cutout, carrier cast shadows and the Leaf self-shadow exception remain. No asset is rebaked. Shader compilation expands level metadata into straight-line sample calls; displacement/shading have no loops, recursion or search.
This deliberately favors speed over accuracy: no exact first crossing, normal-chart inversion, displaced silhouette or certified relief error. Strong slopes, grazing views and close-ups can distort or miss relief; an uncovered/backward candidate is discarded without searching again.
Point queries still use the height pyramid and covered witnesses independently.
Amber hatching marks unresolved ray or shadow work.
The finest reference texel is273.4375nm; below this there is bilinear interpolation, not new evidence.
Extreme close-ups expose texture cells.
The leaf is an authored opaque sheet, not measured anatomy, transmission, thickness, or volumetric tissue.

## Sources and changes

- [ambientCG LeafSet001](https://ambientcg.com/a/LeafSet001):
  colour/displacement/opacity8K-JPG, [CC0](https://docs.ambientcg.com/license/).
  RGB and the [matching 16-bit displacement](https://commons.wikimedia.org/wiki/File:LeafSet001_8K_Displacement.png) share crop(256,128,2560,3936), resampling and flip. RGB uses linear light; depth uses (sample−18500)/45000 before gain. This is relative displacement, not a metrically calibrated scan.
  Middle crop(1282,1666,1663,2047).
- William J. Matthaeus, Jonathan Schmidt, Joseph D. White, Bernd Zechmann (2020),
  [Novel perspectives on stomatal impressions: Rapid and non-invasive surface characterization
  of plant leaves by scanning electron microscopy](https://doi.org/10.1371/journal.pone.0238589),
  Fig2A, [CC BY4.0](https://creativecommons.org/licenses/by/4.0/).
  [Original figure](https://journals.plos.org/plosone/article/figure/image?size=large&id=10.1371/journal.pone.0238589.g002).
  This is chemically fixed, critical-point-dried, sputter-coated adult *Nicotiana benthamiana*,
  not the replicas in panelsC–F. Crop(310,110,912,712), resized/blurred/periodized/contrast-remapped.
  Its50µm bar is approximately215px; the602px crop informs the nominal140µm order.
  SEM intensity is neither calibrated height nor optical colour. Optical green supplies fine RGB.
- Seed611 supplies authored periodic areoles; finer maps are not a common-specimen/species-specific reconstruction.
  All height gains, colour modulation, filtering, roughness, and lighting are artistic.

## Reproduction

Extract `LeafSet001_8K-JPG.zip` under `references/LeafSet001-8K/`; save the full-resolution displacement and SEM figure as `references/LeafSet001_8K_Displacement.png` and `references/pone.0238589.g002.png`.
Run `uv run --no-project tools/bake_leaf.py /path/to/references /path/to/checkout`.
Dependencies are pinned in the script; references/previews stay outside the repository.
Outputs: four linear RGB8/height-alpha zlib tiles and `leaf.bin` (2,184,540 bytes).
Each scalar field has262,144 samples, nine interleaved min/max levels, and padding:436,908 bytes.
GPU upload: 32-byte header plus all five raw planes and min/max pyramids.
Layout, covered positive-zero anchor, and exact finite f32 gains are checked; no upload resampling.
The retained macro asset blends source depth into offline polynomial matte padding near the cutout to suppress halos. This is baked map data, not an intersection model; this resolver change does not rebake any asset.

## Changelog

| When | Who | What | Why |
| --- | --- | --- | --- |
| 2026-11-25 | Assistant | Smooth each height band's vertex slopes at shifted UVs | Remove blocky close-up highlights without changing depth |
| 2026-11-24 | Assistant | Document shared direct approximation, fixed samples and unchanged assets | Favor speed over iterative displacement accuracy |
| 2026-11-11 | Assistant | Documented Leaf usage, fixed surface, source derivatives and bake | Make geometry, limits and attribution explicit |
| 2026-11-24 | Assistant | Documented texh ray approximation | Separate ray, point, and shading rules |
| 2026-11-24 | Assistant | Documented cupped heights and color-only hits | Keep height out of the hit path |
| 2026-11-24 | Assistant | Documented analytic seed and height-corrected hits | Render depth without traversal |
| 2026-11-25 | Assistant | Described blade relief and alternate roots | Preserve oblique cutouts and light the veins |
| 2026-11-24 | Assistant | Scale guide and macro heights together | Keep the curl shallow |
| 2026-11-24 | Assistant | Use matched 16-bit displacement and fit the seed | Replace synthetic folds with the source profile |
