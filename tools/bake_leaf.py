# /// script
# dependencies = ["numpy==2.5.4", "pillow==13.0.0"]
# ///
"""Bake the documented leaf references. All explicit loops have constant bounds."""
from pathlib import Path
import json
import sys
import zlib

import numpy as np
from numpy.typing import NDArray
from PIL import Image, ImageFilter

Array = NDArray[np.float64]
EDGE = 512


def crop(source: Path, box: tuple[int, int, int, int], mode: str) -> Image.Image:
    with Image.open(source) as image:
        return image.convert(mode).crop(box)


def scalar(image: Image.Image) -> Array:
    return np.asarray(image, dtype=np.float64) / 255


def periodic(value: Array) -> Array:
    result = value.copy()
    for axis in range(2):
        weight = np.sin(np.pi * np.arange(EDGE) / EDGE) ** 2
        shape = [1] * result.ndim
        shape[axis] = EDGE
        w = weight.reshape(shape)
        result = w * result + (1 - w) * np.roll(result, EDGE // 2, axis)
    return result


def standardized(value: Array) -> Array:
    return (value - value.mean()) / max(float(value.std()), 1e-9)


def linear(value: Array) -> Array:
    return np.where(value <= 0.04045, value / 12.92, ((value + 0.055) / 1.055) ** 2.4)


def areoles(seed: int) -> Array:
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[:EDGE, :EDGE] / EDGE
    x = x + 0.024 * np.sin(2 * np.pi * (2 * y + x))
    y = y + 0.018 * np.sin(2 * np.pi * (3 * x - y))
    centers = (np.stack(np.meshgrid(np.arange(8), np.arange(8)), -1).reshape(64, 2)
               + rng.uniform(0.15, 0.85, (64, 2))) / 8

    def squared(index: int) -> Array:
        dx = (x - centers[index, 0] + 0.5) % 1 - 0.5
        dy = (y - centers[index, 1] + 0.5) % 1 - 0.5
        return dx * dx + dy * dy

    first, second = squared(0), squared(1)
    near, next_near = np.minimum(first, second), np.maximum(first, second)
    for index in range(2, 64):
        d = squared(index)
        next_near = np.minimum(next_near, np.maximum(near, d))
        near = np.minimum(near, d)
    return np.exp(-((next_near - near) / 0.00085) ** 2)


def pyramid(value: NDArray[np.uint8]) -> bytes:
    data = bytearray(value.tobytes())
    lo, hi = value, value
    for level in range(1, 10):
        side = EDGE >> level
        lo = lo.reshape(side, 2, side, 2).min(axis=(1, 3))
        hi = hi.reshape(side, 2, side, 2).max(axis=(1, 3))
        data.extend(np.stack((lo, hi), -1).tobytes())
    data.extend(bytes((-len(data)) % 4))
    return bytes(data)


def cup(mask: NDArray[np.uint8]) -> Array:
    """Warp the analytic cup and trace the photographed blade's vein layout."""
    row, col = np.mgrid[:EDGE, :EDGE]
    y, across = row / 256 - 1, col / 256 - 1 + 0.09375
    carrier = 4.375 * across ** 2 + 0.0625 * y ** 2 - 0.15
    relief = 0.009 * np.sin(5 * y) + across * (
        0.035 * np.sin(4 * y + 0.8) + 0.063 * across * np.sin(13 * y + 14 * across)
    )
    spine = np.interp(row, (95, 152, 214, 280, 346, 410, 475),
                      (229, 229, 227, 228, 230, 232, 230))
    offset = col - spine
    blade = np.clip((row - 95) / 380, 0, 1)
    relief += 0.045 * np.sin(np.pi * blade) * (1 - 0.45 * blade) * np.exp(
        -(offset / (4.8 - 2.3 * blade)) ** 2
    )
    # Root, signed span, rise in source pixels; left and right veins do not pair.
    for root, span, rise in (
        (125, -48, 56), (156, 72, 61), (167, -65, 62), (199, 89, 68),
        (207, -74, 75), (234, 94, 70), (246, -88, 74), (280, 92, 67),
        (287, -83, 75), (313, 81, 63), (325, -69, 73), (350, 65, 57),
        (366, -57, 69), (392, 46, 58), (403, -39, 53), (430, 31, 39),
    ):
        along = np.clip(offset / span, 0, 1)
        curve = root + rise * along * (0.65 + 0.35 * along)
        width = 3.0 - 1.35 * along
        ridge = 0.026 * np.sin(np.pi * along) * np.exp(-((row - curve) / width) ** 2)
        relief += ridge * (1 - 0.45 * along)
    outline = Image.fromarray(mask)
    # Let the ray seed set the delicate silhouette, not high-gradient veins.
    interior = np.clip((scalar(outline.filter(ImageFilter.GaussianBlur(4))) - 0.66) / 0.30, 0, 1)
    covered = np.asarray(outline.filter(ImageFilter.MaxFilter(9))) >= 128
    return np.clip(np.where(covered, carrier + interior * relief, 0) + 128 / 255, 0, 1)


def bake(references: Path, root: Path) -> None:
    source = references / "LeafSet001-8K"
    macro_box = (256, 128, 2560, 3936)
    middle_box = (1282, 1666, 1663, 2047)

    def image(channel: str, box: tuple[int, int, int, int], mode: str) -> Image.Image:
        return crop(source / f"LeafSet001_8K-JPG_{channel}.jpg", box, mode)

    color = image("Color", macro_box, "RGB").resize((310, EDGE), Image.Resampling.LANCZOS)
    canvas = Image.new("RGB", (EDGE, EDGE), (103, 112, 75))
    canvas.paste(color, (101, 0))
    base_rgb = linear(scalar(canvas.transpose(Image.Transpose.FLIP_TOP_BOTTOM)))
    opacity = image("Opacity", macro_box, "L").resize((310, EDGE), Image.Resampling.LANCZOS)
    mask_image = Image.new("L", (EDGE, EDGE))
    mask_image.paste(opacity, (101, 0))
    mask = np.asarray(mask_image.transpose(Image.Transpose.FLIP_TOP_BOTTOM), dtype=np.uint8)
    visible = mask >= 128
    y, x = np.mgrid[:EDGE, :EDGE] / EDGE

    mid_rgb = linear(scalar(image("Color", middle_box, "RGB").resize((EDGE, EDGE), Image.Resampling.LANCZOS)))
    mid_gray = scalar(image("Displacement", middle_box, "L").resize((EDGE, EDGE), Image.Resampling.LANCZOS)
                      .transpose(Image.Transpose.FLIP_TOP_BOTTOM).filter(ImageFilter.GaussianBlur(0.65)))
    middle_h = np.clip(0.5 + 0.085 * standardized(periodic(mid_gray)), 0.08, 0.92)
    network = areoles(611)
    fine_h = np.clip(0.47 + 0.31 * network + 0.025 * np.sin(2 * np.pi * (7 * x + 3 * y)), 0.05, 0.95)
    sem = crop(references / "pone.0238589.g002.png", (310, 110, 912, 712), "L")
    sem = sem.resize((EDGE, EDGE), Image.Resampling.LANCZOS).filter(ImageFilter.GaussianBlur(0.55))
    micro_h = np.clip(0.5 + 0.13 * standardized(periodic(scalar(sem))), 0.05, 0.95)
    mean = base_rgb[visible].mean(axis=0)
    tone = np.sin(2 * np.pi * (3 * x + 2 * y)) * np.sin(2 * np.pi * (y - x))
    heights = (cup(mask), middle_h, fine_h, micro_h)
    colors = (base_rgb, periodic(mid_rgb[::-1]), mean * (1 + 0.035 * network[..., None]),
              mean * (1 + 0.018 * tone[..., None]))
    tiles = root / "src/material/tiles"
    tiles.mkdir(parents=True, exist_ok=True)
    scalar_data = bytearray()
    preview = Image.new("RGB", (EDGE * 4, EDGE * 2))
    for order in range(4):
        height = np.rint(np.clip(heights[order], 0, 1) * 255).astype(np.uint8)
        rgb = np.rint(np.clip(colors[order], 0, 1) * 255).astype(np.uint8)
        rgba = np.dstack((rgb, height))
        (tiles / f"leaf-{order}.rgba.z").write_bytes(zlib.compress(rgba.tobytes(), 9))
        scalar_data.extend(pyramid(height))
        preview.paste(Image.fromarray(rgb), (order * EDGE, 0))
        preview.paste(Image.fromarray(height).convert("RGB"), (order * EDGE, EDGE))
    scalar_data.extend(pyramid(mask))
    destination = root / "src/heightfield/leaf.bin"
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(scalar_data)
    preview.save(references.parent / "leaf-tiles.jpg", quality=93)
    (references.parent / "leaf-assets.json").write_text(json.dumps({
        "bytes": len(scalar_data), "edge": EDGE, "fields": 5, "stride": len(scalar_data) // 5,
        "extent_m": [0.14, 0.014, 0.0014, 0.00014],
        "reference_half_size_m": 0.07, "macro_crop": macro_box, "middle_crop": middle_box,
        "sem_crop": [310, 110, 912, 712], "sem_scale_bar_px_per_50um": 215,
        "visible_fraction": float(visible.mean()), "anchor_mask": int(mask[256, 256]),
    }, indent=2))
    print(f"Baked four heights + separate cutout: {len(scalar_data)} bytes; anchor {mask[256,256]}")


if __name__ == "__main__":
    bake(Path(sys.argv[1]), Path(sys.argv[2]))
