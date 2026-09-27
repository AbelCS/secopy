#!/usr/bin/env python3
"""Draws the placeholder app icon (1024x1024 PNG) with no dependencies: two cards, the
front one with a check mark. Regenerate the icon set with `npm run tauri icon` from ui/."""
import struct
import sys
import zlib

N = 1024
BG, BACK, FRONT, TICK = (17, 18, 20), (46, 48, 53), (79, 140, 255), (255, 255, 255)


def rounded(x, y, x0, y0, x1, y1, r):
    """Inside a rounded rectangle?"""
    if not (x0 <= x < x1 and y0 <= y < y1):
        return False
    cx = min(max(x, x0 + r), x1 - r)
    cy = min(max(y, y0 + r), y1 - r)
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r


def near_segment(x, y, ax, ay, bx, by, w):
    dx, dy = bx - ax, by - ay
    t = max(0.0, min(1.0, ((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)))
    px, py = ax + t * dx, ay + t * dy
    return (x - px) ** 2 + (y - py) ** 2 <= w * w


rows = []
for y in range(N):
    row = bytearray([0])
    for x in range(N):
        a, c = 0, BG
        if rounded(x, y, 100, 100, 924, 924, 185):
            a = 255
            if rounded(x, y, 250, 210, 700, 660, 70):
                c = BACK
            if rounded(x, y, 330, 360, 780, 810, 70):
                c = FRONT
                if near_segment(x, y, 440, 590, 530, 680, 34) or near_segment(x, y, 530, 680, 680, 490, 34):
                    c = TICK
        row += bytes(c) + bytes([a])
    rows.append(bytes(row))


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", N, N, 8, 6, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(b"".join(rows), 9)) + chunk(b"IEND", b"")
open(sys.argv[1] if len(sys.argv) > 1 else "icon-source.png", "wb").write(png)
