#!/usr/bin/env python3
"""Exact rational boundary proof and affine integrals of captured input triangles.

This qualifies the supplied binary64 geometry/fields, not their curved source.
Unoccluded integrals describe visible coverage only when the full proof passes.
"""
import argparse
from collections import Counter
from fractions import Fraction as F
import hashlib
import json
from pathlib import Path
import re
import runpy
import struct


def real(bits):
    return F(struct.unpack(">d", struct.pack(">Q", bits))[0])


def fingerprint(value):
    data = value.to_bytes(max(1, (abs(value).bit_length() + 8) // 8), "big", signed=True)
    return hashlib.sha256(data).hexdigest()


def exact_value(value):
    return {
        "nearest_binary64": float(value),
        "numerator_bits": abs(value.numerator).bit_length(),
        "denominator_bits": value.denominator.bit_length(),
        "numerator_sha256": fingerprint(value.numerator),
        "denominator_sha256": fingerprint(value.denominator),
    }


def verify(rows):
    geometry = runpy.run_path(str(Path(__file__).with_name("verify-ordered-triangle-union.py")))
    clip, cross, area = (geometry[name] for name in ["clip", "cross", "area"])
    square = [(F(0), F(0)), (F(1), F(0)), (F(1), F(1)), (F(0), F(1))]
    edges = Counter()
    integral = [F(0) for _ in range(5)]
    for row in rows:
        polygon = [tuple(real(bits) for bits in point) for point in row[3]]
        if cross(*polygon) < 0:
            polygon.reverse()
        for a, b in zip(square, square[1:] + square[:1]):
            polygon = clip(polygon, a, b)
        weight = area(polygon)
        if weight == 0:
            continue
        assert weight > 0
        mx = my = F(0)
        for a, b in zip(polygon, polygon[1:] + polygon[:1]):
            product = a[0] * b[1] - b[0] * a[1]
            mx += (a[0] + b[0]) * product / 6
            my += (a[1] + b[1]) * product / 6
            if a != b:
                edges[(min(a, b), max(a, b))] += 1 if a < b else -1
        integral[0] += weight
        for k in range(4):
            f = [real(bits) for bits in row[4][k + 2]]
            integral[k + 1] += f[0] * weight + f[1] * mx + f[2] * my
    interior = invalid = remaining = 0
    end = [F(0) for _ in range(4)]
    for (a, b), count in sorted(edges.items()):
        if count == 0:
            continue
        remaining += 1
        if a[1] == b[1] == 0:
            side, start, stop, expected = 0, a[0], b[0], 1
        elif a[0] == b[0] == 1:
            side, start, stop, expected = 1, a[1], b[1], 1
        elif a[1] == b[1] == 1:
            side, start, stop, expected = 2, a[0], b[0], -1
        elif a[0] == b[0] == 0:
            side, start, stop, expected = 3, a[1], b[1], -1
        else:
            interior += 1
            continue
        if count != expected or start != end[side] or stop <= start:
            invalid += 1
        end[side] = stop
    certified = interior == invalid == 0 and end == [F(1)] * 4
    if certified:
        assert integral[0] == 1
    return {
        "scope": "Exact Fraction arithmetic on binary64 input triangles and affine fields; no original curved-source or output-color-space claim",
        "triangles": len(rows),
        "certified_nonoverlap_full_unit_coverage": certified,
        "residual_edges": remaining,
        "interior_residual_edges": interior,
        "invalid_border_intervals": invalid,
        "unoccluded_affine_integral": [exact_value(value) for value in integral],
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("--assert-full", action="store_true")
    args = parser.parse_args()
    data = args.input.read_bytes()
    extract = runpy.run_path(str(Path(__file__).with_name("extract-ordered-cell.py")))["extract"]
    if args.input.suffix == ".rs":
        fixture = re.fullmatch(r"(?://[^\n]*\n)*\(&\[\n(.*)\n\],\s*\d+\)\s*", data.decode(), re.S)
        if not fixture:
            raise ValueError("expected an exact literal-row capture fixture")
        lines = fixture[1].splitlines()
        if not lines or any(not line.startswith("(") or not line.endswith(("),", ")")) for line in lines):
            raise ValueError("nonliteral or multiline capture row")
        lines = [line.rstrip(",") for line in lines]
        # Reuse the strict row/finite-bit/schema validator of native captures.
        text = (f"ORDERED_CELL_BEGIN bits=[0, 0, {struct.unpack('>Q', struct.pack('>d', 1.))[0]}] candidates={len(lines)} work=100000000\n"
                + "\n".join("ORDERED_CELL_ROW " + line for line in lines) + "\nORDERED_CELL_END\n")
        rows = extract(text)[0]["rows"]
    else:
        captures = extract(data.decode())
        if len(captures) != 1:
            raise ValueError("select exactly one native cell capture")
        rows = captures[0]["rows"]
    result = verify(rows)
    result["input_sha256"] = hashlib.sha256(data).hexdigest()
    print(json.dumps(result, indent=2))
    if args.assert_full and not result["certified_nonoverlap_full_unit_coverage"]:
        raise SystemExit(1)
