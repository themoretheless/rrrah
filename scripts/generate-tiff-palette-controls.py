"""Authored indexed classic/BigTIFF with non-quantized 16-bit palettes."""
from pathlib import Path
import struct
root = Path(__file__).resolve().parents[1] / 'tests/fixtures/raster'
for big in [False, True]:
    for endian, magic, suffix in [('<', b'II', 'le'), ('>', b'MM', 'be')]:
        pack = lambda fmt, *v: struct.pack(endian + fmt, *v)
        tags = [(256, 4, 1, 4), (257, 4, 1, 2), (258, 3, 1, 8), (259, 3, 1, 1),
                (262, 3, 1, 3), (273, 4, 1, 0), (277, 3, 1, 1), (278, 4, 1, 2),
                (279, 4, 1, 8), (320, 3, 768, 0)]
        palette_at = (16 + 8 + len(tags)*20 + 8) if big else (8 + 2 + len(tags)*12 + 4)
        tags[5] = (273, 4, 1, palette_at + 1536)
        tags[-1] = (320, 3, 768, palette_at)
        data = (magic + pack('HHHQ', 43, 8, 0, 16) + pack('Q', len(tags))) if big else (magic + pack('HI', 42, 8) + pack('H', len(tags)))
        for tag, kind, count, value in tags:
            data += pack('HHQ' if big else 'HHI', tag, kind, count)
            if count == 1:
                field = pack('H' if kind == 3 else 'I', value)
                data += field + bytes((8 if big else 4) - len(field))
            else:
                data += pack('Q' if big else 'I', value)
        data += pack('Q' if big else 'I', 0)
        for channel in range(3):
            for i in range(256):
                data += pack('H', [i*251, 65535-i*251, (i*i*31)%65536][channel])
        data += bytes([0, 1, 127, 255, 17, 31, 64, 128])
        (root / f'palette16-{"big-" if big else ""}{suffix}.tiff').write_bytes(data)
