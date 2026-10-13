#!/usr/bin/env python3
"""Structured review of all remaining lock, locale and binary asset records."""
import base64
import collections
import hashlib
import json
import re
import struct
import tomllib
import zlib
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research/dedup-500'
T = Path('/Users/themoretheless/.codex/tmp')
S = T / 'rrrah-skiff-source-20261012'

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def json_trailing_commas(text):
    # Preserve strings byte-for-byte; remove only commas before a closing token.
    chars = list(text)
    quoted = escaped = False
    for i, ch in enumerate(text):
        if quoted:
            if escaped:
                escaped = False
            elif ch == '\\':
                escaped = True
            elif ch == '"':
                quoted = False
        elif ch == '"':
            quoted = True
        elif ch == ',':
            j = i + 1
            while j < len(text) and text[j].isspace():
                j += 1
            if j < len(text) and text[j] in ']}':
                chars[i] = ' '
    return json.loads(''.join(chars))

def flatten(value, prefix=''):
    out = {}
    for key, item in value.items():
        name = f'{prefix}.{key}' if prefix else key
        if isinstance(item, dict):
            out.update(flatten(item, name))
        else:
            assert isinstance(item, str) and item
            out[name] = item
    return out

def png(data):
    assert data[:8] == b'\x89PNG\r\n\x1a\n'
    pos = 8
    chunks = []
    compressed = bytearray()
    while pos < len(data):
        size = struct.unpack_from('>I', data, pos)[0]
        kind = data[pos+4:pos+8]
        payload = data[pos+8:pos+8+size]
        assert len(payload) == size
        assert zlib.crc32(kind + payload) == struct.unpack_from('>I', data, pos+8+size)[0]
        chunks.append(kind.decode('ascii'))
        if kind == b'IHDR':
            header = struct.unpack('>IIBBBBB', payload)
        if kind == b'IDAT':
            compressed.extend(payload)
        pos += 12 + size
        if kind == b'IEND':
            break
    assert pos == len(data) and chunks[0] == 'IHDR' and chunks[-1] == 'IEND'
    raw = zlib.decompress(compressed)
    width, height, depth, color, compression, filtering, interlace = header
    assert (depth, color, compression, filtering, interlace) == (8, 6, 0, 0, 0)
    assert len(raw) == height * (1 + width * 4)
    assert all(raw[y * (1 + width * 4)] <= 4 for y in range(height))
    return dict(width=width, height=height, depth=depth, color_type=color,
                chunks=chunks, inflated_bytes=len(raw), all_chunk_crcs_valid=True)

def main():
    parent = D / 'skiff-full-file-prefix-117-final-pages-review-20261012.json'
    p = json.loads(parent.read_text())
    assert all(sha(f) == h for f, h in p['pins'].items())
    module = D / 'skiff-exact-module-source-runtime-review-20261012.json'
    m = json.loads(module.read_text())
    remaining = p['remaining_files_not_fully_reviewed']
    assert len(remaining) == 12
    assert all(sha(S / n) == row['sha256'] for n, row in m['archive_inventory'].items())

    cargo = tomllib.loads((S / 'src-tauri/Cargo.lock').read_text())
    packages = cargo['package']
    identities = {(x['name'], x['version']) for x in packages}
    assert len(identities) == len(packages) and cargo['version'] == 4
    unresolved = []
    for x in packages:
        if 'source' in x:
            assert x['source'] == 'registry+https://github.com/rust-lang/crates.io-index'
            assert re.fullmatch('[0-9a-f]{64}', x['checksum'])
        for dep in x.get('dependencies', []):
            parts = dep.split()
            matches = [y for y in identities if y[0] == parts[0] and (len(parts) == 1 or y[1] == parts[1])]
            if len(matches) != 1:
                unresolved.append([x['name'], dep, matches])
    assert not unresolved
    local = [x for x in packages if 'source' not in x]
    assert len(local) == 1 and local[0]['name'] == 'skiff'

    bun = json_trailing_commas((S / 'bun.lock').read_text())
    manifest = json.loads((S / 'package.json').read_text())
    assert bun['lockfileVersion'] == bun['configVersion'] == 1
    for section in ['dependencies', 'devDependencies']:
        assert bun['workspaces'][''][section] == manifest[section]
        assert all(name in bun['packages'] for name in manifest[section])
    schemas = collections.Counter()
    for name, row in bun['packages'].items():
        assert isinstance(row, list) and len(row) == 4
        assert isinstance(row[0], str) and isinstance(row[1], str) and isinstance(row[2], dict)
        assert row[3].startswith('sha512-')
        assert len(base64.b64decode(row[3][7:], validate=True)) == 64
        schemas.update(row[2].keys())

    locales = []
    for left, right in [('en-US', 'zh-CN'), ('target-en-US', 'target-zh-CN')]:
        a, b = [flatten(json.loads((S / f'src/locales/{n}.json').read_text())) for n in [left, right]]
        assert a.keys() == b.keys()
        assert all(sorted(re.findall(r'\{[^{}]+\}', a[k])) == sorted(re.findall(r'\{[^{}]+\}', b[k])) for k in a)
        locales.append(dict(pair=[left, right], strings_per_file=len(a), key_and_placeholder_parity=True,
                            all_values_nonempty_strings=True))

    icons = {}
    for n in remaining:
        path = S / n
        data = path.read_bytes()
        if path.suffix == '.png':
            icons[n] = png(data)
        elif path.suffix == '.ico':
            reserved, kind, count = struct.unpack_from('<HHH', data)
            assert (reserved, kind, count) == (0, 1, 1)
            w, h, palette, reserved, planes, bits, size, offset = struct.unpack_from('<BBBBHHII', data, 6)
            assert offset == 22 and offset + size == len(data)
            image = png(data[offset:offset+size])
            assert image['width'] == (w or 256) and image['height'] == (h or 256)
            icons[n] = dict(entries=count, bits=bits, png=image)
        elif path.suffix == '.icns':
            assert data[:4] == b'icns' and struct.unpack_from('>I', data, 4)[0] == len(data)
            pos = 8
            entries = []
            while pos < len(data):
                tag = data[pos:pos+4].decode('ascii')
                size = struct.unpack_from('>I', data, pos+4)[0]
                assert size >= 8 and pos + size <= len(data)
                payload = data[pos+8:pos+size]
                entries.append(dict(type=tag, bytes=size, png=png(payload) if payload.startswith(b'\x89PNG') else None))
                pos += size
            assert pos == len(data)
            icons[n] = dict(entries=entries)
    assert len(icons) == 6
    previews = [T / f'rrrah-skiff-{kind}-review-20261012.png' for kind in ['icns', 'ico']]
    for preview in previews:
        png(preview.read_bytes())
    out = D / 'skiff-complete-source-review-20261012.json'
    assert not out.exists()
    out.write_text(json.dumps(dict(
        status='verified_complete_129_file_review', repository=m['repository'], commit=m['commit'],
        total_files=129, fully_text_read_files=p['full_files_read'], fully_text_read_count=117,
        structured_review_files=remaining, remaining_files_not_reviewed=[],
        cargo=dict(packages=len(packages), dependency_edges=sum(len(x.get('dependencies', [])) for x in packages),
                   all_registry_checksums_well_formed=True, all_edges_resolve=True, local_packages=local),
        bun=dict(packages=len(bun['packages']), direct_manifest_parity=True,
                 all_sha512_integrities_well_formed=True, metadata_fields=dict(schemas)),
        locales=locales, icons=icons,
        review_method='117 text source/config/doc files fully read across prior prefixes. Remaining two locks parsed record-by-record for identities, checksums, edges and direct manifest parity; four locales parsed for every key/value/placeholder with duplicate/delete/cancel/error/confirmation strings inspected in both languages. Six binary icons structurally inspected, PNG CRC/inflation validated; all six displayed, ICNS/ICO via task-owned sips PNG previews. Not a literal line reading claim for generated locks or semantic review of every translation.',
        findings=[
            'Lock files introduce dependency resolution records, not another duplicate matcher. Cargo registry dependency edges resolve; bun direct dependency ranges match package.json. Integrity shape validation does not verify downloaded package content or dependency source safety.',
            'Locale key/placeholder sets match across languages, 468 primary and70 target strings per language. Duplicate text explicitly advertises size/hash/byte comparison and keep1pergroup; earlier overlap/keeper runtime counterexamples remain applicable despite UI claims.',
            'Icons contain application/tray imagery only. Binary structure and visual inspection do not establish mounted app rendering or resource safety.',
        ],
        implementation_adopted=False, full_repository_review_complete=True,
        pins={str(f):sha(f) for f in [parent, module, Path(__file__), *previews] + [S/n for n in m['archive_inventory']]},
        scope='Complete pinned upstream repository file review, with prior explicitly scoped original-unit and authored fixture evidence. Not full upstream application build, dependency-source audit, current rrrah parity, other-platform execution, general resource bound or all500 completion.',
    ), indent=2) + '\n')
    print('Complete129-file review:', len(packages), 'Cargo packages;', len(bun['packages']), 'bun packages;', len(icons), 'icons')

if __name__ == '__main__':
    main()
