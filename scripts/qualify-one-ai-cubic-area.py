#!/usr/bin/env python3
"""Independent numerical source-cubic coverage oracle for pinned one.ai page 0.

Separate from strict Poppler qualification. No renderer/flattened-path code is
used. Numeric convergence is checked, not a formal interval certificate.
"""
import argparse
import hashlib
import json
from pathlib import Path


def cubic_y(t):
    u = 1 - t
    return 3 * u * u * t * 115.151 + 3 * u * t * t * 208.5 + t ** 3 * 208.5


def render_polynomial(iterations):
    width, height = 1366, 768
    output = bytearray(b'\xff' * (width * height * 3))
    def invert_y(distance):
        lo, hi = 0., 1.
        for _ in range(iterations):
            mid = (lo + hi) / 2
            if cubic_y(mid) < distance: lo = mid
            else: hi = mid
        return (lo + hi) / 2
    derivative_y = [3 * 115.151, 2 * (3 * 208.5 - 6 * 115.151), 3 * (3 * 115.151 - 2 * 208.5)]
    for center, radius, control in [(324., 130., 58.203), (1067., 119., 53.278)]:
        def right(t):
            return center + radius - 3 * control * t*t + (3*control-radius) * t*t*t
        roots = {}
        for x in range(int(center), int(center + radius) + 1):
            lo, hi = 0., 1.
            for _ in range(iterations):
                mid = (lo + hi) / 2
                if right(mid) > x: lo = mid
                else: hi = mid
            roots[x] = (lo + hi) / 2
        for y in range(189, 606):
            halves = []
            for low, high in [(max(float(y), 189.), min(y+1., 397.5)),
                              (max(float(y), 397.5), min(y+1., 606.))]:
                if high > low:
                    halves.append(sorted([invert_y(abs(low-397.5)), invert_y(abs(high-397.5))]))
            for x in range(int(center-radius), int(center+radius)):
                area = 0.
                for lo, hi in halves:
                    cuts = sorted([lo, hi] + [roots[k] for k in [x, x+1, int(2*center-x), int(2*center-x-1)]
                                             if k in roots and lo < roots[k] < hi])
                    for begin, end in zip(cuts, cuts[1:]):
                        middle = right((begin+end)/2)
                        if min(x+1., middle) <= max(float(x), 2*center-middle): continue
                        factor, constant = 0., 0.
                        if middle < x+1: factor += 1
                        else: constant += x+1
                        if 2*center-middle > x: factor += 1; constant -= 2*center
                        else: constant -= x
                        polynomial = [factor*(center+radius)+constant, 0., -factor*3*control,
                                      factor*(3*control-radius)]
                        product = [0.] * 6
                        for i, value in enumerate(polynomial):
                            for j, derivative in enumerate(derivative_y): product[i+j] += value*derivative
                        area += sum(value/(i+1)*(end**(i+1)-begin**(i+1)) for i,value in enumerate(product))
                gray = int(255 * (1 - max(0., min(1., area))) + .5)
                offset = (y*width+x)*3
                output[offset:offset+3] = bytes([gray])*3
    return bytes(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--native', type=Path, required=True, help='Native white-composited RGB PPM')
    parser.add_argument('--output-dir', type=Path, required=True, help='Fresh output directory')
    args = parser.parse_args()
    source_hash = hashlib.sha256(args.source.read_bytes()).hexdigest()
    assert source_hash == '97786949c268233a446e1738fb9c2f941e4d109811bd2f972948cdb7a45d64b4'
    args.output_dir.mkdir()
    coarse, fine = render_polynomial(32), render_polynomial(52)
    header, native = args.native.read_bytes().split(b'\n255\n', 1)
    assert header == b'P6\n1366 768' and len(native) == len(fine)
    convergence = sum(coarse[i:i+3] != fine[i:i+3] for i in range(0, len(fine), 3))
    different = sum(native[i:i+3] != fine[i:i+3] for i in range(0, len(fine), 3))
    report = {'scope': 'Independent numerical full-page cubic coverage, not Poppler qualification',
              'source_sha256': source_hash, 'dimensions': [1366, 768],
              'root_bisection_iterations': [32, 52], 'rgb8_convergence_differing_pixels': convergence,
              'native_differing_pixels': different,
              'native_max_channel_error': max(abs(a-b) for a,b in zip(native, fine)),
              'oracle_rgb_sha256': hashlib.sha256(fine).hexdigest(),
              'native_rgb_sha256': hashlib.sha256(native).hexdigest(),
              'method': 'Source cubic controls; split at pixel-boundary roots, integrate polynomial x(t)*dy/dt analytically, white background black fill',
              'limitations': ['Convergence is numerical, not a certified interval bound',
                              'Pinned one.ai page zero with two filled curves only',
                              'Strict Poppler comparison remains separate and unchanged']}
    (args.output_dir / 'oracle.ppm').write_bytes(b'P6\n1366 768\n255\n' + fine)
    (args.output_dir / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))
    return 0 if convergence == 0 and different == 0 else 1


if __name__ == '__main__':
    raise SystemExit(main())
