# Finite leaf

Try Scene controls → Scene → Leaf · deep cup; Add → Finite plane reuses the carrier.
Axis0/1/2 means normalZ/X/Y. Kind5 is a double-sided square with a Leaf cutout.
Leaf relief can be applied to any geometry; steel/brick leave intersections unchanged.
At reference half-size70mm, four vertex-aligned512² height fields span140/14/1.4/0.14mm.
The macro field clamps; finer fields repeat. Raw bytes are centered at128, divided by255,
and multiplied by canonical gains1/2^-10/2^-13/2^-15 before summation.
A separate512² opacity field clips coverage at127.5. It is not height or optical albedo.
Uniform instance size scales the entire surface; peers still share their carrier definition.

## Geometry and rendering

Other carriers keep their hits; a miss is promoted only if the signed bilinear height at its witness reaches the ray.
For the finite Leaf, an analytic cup seeds a candidate UV. A single bilinear height evaluation corrects ray depth; opacity clips the projected UV.
The cup provides smooth geometric normals. Colors are unchanged; Leaf skips self-shadowing.
Fine relief away from the witness is not searched. Grazing corrections are bounded; this is not an exact heightfield intersection.
Point queries still use the height pyramid and covered witnesses independently.
Amber hatching marks unresolved ray or shadow work.
The finest reference texel is273.4375nm; below this there is bilinear interpolation, not new evidence.
Extreme close-ups expose texture cells.
The leaf is an authored opaque sheet, not measured anatomy, transmission, thickness, or volumetric tissue.

## Sources and changes

- [ambientCG LeafSet001](https://ambientcg.com/a/LeafSet001):
  colour/displacement/opacity8K-JPG, [CC0](https://docs.ambientcg.com/license/).
  Optical RGB is cropped, resized, flipped, and converted to linear light. The macro shape is authored.
  Macro crop(256,128,2560,3936); middle crop(1282,1666,1663,2047).
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

Download/extract `LeafSet001_8K-JPG.zip` under `references/LeafSet001-8K/`;
save the cited full-resolution figure as `references/pone.0238589.g002.png`.
Run `uv run --no-project tools/bake_leaf.py /path/to/references /path/to/checkout`.
Dependencies are pinned in the script; references/previews stay outside the repository.
Outputs: four linear RGB8/height-alpha zlib tiles and `leaf.bin` (2,184,540 bytes).
Each scalar field has262,144 samples, nine interleaved min/max levels, and padding:436,908 bytes.
GPU upload omits pyramids: 32-byte header plus five raw planes.
Layout, covered positive-zero anchor, and exact finite f32 gains are checked; no upload resampling.
The macro field quantizes the same quadratic cup used to seed rays.

## Changelog

| When | Who | What | Why |
| --- | --- | --- | --- |
| 2026-11-11 | Assistant | Documented Leaf usage, fixed surface, source derivatives and bake | Make geometry, limits and attribution explicit |
| 2026-11-24 | Assistant | Documented texh ray approximation | Separate ray, point, and shading rules |
| 2026-11-24 | Assistant | Documented cupped heights and color-only hits | Keep height out of the hit path |
| 2026-11-24 | Assistant | Documented analytic seed and height-corrected hits | Render depth without traversal |
