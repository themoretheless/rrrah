#!/usr/bin/env python3
"""Precommit unseen IDs, then select hash-near publisher distractors."""
import csv
import hashlib
import importlib.util
import io
import json
import re
import zipfile
from pathlib import Path

import PIL
from PIL import Image

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp')
SEED = 'rrrah-disc21-negative-holdout-20261012'

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def dhash(path):
    with Image.open(path) as image:
        image = image.convert('L').resize((9, 8), Image.Resampling.LANCZOS)
        pixels = list(image.get_flattened_data())
    return sum(int(pixels[y * 9 + x] > pixels[y * 9 + x + 1]) << (y * 8 + x)
               for y in range(8) for x in range(8))

def main():
    folder = T / 'rrrah-disc21-heldout-negative-inputs-20261012'
    preplan = D / 'dedup-disc21-heldout-negatives-plan-20261012.json'
    out = D / 'dedup-disc21-heldout-negatives-prepared-20261012.json'
    assert not folder.exists() and not preplan.exists() and not out.exists()
    parent = D / 'dedup-disc21-cohort-prepared-20261011.json'
    previous = json.loads(parent.read_text())
    assert all(sha(k) == v for k, v in previous['pins'].items())
    pilot_mp = D / 'dedup-disc21-pilot-prepared-20261011.json'
    pilot = json.loads(pilot_mp.read_text())
    ground_truth = next(Path(k) for k in pilot['pins'] if k.endswith('/dev_ground_truth.csv'))
    rows = list(csv.reader(io.StringIO(ground_truth.read_text())))
    mapping = dict(rows)
    assert len(rows) == len(mapping) == 10000
    folder.mkdir()
    captures = folder / 'exclusion-captures'
    captures.mkdir()
    exclusions = []
    used = set()
    for path in sorted(D.glob('dedup-disc21*.json')):
        raw = path.read_bytes()
        ids = sorted(set(re.findall(r'(?<![A-Za-z0-9])(?:Q\d{5}|R\d{6})(?!\d)', raw.decode())))
        if not ids:
            continue
        copy = captures / path.name
        copy.write_bytes(raw)
        used.update(ids)
        exclusions.append({'original': str(path), 'capture': str(copy), 'sha256': sha(copy), 'ids': ids})
    queries = [f'Q{i:05d}' for i in range(50000) if f'Q{i:05d}' not in mapping and f'Q{i:05d}' not in used]
    queries.sort(key=lambda q: (hashlib.sha256((SEED + ':' + q).encode()).hexdigest(), q))
    queries = queries[:64]
    references = []
    for _, ref in rows:
        if int(ref[1:]) < 50000 and ref not in used and ref not in references:
            references.append(ref)
        if len(references) == 8:
            break
    assert len(queries) == 64 and len(references) == 8
    policy_source = T / 'rrrah-disc21-bidirectional-exposure-collection-harness-20261012/src/bin/disc21_bidirectional_exposure_collection.rs'
    preplan.write_text(json.dumps({
        'status': 'precommitted_before_download_or_matching', 'seed': SEED,
        'candidate_queries': queries, 'reference_ids': references, 'excluded_ids': sorted(used),
        'exclusion_captures': exclusions, 'ground_truth': str(ground_truth),
        'selection': '64 unused publisher distractor query IDs ranked by SHA256(seed:ID); first8 unused distinct shard0 reference IDs by CSV order. After download, nearest reference by fixed64bit grayscale dHash9x8 Lanczos; retain16 closest unique queries ordered(distance,query,reference). No primary matcher outputs or thresholds used.',
        'pins': {str(p): sha(p) for p in [parent, pilot_mp, ground_truth, policy_source, Path(__file__)]},
        'scope': 'Held out from prior local DISC21 policy development/evaluations by conservative ID exclusion, not a claim about pretrained model training data, semantic/burst negatives or all DISC21 precision.',
    }, indent=2) + '\n')
    script = R / 'scripts/prepare-dedup-disc21-pilot-20261011.py'
    spec = importlib.util.spec_from_file_location('disc21_remote', script)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)

    class Cached(module.Remote):
        def read(self, n=-1):
            size = self.size - self.pos if n < 0 else min(n, self.size - self.pos)
            if not size:
                return b''
            for item in self.ranges:
                if item['start'] <= self.pos and item['end'] >= self.pos + size - 1:
                    data = Path(item['path']).read_bytes()
                    offset = self.pos - item['start']
                    self.pos += size
                    return data[offset:offset + size]
            return super().read(n)

    images, archives = [], []
    for filename, ids in [('dev_queries.zip', queries), ('references_0.zip', references)]:
        prior = next(a for a in previous['archives'] if a['url'].endswith('/' + filename))
        remote = Cached(prior['url'], filename[:-4], folder)
        assert remote.etag == prior['etag'] and remote.size == prior['size']
        remote.ranges = list(prior['ranges'])
        remote.bytes = sum(x['end'] - x['start'] + 1 for x in remote.ranges)
        with zipfile.ZipFile(remote) as archive:
            members = {Path(x.filename).stem: x for x in archive.infolist() if x.filename.endswith('.jpg')}
            assert len(members) == 50000
            for image_id in ids:
                info = members[image_id]
                assert info.file_size <= 5242880
                data = archive.read(info)
                assert data[:2] == b'\xff\xd8'
                path = folder / (image_id + '.jpg')
                path.write_bytes(data)
                images.append({'id': image_id, 'path': str(path), 'sha256': sha(path), 'archive': filename,
                               'member': info.filename, 'zip_crc32': info.CRC, 'size': len(data)})
        archives.append(remote.evidence())
        print('Fetched', filename, len(ids), 'images', flush=True)
    paths = {x['id']: x['path'] for x in images}
    hashes = {k: dhash(path) for k, path in paths.items()}
    nearest = []
    for query in queries:
        distance, reference = min(((hashes[query] ^ hashes[ref]).bit_count(), ref) for ref in references)
        nearest.append({'query': query, 'reference': reference, 'distance': distance, 'published_label': 'no_reference_match'})
    nearest.sort(key=lambda x: (x['distance'], x['query'], x['reference']))
    pins = {str(p): sha(p) for p in [preplan, parent, pilot_mp, ground_truth, script, Path(__file__), Path(PIL.__file__)]}
    pins.update({x['path']: x['sha256'] for x in images})
    for archive in archives:
        pins.update({x['path']: x['sha256'] for x in archive['ranges']})
    pins.update({x['capture']: x['sha256'] for x in exclusions})
    out.write_text(json.dumps({
        'status': 'prepared16_unseen_hash_near_publisher_distractor_pairs', 'preplan': str(preplan),
        'pool_queries': queries, 'references': references, 'images': images, 'archives': archives,
        'hashes': hashes, 'nearest_pool': nearest, 'plan': nearest[:16],
        'pillow_version': PIL.__version__, 'pins': pins,
        'scope': 'Original72JPEGs, ETag/range/ZIP CRC verified.16pairs selected before full matching by independent fixed dHash proximity from64unused publisher distractors against8new references. Labels follow published distractor mapping, annotation errors remain possible. Held out from local policy development, not model pretraining; hash-hard retrieval controls, not established semantic/burst or full precision qualification.',
    }, indent=2) + '\n')
    print('Prepared16 held-out hash-near negatives', [x['distance'] for x in nearest[:16]], flush=True)

if __name__ == '__main__':
    main()
