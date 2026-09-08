#!/bin/bash
set -euo pipefail

# SBC-1E V2 bounded repair wrapper.
# Reuses the exact V1 evidence harness from the frozen candidate commit and
# injects only a temporary RGB->RGBA source-icon conversion for Apple builds.
SOURCE_HARNESS_COMMIT="90d3c04a72b178d1e12c4ce044aeb71682a6b23e"
REPO_ROOT="${CM_BUILD_DIR:-$(cd "$(dirname "$0")/.." && pwd)}"
SOURCE_PATH="ci/run_sbc1e_codemagic.sh"
TMP_HARNESS="$(mktemp /tmp/sbc1e_v1_harness.XXXXXX.sh)"

cleanup_wrapper() {
  rm -f "$TMP_HARNESS"
}
trap cleanup_wrapper EXIT

git -C "$REPO_ROOT" cat-file -e "${SOURCE_HARNESS_COMMIT}^{commit}"
git -C "$REPO_ROOT" show "${SOURCE_HARNESS_COMMIT}:${SOURCE_PATH}" > "$TMP_HARNESS"

python3 - "$TMP_HARNESS" <<'PYWRAP'
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text()

finish_needle = '''finish() {
  rc=$?
  set +e
'''
finish_replacement = '''finish() {
  rc=$?
  set +e
  if [ -n "${SBC1E_ICON_BACKUP:-}" ] && [ -f "${SBC1E_ICON_BACKUP:-}" ]; then
    cp "$SBC1E_ICON_BACKUP" "$TAURI_APP/icons/icon.png" 2>/dev/null || true
  fi
'''
if finish_needle not in text:
    raise SystemExit("SBC-1E wrapper: V1 finish() injection point not found")
text = text.replace(finish_needle, finish_replacement, 1)

e4_needle = '''# ---------- E4: physical iPhone/iPad target compile ----------
PHASE="E4_PHYSICAL_IOS_COMPILE"
'''
e4_replacement = r'''# ---------- E4: physical iPhone/iPad target compile ----------
PHASE="E4_PHYSICAL_IOS_COMPILE"

# V2 bounded repair: the frozen proof icon is an 8-bit RGB PNG. Tauri's Apple
# generate_context path requires RGBA. Convert it losslessly in the build
# workspace only, retain the original in the result bundle, and restore it in
# finish() so no product/source asset is changed by the proof run.
SBC1E_ICON_PATH="$TAURI_APP/icons/icon.png"
SBC1E_ICON_BACKUP="$RESULT_DIR/icon.original.png"
export SBC1E_ICON_BACKUP
cp "$SBC1E_ICON_PATH" "$SBC1E_ICON_BACKUP" || stop "Original source icon backed up before temporary RGBA conversion" "ICON_BACKUP_FAIL" 49
pass "Original source icon backed up before temporary RGBA conversion"

if python3 - "$SBC1E_ICON_PATH" >"$LOG_DIR/icon_rgba_conversion.stdout.txt" 2>"$LOG_DIR/icon_rgba_conversion.stderr.txt" <<'PYICON'
import binascii
import struct
import sys
import zlib
from pathlib import Path

path = Path(sys.argv[1])
data = path.read_bytes()
sig = b"\x89PNG\r\n\x1a\n"
if not data.startswith(sig):
    raise SystemExit("source icon is not PNG")

pos = len(sig)
idat = []
ihdr = None
while pos < len(data):
    if pos + 12 > len(data):
        raise SystemExit("truncated PNG chunk")
    n = struct.unpack(">I", data[pos:pos+4])[0]
    kind = data[pos+4:pos+8]
    payload = data[pos+8:pos+8+n]
    crc = data[pos+8+n:pos+12+n]
    if len(payload) != n or len(crc) != 4:
        raise SystemExit("truncated PNG payload")
    if binascii.crc32(kind + payload) & 0xffffffff != struct.unpack(">I", crc)[0]:
        raise SystemExit(f"bad CRC for {kind!r}")
    if kind == b"IHDR":
        ihdr = payload
    elif kind == b"IDAT":
        idat.append(payload)
    elif kind == b"IEND":
        break
    pos += 12 + n

if ihdr is None or not idat:
    raise SystemExit("PNG missing IHDR/IDAT")

w, h, bit_depth, colour_type, compression, filter_method, interlace = struct.unpack(">IIBBBBB", ihdr)
if (bit_depth, compression, filter_method, interlace) != (8, 0, 0, 0):
    raise SystemExit(
        f"unsupported PNG layout bit_depth={bit_depth} compression={compression} "
        f"filter={filter_method} interlace={interlace}"
    )

if colour_type == 6:
    print(f"already RGBA: {w}x{h}")
    raise SystemExit(0)
if colour_type != 2:
    raise SystemExit(f"expected RGB colour type 2, found {colour_type}")

raw = zlib.decompress(b"".join(idat))
bpp = 3
stride = w * bpp
expected = h * (stride + 1)
if len(raw) != expected:
    raise SystemExit(f"unexpected decompressed size {len(raw)} != {expected}")

def paeth(a, b, c):
    p = a + b - c
    pa = abs(p - a)
    pb = abs(p - b)
    pc = abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    if pb <= pc:
        return b
    return c

rgba_rows = bytearray()
prev = bytearray(stride)
off = 0
for _ in range(h):
    f = raw[off]
    scan = bytearray(raw[off + 1: off + 1 + stride])
    off += stride + 1
    recon = bytearray(stride)
    for x in range(stride):
        left = recon[x - bpp] if x >= bpp else 0
        up = prev[x]
        up_left = prev[x - bpp] if x >= bpp else 0
        if f == 0:
            val = scan[x]
        elif f == 1:
            val = (scan[x] + left) & 0xff
        elif f == 2:
            val = (scan[x] + up) & 0xff
        elif f == 3:
            val = (scan[x] + ((left + up) // 2)) & 0xff
        elif f == 4:
            val = (scan[x] + paeth(left, up, up_left)) & 0xff
        else:
            raise SystemExit(f"unsupported PNG filter {f}")
        recon[x] = val
    rgba_rows.append(0)
    for x in range(0, stride, 3):
        rgba_rows.extend((recon[x], recon[x+1], recon[x+2], 255))
    prev = recon

def png_chunk(kind, payload):
    return (
        struct.pack(">I", len(payload))
        + kind
        + payload
        + struct.pack(">I", binascii.crc32(kind + payload) & 0xffffffff)
    )

new_ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
out = (
    sig
    + png_chunk(b"IHDR", new_ihdr)
    + png_chunk(b"IDAT", zlib.compress(bytes(rgba_rows), 9))
    + png_chunk(b"IEND", b"")
)
path.write_bytes(out)

p = len(sig)
n = struct.unpack(">I", out[p:p+4])[0]
kind = out[p+4:p+8]
payload = out[p+8:p+8+n]
if kind != b"IHDR" or struct.unpack(">IIBBBBB", payload)[3] != 6:
    raise SystemExit("emitted PNG is not RGBA")
print(f"converted RGB -> RGBA losslessly: {w}x{h}, bytes={len(out)}")
PYICON
then
  pass "Temporary Apple build icon is valid RGBA PNG"
else
  stop "Temporary Apple build icon is valid RGBA PNG" "ICON_RGBA_CONVERSION_FAIL" 49
fi
shasum -a 256 "$SBC1E_ICON_BACKUP" > "$RESULT_DIR/icon.original.sha256"
shasum -a 256 "$SBC1E_ICON_PATH" > "$RESULT_DIR/icon.rgba.sha256"
'''
if e4_needle not in text:
    raise SystemExit("SBC-1E wrapper: V1 E4 injection point not found")
text = text.replace(e4_needle, e4_replacement, 1)

path.write_text(text)
PYWRAP

chmod +x "$TMP_HARNESS"
trap - EXIT
exec "$TMP_HARNESS"
