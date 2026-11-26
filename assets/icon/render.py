#!/usr/bin/env python3
"""Rasterize the Pandex icon (geometry of pandex.svg) with the stdlib only.

Two styles, same P-key geometry (stem = key shaft, bowl = key bow with a
keyhole cutout):
  white  (default) — current Proton Pass language: off-white rounded square,
         glyph in the violet->peach gradient (violet = theme primary #6D4AFF)
  purple — classic Proton family: purple diagonal gradient square, white glyph

Outputs:
  - assets/icon/pandex-512.png      full-bleed square (Play Store listing)
  - assets/icon/pandex.ico          Windows icon (16..256, rounded)
  - assets/icon/preview.png         light/dark contact sheet
  - apps/android/app/src/main/res/mipmap-*/ic_launcher.png (legacy, rounded)
  - .../mipmap-xxxhdpi/ic_launcher_{background,foreground}.png (adaptive)

The geometry constants below mirror pandex.svg; keep them in sync.
Run: python3 assets/icon/render.py [white|purple]
"""

import math
import os
import struct
import sys
import zlib

D = 512.0  # design space
SS = 2  # supersamples per axis per design pixel
R_BG = 116.0  # legacy corner radius
FG_SCALE = 1.05  # adaptive foreground glyph enlargement (fits the 66dp safe zone)
G_DY = 16.0  # optical centering: the wide bowl at the top needs more clearance
# than the narrow stem tip at the bottom, so the glyph sits below geometric
# center. Keep in sync with the translate() in pandex.svg.

PURPLE_TOP = (0x8F, 0x6B, 0xFF)
PURPLE_BOT = (0x5A, 0x31, 0xF4)
WHITE_BG = (0xF7, 0xF6, 0xFA)
GRAD_VIOLET = (0x6D, 0x4A, 0xFF)
GRAD_PEACH = (0xF2, 0xA9, 0x8A)

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
RES = os.path.join(ROOT, "apps", "android", "app", "src", "main", "res")
HERE = os.path.join(ROOT, "assets", "icon")


def sdf_rr(x, y, cx, cy, hw, hh, r):
    qx = abs(x - cx) - (hw - r)
    qy = abs(y - cy) - (hh - r)
    return math.hypot(max(qx, 0.0), max(qy, 0.0)) + min(max(qx, qy), 0.0) - r


def glyph(x, y):
    y -= G_DY
    stem = sdf_rr(x, y, 180.0, 258.0, 22.0, 130.0, 22.0) <= 0.0
    bow = math.hypot(x - 264.0, y - 210.0) <= 90.0
    if not (stem or bow):
        return False
    hole = math.hypot(x - 264.0, y - 196.0) <= 27.0
    if 206.0 <= y <= 262.0 and abs(x - 264.0) <= 13.0 + 7.0 * (y - 206.0) / 56.0:
        hole = True
    return not hole


def lerp(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def bg_color(style, x, y):
    if style == "white":
        return WHITE_BG
    return lerp(PURPLE_TOP, PURPLE_BOT, min(max((x + y) / (2.0 * D), 0.0), 1.0))


def fg_color(style, x, y):
    if style == "purple":
        return (255, 255, 255)
    # Projection on the SVG gradient axis (354,120)->(158,388), shifted with
    # the glyph: violet at its top-right, peach at its bottom-left.
    t = (196.0 * (354.0 - x) + 268.0 * (y - G_DY - 120.0)) / 110240.0
    return lerp(GRAD_VIOLET, GRAD_PEACH, min(max(t, 0.0), 1.0))


def render_coverage():
    n = int(D)
    cov_r = [0.0] * (n * n)
    cov_g = [0.0] * (n * n)
    cov_fg = [0.0] * (n * n)
    inv = 1.0 / SS
    for py in range(n):
        base = py * n
        for px in range(n):
            cr = cg = cf = 0
            for j in range(SS):
                y = py + (j + 0.5) * inv
                for i in range(SS):
                    x = px + (i + 0.5) * inv
                    if sdf_rr(x, y, 256.0, 256.0, 256.0, 256.0, R_BG) <= 0.0:
                        cr += 1
                    if glyph(x, y):
                        cg += 1
                    fx = (x - 256.0) / FG_SCALE + 256.0
                    fy = (y - 256.0) / FG_SCALE + 256.0
                    if glyph(fx, fy):
                        cf += 1
            k = base + px
            cov_r[k] = cr / (SS * SS)
            cov_g[k] = cg / (SS * SS)
            cov_fg[k] = cf / (SS * SS)
    return cov_r, cov_g, cov_fg


def resample(cov, t):
    s = D / t
    out = [0.0] * (t * t)
    n = int(D)
    for ty in range(t):
        y0 = ty * s
        y1 = y0 + s
        iy0 = int(y0)
        iy1 = min(int(math.ceil(y1)), n)
        for tx in range(t):
            x0 = tx * s
            x1 = x0 + s
            ix0 = int(x0)
            ix1 = min(int(math.ceil(x1)), n)
            tot = 0.0
            wsum = 0.0
            for iy in range(iy0, iy1):
                wy = min(y1, iy + 1) - max(y0, iy)
                row = iy * n
                for ix in range(ix0, ix1):
                    w = (min(x1, ix + 1) - max(x0, ix)) * wy
                    tot += cov[row + ix] * w
                    wsum += w
            out[ty * t + tx] = tot / wsum
    return out


def chunk(tag, data):
    return (
        struct.pack(">I", len(data))
        + tag
        + data
        + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    )


def png_from_pixels(t, px):
    raw = b"".join(
        b"\x00" + b"".join(struct.pack("4B", *p) for p in px[y * t : (y + 1) * t])
        for y in range(t)
    )
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", t, t, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def layer_pixels(t, alpha, style, layer):
    """Single adaptive layer: 'bg' (opaque shape color) or 'fg' (glyph)."""
    s = D / t
    px = []
    for ty in range(t):
        cy = (ty + 0.5) * s
        for tx in range(t):
            cx = (tx + 0.5) * s
            rgb = bg_color(style, cx, cy) if layer == "bg" else fg_color(style, cx, cy)
            px.append((rgb[0], rgb[1], rgb[2], round(alpha[ty * t + tx] * 255)))
    return px


def merged_pixels(t, ar, ag, style, circle=False):
    """Legacy icon: glyph over the background, rounded (or circle) alpha."""
    s = D / t
    px = []
    c = (t - 1) / 2.0
    r = t / 2.0
    for ty in range(t):
        cy = (ty + 0.5) * s
        for tx in range(t):
            cx = (tx + 0.5) * s
            g = ag[ty * t + tx]
            a = ar[ty * t + tx]
            if circle and math.hypot(tx - c, ty - c) > r:
                a = 0.0
            bg = bg_color(style, cx, cy)
            fg = fg_color(style, cx, cy)
            rgb = tuple(round(f * g + b * (1 - g)) for f, b in zip(fg, bg))
            px.append((rgb[0], rgb[1], rgb[2], round(a * 255)))
    return px


def write_pixels(path, t, px):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(png_from_pixels(t, px))
    print(f"wrote {os.path.relpath(path, ROOT)} ({t}x{t})")


def ico_bytes(entries):
    # entries: list of (size, png_bytes); PNG-compressed ICO (Vista+).
    header = struct.pack("<HHH", 0, 1, len(entries))
    offset = 6 + 16 * len(entries)
    directory = b""
    blob = b""
    for size, data in entries:
        b = 0 if size >= 256 else size
        directory += struct.pack("<BBBBHHII", b, b, 0, 0, 1, 32, len(data), offset)
        blob += data
        offset += len(data)
    return header + directory + blob


def composite(dst, dst_w, x, y, src, src_w):
    """Alpha-blit RGBA src onto an RGB canvas list."""
    for sy in range(src_w):
        dy = y + sy
        if dy < 0 or dy >= dst_w:
            continue
        for sx in range(src_w):
            dx = x + sx
            if dx < 0 or dx >= dst_w:
                continue
            r, g, b, a = src[sy * src_w + sx]
            if a == 0:
                continue
            k = dy * dst_w + dx
            if a == 255:
                dst[k] = (r, g, b)
            else:
                dr, dg, db = dst[k]
                f = a / 255.0
                dst[k] = (
                    round(r * f + dr * (1 - f)),
                    round(g * f + dg * (1 - f)),
                    round(b * f + db * (1 - f)),
                )


def main():
    style = sys.argv[1] if len(sys.argv) > 1 else "white"
    if style not in ("white", "purple"):
        raise SystemExit("style must be white or purple")
    cov_r, cov_g, cov_fg = render_coverage()
    one = [1.0] * (int(D) * int(D))

    # Play Store listing icon: full-bleed square, glyph merged.
    write_pixels(
        os.path.join(HERE, "pandex-512.png"),
        512,
        merged_pixels(512, resample(one, 512), resample(cov_g, 512), style),
    )

    # Android legacy launcher icons (rounded, glyph merged) + adaptive layers.
    for dpi, size in [("mdpi", 48), ("hdpi", 72), ("xhdpi", 96), ("xxhdpi", 144), ("xxxhdpi", 192)]:
        write_pixels(
            os.path.join(RES, f"mipmap-{dpi}", "ic_launcher.png"),
            size,
            merged_pixels(size, resample(cov_r, size), resample(cov_g, size), style),
        )
    write_pixels(
        os.path.join(RES, "mipmap-xxxhdpi", "ic_launcher_background.png"),
        432,
        layer_pixels(432, resample(one, 432), style, "bg"),
    )
    write_pixels(
        os.path.join(RES, "mipmap-xxxhdpi", "ic_launcher_foreground.png"),
        432,
        layer_pixels(432, resample(cov_fg, 432), style, "fg"),
    )

    # Windows .ico (NSH 0.6.1 cannot embed it into the exe yet; kept for the
    # installer / future NSH support).
    sizes = [16, 24, 32, 48, 64, 128, 256]
    entries = [
        (s, png_from_pixels(s, merged_pixels(s, resample(cov_r, s), resample(cov_g, s), style)))
        for s in sizes
    ]
    with open(os.path.join(HERE, "pandex.ico"), "wb") as f:
        f.write(ico_bytes(entries))
    print("wrote assets/icon/pandex.ico")

    # Contact sheet: light and dark panels, legacy sizes + adaptive square and
    # circle previews (as Android launchers crop it).
    sheet_w, panel_h = 1120, 280
    light = (0xDD, 0xDA, 0xE6) if style == "white" else (0xF4, 0xF3, 0xF8)
    sheet = [light] * (sheet_w * panel_h) + [(0x16, 0x14, 0x1F)] * (sheet_w * panel_h)
    icons = []
    for size in (192, 96, 72, 48):
        icons.append((size, merged_pixels(size, resample(cov_r, size), resample(cov_g, size), style)))
    bg432 = layer_pixels(432, resample(one, 432), style, "bg")
    fg432 = layer_pixels(432, resample(cov_fg, 432), style, "fg")
    adaptive = [(0, 0, 0)] * (432 * 432)
    composite(adaptive, 432, 0, 0, bg432, 432)
    composite(adaptive, 432, 0, 0, fg432, 432)
    s = 432 / 192
    apx = []
    for ty in range(192):
        for tx in range(192):
            sx, sy = int((tx + 0.5) * s), int((ty + 0.5) * s)
            r, g, b = adaptive[sy * 432 + sx]
            apx.append((r, g, b, 255))
    icons.append((192, apx))
    icons.append((192, merged_pixels(192, resample(cov_r, 192), resample(cov_g, 192), style, circle=True)))
    for panel in range(2):
        x = 48
        y = panel * panel_h + (panel_h - 192) // 2
        for size, px in icons:
            composite(sheet, sheet_w, x, y + (192 - size) // 2, px, size)
            x += size + 40
    raw = b"".join(
        b"\x00" + b"".join(struct.pack("3B", *sheet[y * sheet_w + x]) for x in range(sheet_w))
        for y in range(2 * panel_h)
    )
    data = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", sheet_w, 2 * panel_h, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )
    with open(os.path.join(HERE, "preview.png"), "wb") as f:
        f.write(data)
    print(f"wrote assets/icon/preview.png ({style})")


if __name__ == "__main__":
    main()
