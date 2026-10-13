#!/usr/bin/env python3
import csv
import hashlib
import io
import json
import re
import zipfile
from pathlib import Path

import PIL
from PIL import Image

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research'

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

class Ranges:
    def __init__(self, archive):
        self.size, self.ranges, self.pos = archive['size'], archive['ranges'], 0
    def tell(self):
        return self.pos
    def seek(self, offset, whence=0):
        self.pos = offset if whence == 0 else self.pos + offset if whence == 1 else self.size + offset
        assert 0 <= self.pos <= self.size
        return self.pos
    def seekable(self):
        return True
    def read(self, size=-1):
        left = self.size - self.pos if size < 0 else min(size, self.size - self.pos)
        result = bytearray()
        while left:
            candidates = [x for x in self.ranges if x['start'] <= self.pos <= x['end']]
            assert candidates, ('missing pinned archive range', self.pos, left)
            item = max(candidates, key=lambda x: x['end'])
            amount = min(left, item['end'] + 1 - self.pos)
            with Path(item['path']).open('rb') as source:
                source.seek(self.pos - item['start'])
                data = source.read(amount)
            assert len(data) == amount
            result.extend(data)
            self.pos += amount
            left -= amount
        return bytes(result)

def independent_hash(path):
    with Image.open(path) as image:
        values = list(image.convert('L').resize((9, 8), Image.Resampling.LANCZOS).get_flattened_data())
    bits = []
    for row in range(8):
        for col in range(8):
            bits.append('1' if values[row * 9 + col] > values[row * 9 + col + 1] else '0')
    return int(''.join(reversed(bits)), 2)

def main():
    prep = D / 'dedup-disc21-heldout-negatives-prepared-20261012.json'
    c = json.loads(prep.read_text())
    assert all(sha(k) == v for k, v in c['pins'].items())
    plan_path = Path(c['preplan'])
    pre = json.loads(plan_path.read_text())
    assert all(sha(k) == v for k, v in pre['pins'].items())
    assert PIL.__version__ == c['pillow_version']
    rows = list(csv.reader(io.StringIO(Path(pre['ground_truth']).read_text())))
    mapping = dict(rows)
    assert len(rows) == len(mapping) == 10000
    used = set()
    for capture in pre['exclusion_captures']:
        raw = Path(capture['capture']).read_text()
        assert sha(capture['capture']) == capture['sha256']
        ids = sorted(set(re.findall(r'(?<![A-Za-z0-9])(?:Q\d{5}|R\d{6})(?!\d)', raw)))
        assert ids == capture['ids']
        used.update(ids)
    assert used == set(pre['excluded_ids'])
    eligible = [f'Q{i:05d}' for i in range(50000) if f'Q{i:05d}' not in mapping and f'Q{i:05d}' not in used]
    eligible.sort(key=lambda q: (hashlib.sha256((pre['seed'] + ':' + q).encode()).hexdigest(), q))
    assert eligible[:64] == pre['candidate_queries'] == c['pool_queries']
    refs = []
    for _, ref in rows:
        if int(ref[1:]) < 50000 and ref not in used and ref not in refs:
            refs.append(ref)
        if len(refs) == 8:
            break
    assert refs == pre['reference_ids'] == c['references']
    assert set(refs + eligible[:64]).isdisjoint(used)
    images = {x['id']: x for x in c['images']}
    assert len(images) == len(c['images']) == 72
    for archive in c['archives']:
        with zipfile.ZipFile(Ranges(archive)) as source:
            for image in c['images']:
                if not archive['url'].endswith('/' + image['archive']):
                    continue
                info = source.getinfo(image['member'])
                assert info.CRC == image['zip_crc32']
                data = source.read(info)
                assert data == Path(image['path']).read_bytes() and len(data) == image['size']
    hashes = {key: independent_hash(x['path']) for key, x in images.items()}
    assert hashes == c['hashes']
    nearest = []
    for query in c['pool_queries']:
        distance, ref = min(((hashes[query] ^ hashes[r]).bit_count(), r) for r in refs)
        nearest.append(dict(query=query, reference=ref, distance=distance, published_label='no_reference_match'))
    nearest.sort(key=lambda x: (x['distance'], x['query'], x['reference']))
    assert nearest == c['nearest_pool'] and nearest[:16] == c['plan']
    out = D / 'dedup-disc21-heldout-negatives-prepared-audit-20261012.json'
    assert not out.exists()
    out.write_text(json.dumps({
        'status': 'verified_precommitted_unused16_hash_near_distractor_pairs',
        'pairs': 16, 'query_pool': 64, 'references': 8,
        'prior_excluded_ids': len(used), 'distances': [x['distance'] for x in c['plan']],
        'pins': {str(p): sha(p) for p in [prep, plan_path, Path(__file__)]},
        'scope': 'All prior capture IDs, publisher mapping exclusion, deterministic candidate/reference selection,512distance comparisons,16selected pairs and all72JPEG ZIP provenance independently reconstructed offline. Matching not executed here. Heldout from local policy development, not pretrained model data; hash-near controls, not semantic/burst or general precision proof.',
    }, indent=2) + '\n')
    print('Verified16 unused hash-near distractor pairs,72JPEGs and512candidate distances')

if __name__ == '__main__':
    main()
