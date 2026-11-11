#!/usr/bin/env bash
# Regenerates every derived icon from the one master, assets/icon/telima.png (1024x1024).
# macOS only (sips, iconutil). The app id comes from crates/telima-ui/src/lib.rs.
set -euo pipefail
cd "$(dirname "$0")/.."
APP_ID=$(sed -n 's/^pub const APP_ID: &str = "\(.*\)";$/\1/p' crates/telima-ui/src/lib.rs)
[ -n "$APP_ID" ] || { echo "APP_ID not found in crates/telima-ui/src/lib.rs" >&2; exit 1; }
MASTER=assets/icon/telima.png
rm -rf assets/icon/hicolor
for s in 16 32 48 64 128 256 512; do
  d="assets/icon/hicolor/${s}x${s}/apps"
  mkdir -p "$d"
  sips -z "$s" "$s" "$MASTER" --out "$d/$APP_ID.png" >/dev/null
done
SET=$(mktemp -d)/telima.iconset
mkdir -p "$SET"
for s in 16 32 128 256 512; do
  sips -z "$s" "$s" "$MASTER" --out "$SET/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2))
  sips -z "$d" "$d" "$MASTER" --out "$SET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$SET" -o assets/icon/telima.icns
rm -rf "$(dirname "$SET")"

# Tray icons (canvas 11): the cube glyph plus a state badge, drawn in an 18x18
# design grid (stroke 1.5, round caps) and rasterised by plain python3 so the
# bytes are reproducible. macOS gets black template images (alpha only matters);
# Linux has no template tinting, so each size ships a dark glyph for light
# panels (-light, gray-900) and a light glyph for dark panels (-dark, gray-100).
rm -rf assets/icon/tray
python3 - assets/icon/tray <<'PY'
import struct, sys, zlib
from pathlib import Path

OUT = Path(sys.argv[1])
SUB = 12  # supersamples per axis
HALF = 0.75  # half the 1.5 stroke
HEX = [(7, 1.5), (12, 4.25), (12, 9.75), (7, 12.5), (2, 9.75), (2, 4.25)]
SEGMENTS = [(HEX[i], HEX[(i + 1) % 6]) for i in range(6)] + [
    ((2, 4.25), (7, 7)), ((7, 7), (12, 4.25)), ((7, 7), (7, 12.5))]
BX, BY = 14.5, 14.5  # badge centre


def near_segment(x, y, a, b):
    (ax, ay), (bx, by) = a, b
    dx, dy = bx - ax, by - ay
    t = max(0.0, min(1.0, ((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)))
    px, py = ax + t * dx - x, ay + t * dy - y
    return px * px + py * py <= HALF * HALF


def in_triangle(x, y, p):
    def side(a, b):
        return (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0])
    s = [side(p[i], p[(i + 1) % 3]) for i in range(3)]
    return all(v >= 0 for v in s) or all(v <= 0 for v in s)


def badge(state, x, y):
    r = ((x - BX) ** 2 + (y - BY) ** 2) ** 0.5
    ring = abs(r - 2.25) <= HALF
    if state == "stopped":
        return ring
    if state == "running":
        return r <= 2.75
    if state == "busy":
        return ring or (x >= BX and r <= 2.25)
    return in_triangle(x, y, [(14.5, 11.25), (17.5, 16.75), (11.5, 16.75)])


def inside(state, x, y):
    return badge(state, x, y) or any(near_segment(x, y, a, b) for a, b in SEGMENTS)


def coverage(state, size):
    k = 18.0 / size
    rows = []
    for py in range(size):
        row = []
        for px in range(size):
            hits = 0
            for j in range(SUB):
                y = (py + (j + 0.5) / SUB) * k
                for i in range(SUB):
                    if inside(state, (px + (i + 0.5) / SUB) * k, y):
                        hits += 1
            row.append(round(255 * hits / (SUB * SUB)))
        rows.append(row)
    return rows


def write_png(path, alpha, rgb):
    raw = b"".join(b"\0" + bytes(c for a in row for c in (*rgb, a)) for row in alpha)
    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data))
    size = len(alpha)
    png = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
           + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(png)


PANELS = {"light": (0x16, 0x16, 0x18), "dark": (0xEC, 0xEC, 0xEE)}
for state in ("stopped", "running", "busy", "error"):
    write_png(OUT / f"macos/{state}.png", coverage(state, 18), (0, 0, 0))
    write_png(OUT / f"macos/{state}@2x.png", coverage(state, 36), (0, 0, 0))
    for size in (22, 24):
        alpha = coverage(state, size)
        for panel, rgb in PANELS.items():
            write_png(OUT / f"linux/{size}/{state}-{panel}.png", alpha, rgb)
PY
echo "icons regenerated for $APP_ID"
