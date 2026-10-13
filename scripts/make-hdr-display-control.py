#!/usr/bin/env python3
"""Write an original CC0 linear Rec.709 RGBE display control; print its manifest."""
import argparse
import hashlib
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    width, height = 1024, 512
    header = (
        b"#?RADIANCE\nPRIMARIES= .64 .33 .30 .60 .15 .06 .3127 .3290\n"
        b"FORMAT=32-bit_rle_rgbe\n\n-Y 512 +X 1024\n"
    )
    # 128 * 2**(exponent - 136) gives exact neutral powers of two.
    planes = [bytes([128]) * width] * 3 + [
        b"".join(bytes([exponent]) * 128 for exponent in range(126, 134))
    ]
    scanline = bytes([2, 2, 4, 0]) + b"".join(
        bytes([128]) + plane[start:start + 128]
        for plane in planes for start in range(0, width, 128)
    )
    data = header + scanline * height
    # Explicit output path, but refuse accidental replacement of another file.
    with args.output.open("xb") as output:
        output.write(data)
    print(json.dumps({
        "producer": "Original CC0 authored Radiance RGBE control; declared Rec709 primaries",
        "width": width, "height": height,
        "linear_neutral_stripes": [0.125, 0.25, 0.5, 1, 2, 4, 8, 16],
        "sha256": hashlib.sha256(data).hexdigest(),
    }, indent=2))


if __name__ == "__main__":
    main()
