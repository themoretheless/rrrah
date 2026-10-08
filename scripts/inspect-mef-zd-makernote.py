#!/usr/bin/env python3
"""Inventory a pinned ZD MakerNote without guessing white-balance semantics."""
import argparse
import hashlib
import json
from pathlib import Path
import struct

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('source', type=Path)
parser.add_argument('--report', type=Path, required=True)
args = parser.parse_args()
data = args.source.read_bytes()
expected = 'bcd63507c3c4cc3ea1bad2945e3f88cb4677a1a3c762e1acc38df4445714eb7c'
if hashlib.sha256(data).hexdigest() != expected:
    raise SystemExit('Only the pinned Mamiya ZD source is qualified for this inventory')
sizes = {2: 1, 3: 2, 4: 4, 7: 1}
offset, seen, directories = 800, set(), []
while offset:
    if offset in seen or len(seen) >= 8 or not 800 <= offset < 116920:
        raise SystemExit('MakerNote directory cycle/count/range refused')
    seen.add(offset)
    count = struct.unpack_from('>H', data, offset)[0]
    if count > 128 or offset + 6 + count * 12 > 116920:
        raise SystemExit('MakerNote table exceeds admitted range')
    entries = []
    for index in range(count):
        entry = offset + 2 + index * 12
        tag, kind, length, value = struct.unpack_from('>HHII', data, entry)
        if kind not in sizes:
            raise SystemExit(f'Unknown field type {kind}')
        size = sizes[kind] * length
        start = entry + 8 if size <= 4 else value
        if start < 800 or start + size > 116920:
            raise SystemExit('MakerNote field exceeds admitted range')
        raw = data[start:start + size]
        field = {'tag': hex(tag), 'type': kind, 'count': length,
                 'sha256': hashlib.sha256(raw).hexdigest()}
        if kind in (3, 4) and length <= 16:
            field['values'] = list(struct.unpack('>' + ('H' if kind == 3 else 'I') * length, raw))
        # Preserve short opaque payloads verbatim. Any numeric reinterpretation
        # below is investigative evidence, not a claim about camera semantics.
        if kind == 7 and length <= 32:
            field['raw_hex'] = raw.hex()
            if length % 4 == 0:
                field['candidate_big_endian_u32'] = list(struct.unpack('>' + 'I' * (length // 4), raw))
        if kind == 2:
            field['text'] = raw.rstrip(b'\0').decode('ascii', errors='replace')
        entries.append(field)
    next_offset = struct.unpack_from('>I', data, offset + 2 + count * 12)[0]
    directories.append({'offset': offset, 'entries': entries, 'next': next_offset})
    offset = next_offset
report = {'source_sha256': expected, 'directories': directories,
          'scope': 'Structural inventory only; numeric fields have no inferred camera meaning',
          'limitations': ['White-balance semantics and camera color remain unresolved']}
args.report.write_text(json.dumps(report, indent=2) + '\n')
