"""CC0 authored 12-bit PQ/HLG HEVC lossless gray and RGB controls.

Requires libheif heif-enc/x265 and FFmpeg. Every recovered 12-bit RGB code is
checked against known input; FFmpeg expands grayscale and RGB to u16 differently.
This is not a photographic camera corpus.
"""
from pathlib import Path
import struct
import subprocess
import tempfile
import zlib

ROOT = Path(__file__).resolve().parents[1] / 'tests/fixtures/raster'
WIDTH, HEIGHT = 64, 48
CODES = [0, 1024, 2048, 2049, 3072, 4095, 512, 1536]
COLORS = [(4095, 0, 0), (0, 4095, 0), (0, 0, 4095), (2048, 3072, 1024)]
ALPHAS = [0, 1024, 2048, 4095]


def chunk(kind, value):
    return (struct.pack('>I', len(value)) + kind + value
            + struct.pack('>I', zlib.crc32(kind + value)))


def main():
    with tempfile.TemporaryDirectory(prefix='rrrah-heif-hdr-') as directory:
        source = Path(directory) / 'input.png'
        for name, transfer in [('pq', 16), ('hlg', 18)]:
            for mode in ['gray', 'color', 'color-alpha']:
                color = mode != 'gray'
                alpha = mode == 'color-alpha'
                pixels = [COLORS[x // 16] if color else (CODES[x // 8],) * 3
                          for _ in range(HEIGHT) for x in range(WIDTH)]
                rows = b''.join(b'\0' + b''.join(
                    struct.pack('>H', value << 4)
                    for rgb in pixels[y * WIDTH:(y + 1) * WIDTH]
                    for value in (rgb + (ALPHAS[y // 12],) if alpha else rgb if color else rgb[:1]))
                    for y in range(HEIGHT))
                source.write_bytes(b'\x89PNG\r\n\x1a\n'
                                   + chunk(b'IHDR', struct.pack('>IIBBBBB', WIDTH, HEIGHT, 16,
                                                                6 if alpha else 2 if color else 0, 0, 0, 0))
                                   + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b''))
                output = ROOT / f'heif-{name}12{"-" + mode if color else ""}.heic'
                subprocess.run(['heif-enc', '-L', '--hevc', '-b', '12', '-p', 'x265:pools=1',
                                '--matrix_coefficients', '0', '--colour_primaries', '9',
                                '--transfer_characteristic', str(transfer),
                                '-o', str(output), str(source)], check=True,
                               stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
                oracle = Path(str(output) + '.rgba16le')
                subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-i', str(output),
                                '-frames:v', '1', '-pix_fmt', 'rgba64le', '-f', 'rawvideo',
                                '-y', str(oracle)], check=True)
                decoded = list(struct.iter_unpack('<4H', oracle.read_bytes()))
                assert len(decoded) == len(pixels), output
                assert all(tuple(v >> 4 for v in rgba[:3]) == rgb
                           for rgb, rgba in zip(pixels, decoded)), output
                if alpha:
                    alpha_oracle = Path(str(output) + '.alpha12le')
                    subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-i', str(output),
                                    '-map', '0:1', '-frames:v', '1', '-pix_fmt', 'gray12le',
                                    '-f', 'rawvideo', '-y', str(alpha_oracle)], check=True)
                    decoded_alpha = [v[0] for v in struct.iter_unpack('<H', alpha_oracle.read_bytes())]
                    assert decoded_alpha == [ALPHAS[y // 12] for y in range(HEIGHT)
                                             for _ in range(WIDTH)], output
                    print(output.name, 'all FFmpeg auxiliary alpha codes exact')
                else:
                    assert all(rgba[3] == 65535 for rgba in decoded), output
                print(output.name, output.stat().st_size, 'all FFmpeg 12-bit codes exact')


if __name__ == '__main__':
    main()
