"""Генерация исходной иконки VoiceBridge (1024x1024) без внешних зависимостей."""

import math
import struct
import zlib

SIZE = 1024
R = 184  # радиус скругления фона
MARGIN = 24  # отступ фона от краёв (для скругления углов)

# Цвета фона (вертикальный градиент)
BG_TOP = (27, 38, 55)
BG_BOTTOM = (10, 15, 23)

# Цвета волны (вертикальный градиент, циан)
BAR_TOP = (103, 232, 249)   # #67e8f9
BAR_BOTTOM = (14, 165, 233)  # #0ea5e9

BARS = [
    (0.40, 0.30),
    (0.80, 0.28),
    (0.55, 0.26),
    (1.00, 0.34),
    (0.55, 0.26),
    (0.80, 0.28),
    (0.40, 0.30),
]
GAP = 20


def smoothstep(e0, e1, x):
    t = max(0.0, min(1.0, (x - e0) / (e1 - e0)))
    return t * t * (3.0 - 2.0 * t)


def rounded_rect_sd(px, py, cx, cy, hw, hh, r):
    dx = abs(px - cx) - (hw - r)
    dy = abs(py - cy) - (hh - r)
    ax = max(dx, 0.0)
    ay = max(dy, 0.0)
    return math.hypot(ax, ay) + min(max(dx, dy), 0.0) - r


def lerp3(a, b, t):
    return tuple(a[i] + (b[i] - a[i]) * t for i in range(3))


def build_pixels():
    cx = cy = SIZE / 2
    hw = hh = SIZE / 2 - MARGIN

    # Ширины и позиции столбцов
    total_gap = (len(BARS) - 1) * GAP
    bar_w = int((SIZE - 320 - total_gap) / len(BARS))
    widths = [bar_w] * len(BARS)
    positions = []
    x = cx - (sum(widths) + total_gap) / 2
    for w in widths:
        positions.append(x + w / 2)
        x += w + GAP

    max_half = 240.0

    pixels = bytearray()
    for py in range(SIZE):
        pixels.append(0)  # filter type: None
        for px in range(SIZE):
            # Фон
            sd_bg = rounded_rect_sd(px + 0.5, py + 0.5, cx, cy, hw, hh, R)
            alpha_bg = 1.0 - smoothstep(-1.0, 1.0, sd_bg)
            if alpha_bg <= 0.0:
                pixels += b"\x00\x00\x00\x00"
                continue

            t_bg = py / SIZE
            col = lerp3(BG_TOP, BG_BOTTOM, t_bg)

            # Столбцы волны
            for i, (amp, rel_w) in enumerate(BARS):
                bx = positions[i]
                bw = widths[i] * (0.75 + rel_w)
                bh = max_half * amp
                sd_bar = rounded_rect_sd(px + 0.5, py + 0.5, bx, cy, bw / 2, bh, bw / 2)
                cov = 1.0 - smoothstep(-1.0, 1.0, sd_bar)
                if cov > 0.0:
                    t_bar = (py - (cy - bh)) / (2 * bh)
                    t_bar = max(0.0, min(1.0, t_bar))
                    bar_col = lerp3(BAR_TOP, BAR_BOTTOM, t_bar)
                    col = tuple(col[j] * (1 - cov) + bar_col[j] * cov for j in range(3))

            r, g, b = (int(round(c)) for c in col)
            a = int(round(alpha_bg * 255))
            pixels += bytes((r, g, b, a))

    return pixels


def write_png(path, size, raw_pixels):
    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        c += struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        return c

    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    idat = zlib.compress(bytes(raw_pixels), 9)
    with open(path, "wb") as f:
        f.write(sig)
        f.write(chunk(b"IHDR", ihdr))
        f.write(chunk(b"IDAT", idat))
        f.write(chunk(b"IEND", b""))


if __name__ == "__main__":
    import sys

    if "--fullbleed" in sys.argv:
        # Без скругления, отступов и прозрачных углов (для мобильных иконок).
        R = 0
        MARGIN = 0
        out = "app-icon-fullbleed.png"
    else:
        out = "app-icon.png"
    write_png(out, SIZE, build_pixels())
    print(f"{out} written")
