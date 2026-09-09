#!/usr/bin/env python3
"""Generate deterministic synthetic UK-style receipt fixtures for SBC-6A."""
from __future__ import annotations

import argparse
import json
import math
import random
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageEnhance, ImageFilter, ImageFont

SEED = 608072026
WIDTH = 1100
MARGIN = 70

RECEIPTS = [
    {
        "id": "r01",
        "merchant": "NORTH PIER STATIONERS",
        "date": "04/04/2026",
        "reference": "NP-260404-1842",
        "items": [("A4 COPY PAPER", "8.99"), ("BLACK INK", "19.50")],
        "total": "28.49",
    },
    {
        "id": "r02",
        "merchant": "WYRE TOOL SUPPLIES",
        "date": "06/04/2026",
        "reference": "WTS-004981",
        "items": [("DRILL BITS", "14.75"), ("WORK GLOVES", "6.20"), ("TAPE", "3.05")],
        "total": "24.00",
    },
    {
        "id": "r03",
        "merchant": "FYLDE FUEL AND SERVICE",
        "date": "19/05/2026",
        "reference": "FFS-190526-77",
        "items": [("UNLEADED", "67.43")],
        "total": "67.43",
    },
    {
        "id": "r04",
        "merchant": "LANCASTER PACKAGING CO",
        "date": "31/08/2026",
        "reference": "LP-83106",
        "items": [("MAILERS", "42.00"), ("LABEL ROLLS", "18.35")],
        "total": "60.35",
    },
    {
        "id": "r05",
        "merchant": "SEASIDE ELECTRICAL WHOLESALE",
        "date": "01/09/2026",
        "reference": "SEW-0007421",
        "items": [("CABLE REEL", "128.40"), ("CONNECTORS", "36.75"), ("DELIVERY", "9.99")],
        "total": "175.14",
    },
    {
        "id": "r06",
        "merchant": "BLACKPOOL BUSINESS EQUIPMENT",
        "date": "09/09/2026",
        "reference": "BBE-992104",
        "items": [("OFFICE CHAIR", "899.99"), ("DESK LAMP", "74.50"), ("CABLE TRAY", "25.50")],
        "total": "999.99",
    },
]

CONDITIONS = (
    "clean",
    "rotation",
    "skew",
    "blur",
    "shadow",
    "low_contrast",
    "thermal_fade",
    "long_noisy",
)


def money_to_pence(value: str) -> int:
    whole, frac = value.split(".")
    return int(whole) * 100 + int(frac)


def font(size: int, *, mono: bool = False) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    names = ["DejaVuSansMono.ttf", "DejaVuSans.ttf"] if mono else ["DejaVuSans.ttf", "DejaVuSansMono.ttf"]
    for name in names:
        try:
            return ImageFont.truetype(name, size=size)
        except OSError:
            pass
    # Pillow 12 supports a scalable built-in default font.
    try:
        return ImageFont.load_default(size=size)
    except TypeError:
        return ImageFont.load_default()


def render_receipt(spec: dict[str, object]) -> Image.Image:
    items = spec["items"]
    assert isinstance(items, list)
    height = 620 + 70 * len(items)
    image = Image.new("L", (WIDTH, height), 248)
    draw = ImageDraw.Draw(image)
    title_font = font(46)
    body_font = font(34, mono=True)
    small_font = font(28, mono=True)

    y = 55
    draw.text((MARGIN, y), str(spec["merchant"]), fill=15, font=title_font)
    y += 86
    draw.line((MARGIN, y, WIDTH - MARGIN, y), fill=80, width=3)
    y += 35
    draw.text((MARGIN, y), f"DATE: {spec['date']}", fill=25, font=body_font)
    y += 55
    draw.text((MARGIN, y), f"RECEIPT: {spec['reference']}", fill=25, font=body_font)
    y += 72
    for label, amount in items:
        draw.text((MARGIN, y), str(label), fill=30, font=small_font)
        right = f"GBP {amount}"
        box = draw.textbbox((0, 0), right, font=small_font)
        draw.text((WIDTH - MARGIN - (box[2] - box[0]), y), right, fill=30, font=small_font)
        y += 62
    draw.line((MARGIN, y, WIDTH - MARGIN, y), fill=80, width=3)
    y += 30
    total_text = f"TOTAL GBP {spec['total']}"
    box = draw.textbbox((0, 0), total_text, font=title_font)
    draw.text((WIDTH - MARGIN - (box[2] - box[0]), y), total_text, fill=5, font=title_font)
    y += 85
    draw.text((MARGIN, y), "THANK YOU", fill=70, font=small_font)
    return image


def add_shadow(image: Image.Image) -> Image.Image:
    w, h = image.size
    mask = Image.new("L", image.size, 255)
    px = mask.load()
    for y in range(h):
        for x in range(w):
            dx = (x - 0.78 * w) / (0.65 * w)
            dy = (y - 0.32 * h) / (0.70 * h)
            d = math.sqrt(dx * dx + dy * dy)
            shade = max(0.50, min(1.0, 0.58 + 0.55 * d))
            px[x, y] = int(255 * shade)
    return ImageChops.multiply(image, mask)


def add_thermal_fade(image: Image.Image, rng: random.Random) -> Image.Image:
    faded = ImageEnhance.Contrast(image).enhance(0.56)
    faded = ImageEnhance.Brightness(faded).enhance(1.10)
    draw = ImageDraw.Draw(faded)
    for _ in range(18):
        y = rng.randrange(0, faded.height)
        width = rng.randrange(1, 5)
        tone = rng.randrange(220, 248)
        draw.line((0, y, faded.width, y), fill=tone, width=width)
    return faded


def add_long_noise(image: Image.Image, rng: random.Random) -> Image.Image:
    extra = 480
    canvas = Image.new("L", (image.width, image.height + extra), 246)
    canvas.paste(image, (0, 100))
    draw = ImageDraw.Draw(canvas)
    # Light fold/wrinkle lines and speckle. They are deliberately not allowed to alter truth.
    for _ in range(11):
        y = rng.randrange(30, canvas.height - 30)
        tone = rng.randrange(185, 235)
        draw.line((rng.randrange(0, 120), y, rng.randrange(canvas.width - 120, canvas.width), y + rng.randrange(-15, 16)), fill=tone, width=rng.randrange(1, 4))
    for _ in range(1800):
        x = rng.randrange(0, canvas.width)
        y = rng.randrange(0, canvas.height)
        if rng.random() < 0.5:
            draw.point((x, y), fill=rng.randrange(175, 235))
    return canvas.filter(ImageFilter.GaussianBlur(radius=0.35))


def transform(image: Image.Image, condition: str, rng: random.Random) -> Image.Image:
    if condition == "clean":
        return image.copy()
    if condition == "rotation":
        return image.rotate(2.7, resample=Image.Resampling.BICUBIC, expand=True, fillcolor=250)
    if condition == "skew":
        # Bounded affine skew is a deterministic proxy for a mild oblique phone photo.
        return image.transform(
            (image.width + 90, image.height + 50),
            Image.Transform.AFFINE,
            (1.0, -0.065, 28.0, 0.025, 1.0, -4.0),
            resample=Image.Resampling.BICUBIC,
            fillcolor=250,
        )
    if condition == "blur":
        return image.filter(ImageFilter.GaussianBlur(radius=1.65))
    if condition == "shadow":
        return add_shadow(image)
    if condition == "low_contrast":
        return ImageEnhance.Contrast(ImageEnhance.Brightness(image).enhance(1.04)).enhance(0.43)
    if condition == "thermal_fade":
        return add_thermal_fade(image.copy(), rng)
    if condition == "long_noisy":
        return add_long_noise(image.copy(), rng)
    raise ValueError(f"unknown condition: {condition}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    output = Path(args.output).resolve()
    images = output / "images"
    images.mkdir(parents=True, exist_ok=True)
    rng = random.Random(SEED)
    manifest: dict[str, object] = {
        "schema": "sbc6a.synthetic_receipts.v1",
        "seed": SEED,
        "count": 0,
        "conditions": list(CONDITIONS),
        "fixtures": [],
    }

    fixtures: list[dict[str, object]] = []
    for receipt in RECEIPTS:
        base = render_receipt(receipt)
        for condition in CONDITIONS:
            derived_rng = random.Random(rng.randrange(0, 2**31 - 1))
            img = transform(base, condition, derived_rng)
            filename = f"{receipt['id']}__{condition}.png"
            img.save(images / filename, format="PNG", optimize=False)
            fixtures.append(
                {
                    "fixture_id": f"{receipt['id']}::{condition}",
                    "image": f"images/{filename}",
                    "condition": condition,
                    "truth": {
                        "merchant": receipt["merchant"],
                        "date": receipt["date"],
                        "total_pence": money_to_pence(str(receipt["total"])),
                        "currency": "GBP",
                        "reference": receipt["reference"],
                    },
                }
            )

    manifest["fixtures"] = fixtures
    manifest["count"] = len(fixtures)
    if len(fixtures) != 48:
        raise RuntimeError(f"fixture count drift: {len(fixtures)}")
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"SBC6A_SYNTHETIC_RECEIPTS_READY count={len(fixtures)} path={output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
