#!/usr/bin/env python3
"""Inspect pinned ZD TIFF/MakerNote metadata; never decode or develop pixels."""
import argparse
import hashlib
import json
from pathlib import Path
import struct

parser = argparse.ArgumentParser()
parser.add_argument('--source', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
data = args.source.read_bytes()
digest = hashlib.sha256(data).hexdigest()
assert digest == 'bcd63507c3c4cc3ea1bad2945e3f88cb4677a1a3c762e1acc38df4445714eb7c'
assert data[:4] == b'MM\0*'
sizes = {1: 1, 2: 1, 3: 2, 4: 4, 5: 8, 7: 1, 9: 4, 10: 8, 11: 4, 12: 8}

def directory(offset):
    assert 0 <= offset <= len(data) - 2
    count, = struct.unpack_from('>H', data, offset)
    assert count <= 512 and offset + 2 + count * 12 + 4 <= len(data)
    entries = {}
    for index in range(count):
        position = offset + 2 + index * 12
        tag, kind, length, pointer = struct.unpack_from('>HHII', data, position)
        assert tag not in entries and kind in sizes
        size = length * sizes[kind]
        start = position + 8 if size <= 4 else pointer
        assert size <= 4 * 1024 * 1024 and start + size <= len(data)
        entries[tag] = (kind, length, start, data[start:start + size])
    return entries

def scalar(entry):
    kind, length, _, value = entry
    assert kind == 4 and length == 1
    return struct.unpack('>I', value)[0]

root = directory(struct.unpack_from('>I', data, 4)[0])
exif = directory(scalar(root[34665]))
kind, length, maker_offset, maker = exif[37500]
assert kind == 7 and length == 116120
notes = directory(maker_offset)
assert set(notes) == set(range(0x81, 0x8a))
report = []
for tag, (kind, length, offset, value) in notes.items():
    item = {'tag': hex(tag), 'type': kind, 'count': length, 'offset': offset,
            'bytes': len(value), 'nonzero_bytes': sum(x != 0 for x in value),
            'sha256': hashlib.sha256(value).hexdigest()}
    if kind == 4:
        item['unsigned_values'] = list(struct.unpack('>' + 'I' * length, value))
    report.append(item)
for tag in [0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88]:
    assert not any(notes[tag][3])
standard_tags = [50721, 50722, 50728, 50729, 34675, 50831]
result = {'status': 'pinned_metadata_audited_color_unresolved', 'source_sha256': digest,
          'maker_note_offset': maker_offset, 'maker_note_entries': report,
          'standard_color_tags_in_root_or_exif': {str(t): t in root or t in exif for t in standard_tags},
          'conclusion': 'All seven non-text MakerNote entries are zero; text descriptions do not supply gains or a color transform.',
          'limitations': ['pinned single Mamiya ZD source', 'root/Exif/MakerNote metadata only',
                          'no assertion that arbitrary unknown file regions contain no color data',
                          'no camera white balance inferred; MEF viewer remains unqualified']}
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(result, indent=2) + '\n')
print('Nine MakerNote entries audited: seven non-text blocks entirely zero; color remains unresolved')
