#!/usr/bin/env python3
"""CC0 authored full-canvas opaque MNG-VLC fixtures, using Python/zlib PNG encoding.
Not an external MNG decoder oracle. Regenerates tests/fixtures/mng exactly.
"""
import hashlib
import json
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).resolve().parents[1] / "tests/fixtures/mng"
CASES = [
    ("rgb8-two-frames", 8, 2, [
        [255, 0, 0, 0, 255, 0, 0, 0, 255, 12, 34, 56, 78, 90, 123, 255, 255, 255],
        [0, 0, 0, 255, 255, 0, 0, 255, 255, 255, 0, 255, 37, 81, 129, 1, 2, 3],
    ]),
    ("gray16-still", 16, 0, [[0, 1, 257, 32768, 65534, 65535]]),
]


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


def png(width, height, depth, color, values, local_srgb=True):
    channels = 3 if color == 2 else 1
    data = b"".join(
        b"\0" + (bytes(values[y * width * channels:(y + 1) * width * channels])
                  if depth == 8 else struct.pack(">" + str(width * channels) + "H", *values[y * width * channels:(y + 1) * width * channels]))
        for y in range(height)
    )
    return (chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, depth, color, 0, 0, 0))
            + (chunk(b"sRGB", b"\0") if local_srgb else b"") + chunk(b"IDAT", zlib.compress(data)) + chunk(b"IEND", b""))


def main():
    ROOT.mkdir(exist_ok=True)
    if not any(c[0] == "rgb8-two-layer-still" for c in CASES):
        CASES.append(("rgb8-two-layer-still", 8, 2, CASES[0][3]))
    if not any(c[0] == "rgb8-global-srgb" for c in CASES):
        CASES.append(("rgb8-global-srgb", 8, 2, CASES[0][3]))
    for name, depth in [("rgb8-partial-two-frames",8), ("rgb16-partial-two-frames",16),
                        ("rgb8-partial-still",8), ("rgb8-tall-partial-two-frames",8)]:
        if not any(c[0] == name for c in CASES):
            base = CASES[0][3][0]
            frames = [base, [9,8,7,6,5,4]] if depth == 8 else [[v*257 for v in base], [4095,8191,65534,32768,101,22007]]
            CASES.append((name,depth,2,frames))
    cases = []
    for name, depth, color, frames in CASES:
        ticks = 0 if name.endswith("-still") else 10
        dimensions = [(1,2) if "tall-partial" in name and i else
                      (2,1) if "partial" in name and i else (3,2) for i in range(len(frames))]
        payload = (b"\x8aMNG\r\n\x1a\n" + chunk(b"MHDR", struct.pack(">7I", 3, 2,
                   ticks, len(frames)+1, 1 if ticks == 0 else len(frames), len(frames) if ticks else 0, 65))
                   + (chunk(b"sRGB", b"\0") if name == "rgb8-global-srgb" else b"")
                   + b"".join(png(w,h,depth,color,values,name != "rgb8-global-srgb")
                              for values,(w,h) in zip(frames,dimensions)) + chunk(b"MEND",b""))
        (ROOT / (name+".mng")).write_bytes(payload)
        layers, expected, canvas = [], [], [0]*24
        for values,(w,h) in zip(frames,dimensions):
            rgba=[]
            for index in range(w*h):
                rgba.extend(values[index*3:index*3+3]+[(1<<depth)-1] if color == 2
                            else [values[index]]*3+[(1<<depth)-1])
            layers.append(rgba)
            for row in range(h):
                canvas[row*12:row*12+w*4]=rgba[row*w*4:(row+1)*w*4]
            if ticks: expected.append(canvas.copy())
        if not ticks: expected=[canvas.copy()]
        cases.append({"file":name+".mng","sha256":hashlib.sha256(payload).hexdigest(),
                      "sample_bits":depth,"width":3,"height":2,"ticks_per_second":ticks,
                      "layer_dimensions":dimensions,"layer_rgba":layers,"rgba":expected})
    (ROOT / "manifest.json").write_text(json.dumps({
        "generator": "Python struct/zlib authored MNG-VLC; independent integer source values, not external MNG decoder proof",
        "license": "CC0-1.0", "cases": cases,
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
