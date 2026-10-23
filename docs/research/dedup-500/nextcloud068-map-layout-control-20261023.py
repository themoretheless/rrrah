"""Decode all mapping segments without executing embedded JavaScript."""
import hashlib
import json
from pathlib import Path
import sys

ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'

def decode(segment):
    result = []
    value = shift = 0
    for char in segment:
        digit = ALPHABET.index(char)
        value |= (digit & 31) << shift
        if digit & 32:
            shift += 5
        else:
            result.append(-(value >> 1) if value & 1 else value >> 1)
            value = shift = 0
    if shift:
        raise ValueError('unterminated VLQ')
    return result

assert decode('AAAA') == [0, 0, 0, 0]
assert decode('D') == [-1]
assert decode('gC') == [32]
assert decode('hC') == [-32]
try:
    decode('g')
except ValueError:
    pass
else:
    raise AssertionError('continuation without terminator accepted')

results = []
assert (len(sys.argv) - 1) % 2 == 0
for filename, generated_file in zip(sys.argv[1::2], sys.argv[2::2]):
    raw = Path(filename).read_bytes()
    data = json.loads(raw)
    generated_raw = Path(generated_file).read_bytes()
    generated_lengths = [len(s.encode('utf-16-le')) // 2 for s in generated_raw.decode().split('\n')]
    source = line = column = name = 0
    segments = mapped = named = 0
    errors = []
    contents = [s.split('\n') if s is not None else None for s in data['sourcesContent']]
    for generated_line, text in enumerate(data['mappings'].split(';')):
        generated_column = 0
        for segment in text.split(','):
            if not segment:
                continue
            values = decode(segment)
            assert len(values) in (1, 4, 5), values
            segments += 1
            generated_column += values[0]
            if generated_column < 0 or values[0] < 0:
                errors.append([generated_line, segment, 'generated-column'])
            if generated_line >= len(generated_lengths) or generated_column > generated_lengths[generated_line]:
                errors.append([generated_line, segment, 'generated-bounds'])
            if len(values) == 1:
                continue
            mapped += 1
            source += values[1]
            line += values[2]
            column += values[3]
            if not 0 <= source < len(contents):
                errors.append([generated_line, segment, 'source-index'])
            elif contents[source] is not None:
                lines = contents[source]
                if not 0 <= line < len(lines):
                    errors.append([generated_line, segment, 'source-line'])
                elif not 0 <= column <= len(lines[line].encode('utf-16-le')) // 2:
                    errors.append([generated_line, segment, 'source-column'])
            if len(values) == 5:
                named += 1
                name += values[4]
                if not 0 <= name < len(data['names']):
                    errors.append([generated_line, segment, 'name-index'])
    results.append({'file': data['file'], 'sha256': hashlib.sha256(raw).hexdigest(),
                    'generated_sha256': hashlib.sha256(generated_raw).hexdigest(),
                    'segments': segments, 'mapped': mapped, 'named': named,
                    'sources': len(contents), 'mapping_lines': len(data['mappings'].split(';')),
                    'errors': errors})
print(json.dumps({'decoder_primitive_controls': 5, 'maps': results,
                  'scope': 'All VLQ mapping segments decoded and source/name bounds checked. Generated UTF16 bounds also checked against pinned emitted files. Semantic emitted-code equivalence and manual full-file review are not established.'}, indent=2))
