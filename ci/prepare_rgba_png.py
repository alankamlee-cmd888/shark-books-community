#!/usr/bin/env python3
"""Convert an 8-bit non-interlaced RGB PNG to RGBA for Apple CI, in place."""
from __future__ import annotations

import binascii
import struct
import sys
import zlib
from pathlib import Path

PNG = b"\x89PNG\r\n\x1a\n"


def chunks(data: bytes):
    pos = len(PNG)
    while pos < len(data):
        length = struct.unpack(">I", data[pos : pos + 4])[0]
        kind = data[pos + 4 : pos + 8]
        payload = data[pos + 8 : pos + 8 + length]
        crc = struct.unpack(">I", data[pos + 8 + length : pos + 12 + length])[0]
        if binascii.crc32(kind + payload) & 0xFFFFFFFF != crc:
            raise ValueError(f"CRC mismatch in {kind!r}")
        yield kind, payload
        pos += 12 + length


def paeth(a: int, b: int, c: int) -> int:
    p = a + b - c
    pa = abs(p - a)
    pb = abs(p - b)
    pc = abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    if pb <= pc:
        return b
    return c


def unfilter(raw: bytes, width: int, height: int, bpp: int) -> list[bytes]:
    stride = width * bpp
    rows: list[bytes] = []
    pos = 0
    prior = bytearray(stride)
    for _ in range(height):
        filter_type = raw[pos]
        pos += 1
        encoded = raw[pos : pos + stride]
        pos += stride
        row = bytearray(stride)
        for i, value in enumerate(encoded):
            left = row[i - bpp] if i >= bpp else 0
            up = prior[i]
            up_left = prior[i - bpp] if i >= bpp else 0
            if filter_type == 0:
                decoded = value
            elif filter_type == 1:
                decoded = (value + left) & 0xFF
            elif filter_type == 2:
                decoded = (value + up) & 0xFF
            elif filter_type == 3:
                decoded = (value + ((left + up) // 2)) & 0xFF
            elif filter_type == 4:
                decoded = (value + paeth(left, up, up_left)) & 0xFF
            else:
                raise ValueError(f"unsupported PNG filter {filter_type}")
            row[i] = decoded
        rows.append(bytes(row))
        prior = row
    if pos != len(raw):
        raise ValueError("unexpected decompressed PNG length")
    return rows


def chunk(kind: bytes, payload: bytes) -> bytes:
    return (
        struct.pack(">I", len(payload))
        + kind
        + payload
        + struct.pack(">I", binascii.crc32(kind + payload) & 0xFFFFFFFF)
    )


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: prepare_rgba_png.py <png>")
    path = Path(sys.argv[1])
    data = path.read_bytes()
    if not data.startswith(PNG):
        raise ValueError("not a PNG")

    parsed = list(chunks(data))
    ihdr = next(payload for kind, payload in parsed if kind == b"IHDR")
    width, height, depth, colour, compression, filtering, interlace = struct.unpack(
        ">IIBBBBB", ihdr
    )
    if colour == 6:
        print("PNG already RGBA")
        return 0
    if (depth, colour, compression, filtering, interlace) != (8, 2, 0, 0, 0):
        raise ValueError(
            f"expected 8-bit non-interlaced RGB PNG; got depth={depth} colour={colour} interlace={interlace}"
        )

    compressed = b"".join(payload for kind, payload in parsed if kind == b"IDAT")
    rows = unfilter(zlib.decompress(compressed), width, height, 3)
    rgba_raw = bytearray()
    for row in rows:
        rgba_raw.append(0)
        for i in range(0, len(row), 3):
            rgba_raw.extend(row[i : i + 3])
            rgba_raw.append(255)

    new_ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    out = bytearray(PNG)
    out += chunk(b"IHDR", new_ihdr)
    for kind, payload in parsed:
        if kind in {b"IHDR", b"IDAT", b"IEND"}:
            continue
        out += chunk(kind, payload)
    out += chunk(b"IDAT", zlib.compress(bytes(rgba_raw), 9))
    out += chunk(b"IEND", b"")
    path.write_bytes(out)
    print(f"converted {path} to RGBA {width}x{height}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
