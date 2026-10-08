#!/usr/bin/env python3
"""Create authored/reference-encoded QOI controls using pinned test-only C code."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
REFERENCE = ROOT / 'scripts/oracles/qoi-reference'
OUTPUT = ROOT / 'tests/fixtures/qoi'

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def opcodes(data):
    counts = {}
    cursor = 14
    while cursor < len(data) - 8:
        value = data[cursor]
        kind, length = ('RGBA', 5) if value == 255 else ('RGB', 4) if value == 254 else (
            [('INDEX', 1), ('DIFF', 1), ('LUMA', 2), ('RUN', 1)][value >> 6])
        counts[kind] = counts.get(kind, 0) + 1
        cursor += length
    assert cursor == len(data) - 8
    return counts

def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    rows = []
    with tempfile.TemporaryDirectory() as temporary:
        temporary = Path(temporary)
        driver = temporary / 'qoi-reference'
        subprocess.run(['cc', '-std=c99', '-O2', str(REFERENCE / 'driver.c'), '-o', str(driver)], check=True)
        for name, channels in [('opaque-rgb', 3), ('opaque-rgba', 4), ('alpha-rgba', 4)]:
            width, height = 257, 3
            pixels = []
            for index in range(width * height):
                if index < 130:
                    rgb = [0, 0, 0]
                elif index % 11 < 3:
                    rgb = [32, 64, 96]
                elif index % 11 < 6:
                    rgb = [(index * 73) % 256, (index * 131) % 256, (index * 199) % 256]
                else:
                    rgb = [index % 256, (index + 1) % 256, (index + 2) % 256]
                alpha = [0, 1, 127, 128, 254, 255][index % 6] if name == 'alpha-rgba' else 255
                pixels.append(bytes(rgb + [alpha]))
            expected = b''.join(pixels)
            raw = temporary / 'source.raw'
            raw.write_bytes(b''.join(pixel[:channels] for pixel in pixels))
            for flag in [0, 1]:
                source = OUTPUT / f'{name}-{flag}.qoi'
                subprocess.run([str(driver), 'encode', str(raw), str(source), str(width), str(height), str(channels), str(flag)], check=True)
                record(driver, source, width, height, channels, flag, expected, 'upstream-reference-encoder', rows)
        # All six opcode families, long initial run, byte wrapping and INDEX
        # reuse across rows. Expected pixels are specified independently.
        body = bytes([0xfd, 0xff, 255, 0, 255, 128, 0x7f, 0xbf, 0xff,
                      0xfe, 32, 64, 96, 54, 0xc0])
        expected = bytes([0, 0, 0, 255]) * 62 + bytes([
            255, 0, 255, 128, 0, 1, 0, 128, 38, 32, 38, 128,
            32, 64, 96, 128, 255, 0, 255, 128, 255, 0, 255, 128])
        for flag in [0, 1]:
            source = OUTPUT / f'all-opcodes-{flag}.qoi'
            source.write_bytes(b'qoif' + (17).to_bytes(4, 'big') + (4).to_bytes(4, 'big')
                               + bytes([4, flag]) + body + bytes([0] * 7 + [1]))
            record(driver, source, 17, 4, 4, flag, expected, 'authored-reference-decoder-verified', rows)
    manifest = {'upstream': 'https://github.com/phoboslab/qoi',
                'commit': 'ffb2d2cb74a1de60819b21b939f7209aa53e91c1',
                'header_sha256': digest(REFERENCE / 'qoi.h'),
                'driver_sha256': digest(REFERENCE / 'driver.c'), 'fixtures': rows}
    (OUTPUT / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')

def record(driver, source, width, height, channels, flag, expected, producer, rows):
    oracle = source.with_suffix('.rgba')
    subprocess.run([str(driver), 'decode', str(source), str(oracle)], check=True)
    assert oracle.read_bytes() == expected, source.name
    rows.append({'source': source.name, 'oracle': oracle.name, 'width': width,
                 'height': height, 'channels': channels, 'flag': flag, 'producer': producer,
                 'source_sha256': digest(source), 'oracle_sha256': digest(oracle),
                 'opcodes': opcodes(source.read_bytes())})

if __name__ == '__main__':
    main()
