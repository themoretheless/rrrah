#!/usr/bin/env python3
"""Inspect pinned font table containers and render a bounded FreeType specimen."""
from pathlib import Path
import hashlib
import json
import struct
import sys
from PIL import Image, ImageDraw, ImageFont, __version__ as pillow_version

source, output = map(Path, sys.argv[1:])
output.mkdir(parents=True, exist_ok=True)
b = source.read_bytes()
u16 = lambda off: struct.unpack_from(">H", b, off)[0]
u32 = lambda off: struct.unpack_from(">I", b, off)[0]
def checksum(data):
    data += b"\0" * (-len(data) % 4)
    return sum(struct.unpack(">" + "I" * (len(data) // 4), data)) & 0xffffffff

assert b[:4] == b"OTTO"
tables = {}
records = []
for i in range(u16(4)):
    tag, expected, off, length = struct.unpack_from(">4sIII", b, 12 + i * 16)
    tag = tag.decode("ascii")
    assert tag not in tables and off % 4 == 0 and off + length <= len(b)
    data = b[off:off + length]
    if tag == "head":
        data = data[:8] + b"\0" * 4 + data[12:]
    assert checksum(data) == expected, tag
    tables[tag] = (off, length)
    records.append({"tag": tag, "offset": off, "length": length,
                    "checksum_verified": True})
intervals = sorted((off, off + length) for off, length in tables.values())
assert all(a[1] <= c[0] for a, c in zip(intervals, intervals[1:]))
assert checksum(b) == 0xB1B0AFBA
off, length = tables["name"]
names = []
for i in range(u16(off + 2)):
    platform, encoding, language, name_id, size, rel = struct.unpack_from(">6H", b, off + 6 + i * 12)
    start = off + u16(off + 4) + rel
    assert off <= start <= start + size <= off + length
    raw = b[start:start + size]
    codec = "utf-16-be" if platform in (0, 3) else "mac_roman" if platform == 1 else "latin1"
    text = raw.decode(codec)
    names.append({"platform": platform, "encoding": encoding, "language": language,
                  "name_id": name_id, "text": text})
cmap, cmap_length = tables["cmap"]
cmap_records = []
for i in range(u16(cmap + 2)):
    platform, encoding, rel = struct.unpack_from(">HHI", b, cmap + 4 + i * 8)
    start = cmap + rel
    assert cmap <= start < cmap + cmap_length
    fmt = u16(start)
    size = u32(start + 4) if fmt in (8, 10, 12, 13) else u32(start + 2) if fmt == 14 else u16(start + 2)
    assert start + size <= cmap + cmap_length
    cmap_records.append({"platform": platform, "encoding": encoding,
                         "format": fmt, "bytes": size})
font = ImageFont.truetype(str(source), 38)
image = Image.new("RGB", (1100, 240), "white")
draw = ImageDraw.Draw(image)
for y, text in zip((12, 66, 120, 174), (
        "FreeSerif: Image 1 / Image 2 — 0123456789",
        "Duplicate files / Перевірка / Дубликаты",
        "Αλφάβητο  ABC xyz  Жж Яя  € £ ©",
        "Aa Bb Cc Dd Ee Ff — source font, native FreeType")):
    draw.text((15, y), text, font=font, fill="black")
specimen = output / "font-specimen.png"
image.save(specimen)
result = {"source_sha256": hashlib.sha256(b).hexdigest(), "bytes": len(b),
          "tables": records, "whole_font_checksum_verified": True,
          "name_records": names, "cmap_containers": cmap_records,
          "glyph_count": u16(tables["maxp"][0] + 4),
          "pillow_version": pillow_version, "family_style": font.getname(),
          "specimen": str(specimen),
          "scope": "All SFNT table ranges/checksums, name records and cmap container bounds; bounded actual FreeType render. Not all CFF glyph programs, shaping, language or original GUI qualification."}
(output / "font-control.json").write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")
print(json.dumps({k: result[k] for k in ("bytes", "glyph_count", "family_style", "cmap_containers")}, ensure_ascii=False))
for text in sorted({n["text"] for n in names if n["name_id"] in (0, 13, 14)}):
    print(text)
