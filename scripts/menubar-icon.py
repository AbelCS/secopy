"""Draws Secopy's menu bar icon: a 36×36 template PNG (black + alpha) of two overlapping squares."""
import struct, sys, zlib

S = 36
px = [[0] * S for _ in range(S)]

def outline(x0, y0, x1, y1, w=3):
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            edge = x - x0 < w or x1 - x < w or y - y0 < w or y1 - y < w
            corner = (x in (x0, x1)) and (y in (y0, y1))
            if edge and not corner:
                px[y][x] = 255

def clear(x0, y0, x1, y1):
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            px[y][x] = 0

outline(3, 3, 23, 23)
clear(13, 13, 33, 33)
outline(13, 13, 33, 33)

raw = b"".join(b"\x00" + b"".join(struct.pack("BBBB", 0, 0, 0, a) for a in row) for row in px)
def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", S, S, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
open(sys.argv[1], "wb").write(png)
