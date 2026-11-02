#!/usr/bin/env python3
"""Extract exact ordered-cell input bits from an opt-in native diagnostic log."""
import argparse
import ast
import hashlib
import json
import math
from pathlib import Path
import re
import struct


def bits(value):
    if type(value) is not int or not 0 <= value < 2**64:
        raise ValueError('invalid f64 bit pattern')
    if not math.isfinite(struct.unpack('>d', struct.pack('>Q', value))[0]):
        raise ValueError('nonfinite replay input')
    return value


def extract(text):
    captures = []
    current = None
    for line in text.splitlines():
        if line.startswith('ORDERED_CELL_BEGIN '):
            if current is not None:
                raise ValueError('nested or incomplete capture')
            match = re.fullmatch(r'ORDERED_CELL_BEGIN bits=(\[.*\]) candidates=(\d+) work=(\d+)', line)
            if not match:
                raise ValueError('invalid capture header')
            frame = ast.literal_eval(match[1])
            if len(frame) != 3:
                raise ValueError('invalid coordinate frame')
            current = dict(frame_bits=[bits(v) for v in frame], count=int(match[2]),
                           work_before=int(match[3]), rows=[])
            if struct.unpack('>d', struct.pack('>Q', frame[2]))[0] <= 0:
                raise ValueError('nonpositive cell side')
        elif line.startswith('ORDERED_CELL_ROW '):
            if current is None:
                raise ValueError('row outside capture')
            row = ast.literal_eval(line.removeprefix('ORDERED_CELL_ROW '))
            patch, sample, components, points, fields = row
            if any(type(v) is not int or v < 0 for v in [patch, sample, components]) or not 1 <= components <= 4:
                raise ValueError('invalid source identity/components')
            if len(points) != 3 or any(len(p) != 2 for p in points) or len(fields) != 6 or any(len(f) != 3 for f in fields):
                raise ValueError('invalid triangle/affine shape')
            for group in [*points, *fields]:
                for v in group:
                    bits(v)
            current['rows'].append(row)
        elif line == 'ORDERED_CELL_END':
            if current is None or len(current['rows']) != current.pop('count'):
                raise ValueError('capture count mismatch')
            captures.append(current)
            current = None
        elif line.startswith('ORDERED_CELL_INVALID_OWNER'):
            raise ValueError('native owner reconstruction failed')
    if current is not None or not captures:
        raise ValueError('missing or incomplete capture')
    return captures


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('log', type=Path)
    parser.add_argument('source', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--rust', action='store_true', help='Emit one Rust replay fixture without duplicating its rows in JSON')
    parser.add_argument('--capture-index', type=int, help='Explicit zero-based selection when a window contains multiple cells')
    args = parser.parse_args()
    data = args.log.read_bytes()
    result = dict(source_sha256=hashlib.sha256(args.source.read_bytes()).hexdigest(),
                  log_sha256=hashlib.sha256(data).hexdigest(),
                  captures=extract(data.decode()))
    if args.rust:
        if args.capture_index is None and len(result['captures']) != 1:
            raise ValueError('Rust fixture requires exactly one capture or --capture-index')
        index = 0 if args.capture_index is None else args.capture_index
        if not 0 <= index < len(result['captures']):
            raise ValueError('capture index out of range')
        capture = result['captures'][index]
        rows = ',\n'.join(repr(tuple(row)) for row in capture['rows'])
        args.output.write_text('// Exact native cell input; metadata is recorded in the associated report.\n(&[\n' + rows + '\n], ' + str(capture['work_before']) + ')\n')
    else:
        args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(f"captures={len(result['captures'])} rows={[len(c['rows']) for c in result['captures']]}")
