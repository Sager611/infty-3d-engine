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
        return image.crop(box).convert(mode)


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


def macro_depth(source: Path, box: tuple[int, int, int, int], mask: NDArray[np.uint8]) -> Array:
    """Align the 16-bit displacement with RGB; taper the image matte to the ray seed."""
    depth = crop(source, box, "F").resize((310, EDGE), Image.Resampling.LANCZOS)
    raw = np.zeros((EDGE, EDGE), dtype=np.float64)
    raw[:, 101:411] = np.asarray(depth)[::-1]
    sampled = (raw - 18500) / 45000
    y, x = np.mgrid[:EDGE, :EDGE] / 256 - 1
    guide = x * (2.5 * x + 0.43 * y + 0.245) + y * (0.37 * y - 0.21) - 0.15
    outline = Image.fromarray((mask >= 128).astype(np.uint8) * 255)
    interior = np.clip((scalar(outline.filter(ImageFilter.GaussianBlur(2))) - 0.70) / 0.27, 0, 1)
    covered = np.asarray(outline.filter(ImageFilter.MaxFilter(9))) >= 128
    return np.clip(np.where(covered, guide + interior * (sampled - guide), 0) + 128 / 255, 0, 1)


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
    macro_h = macro_depth(references / "LeafSet001_8K_Displacement.png", macro_box, mask)
    heights = (macro_h, middle_h, fine_h, micro_h)
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
