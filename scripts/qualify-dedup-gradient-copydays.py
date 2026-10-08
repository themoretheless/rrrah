#!/usr/bin/env python3
"""Fixed gradient diagnostic; process/pixel refusals never become negatives."""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import subprocess

p = argparse.ArgumentParser()
for key in ('manifest', 'probe', 'report'):
    p.add_argument(key, type=Path)
p.add_argument('--negative-query')
p.add_argument('--resume', action='store_true')
p.add_argument('--collection', action='store_true', help='Two-source indexed native gradient collection followed by fresh file confirmation.')
p.add_argument('--file-api', action='store_true', help='Use the public managed file lifecycle instead of the diagnostic primitives.')
p.add_argument('--interpolated', action='store_true', help='Explicit interpolated spatial cells through the same file policy.')
a = p.parse_args()
if a.collection:
    a.file_api=True
if a.interpolated and not a.file_api:
    p.error('--interpolated requires --file-api')
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
m = json.loads(a.manifest.read_text())
pairs = m['positive_pairs']
assert len(pairs) == 229
if a.negative_query:
    query = next(v['right'] for v in pairs if v['query_id'] == a.negative_query)
    pairs = [{'query_id': query['filename'] + '/' + image['filename'],
              'left': image, 'right': query, 'label': 'different_publisher_origin'}
             for image in m['images']['original'] if image['group_id'] != query['group_id']]
    assert len(pairs) == 156
r = {'manifest_sha256': digest(a.manifest), 'probe_sha256': digest(a.probe),
     'mode': 'gradient_pyramid_public_files_interpolated' if a.interpolated else 'gradient_pyramid_public_files' if a.file_api else 'gradient_pyramid_three_levels', 'negative_query': a.negative_query,
     'required_pairs': len(pairs), 'results': [],
     'scope': 'Fixed three-level gradient diagnostic with native geometry and radius3 '
              'constrained pixel verification. Explicit pixel/process/time refusals '
              'stay separate. Not file/index lifecycle, managed feature allocations, '
              'semantic/burst or full-library qualification.'}
if a.file_api:
    r['scope'] = ('Public managed gradient file API with source/dependency/cancellation '
                  'guards. Fixed three-level native geometry and radius3 pixel policy. '
                  'Explicit refusals stay separate; not collection, semantic/burst, '
                  'RSS/allocation completeness or full-library qualification.')
if a.collection:
    r['mode']='gradient_pyramid_collection_interpolated' if a.interpolated else 'gradient_pyramid_collection_fixed'
    r['scope']='All requested publisher pairs through two-source indexed gradient collection, followed by fresh file confirmation. Retrieval loss is separate from pixel rejection. Not one full386-image collection, measured scale, semantic/burst or full-library qualification.'
if a.interpolated:
    r['scope'] += ' Explicit spatial-cell interpolation; original fixed matching/geometry/pixel policies retained. Not a replacement default.'


def verify_inputs(pair):
    for side in ('left', 'right'):
        image = pair[side]
        assert digest(Path(image['normalized_path'])) == image['normalized_sha256']


def checkpoint():
    r['completed_pairs'] = len(r['results'])
    r['status_counts'] = dict(collections.Counter(v['status'] for v in r['results']))
    r['candidates'] = sum(v['status'] == 'ok' and v['evidence']['candidate'] for v in r['results'])
    temporary = a.report.with_suffix('.tmp')
    temporary.write_text(json.dumps(r, indent=2) + '\n')
    temporary.replace(a.report)


if a.resume:
    old = json.loads(a.report.read_text())
    for key in ('manifest_sha256', 'probe_sha256', 'mode', 'negative_query', 'required_pairs'):
        assert old[key] == r[key]
    assert old['completed_pairs'] == len(old['results']) <= len(pairs)
    for row, pair in zip(old['results'], pairs):
        assert row['query_id'] == pair['query_id'] and row['label'] == pair['label']
        assert row['status'] in ('ok', 'pixel_refusal', 'process_error', 'timeout')
        if row['status'] in ('ok', 'pixel_refusal'):
            assert row['returncode'] == 0 and row['evidence']['status'] == 'ok'
            assert row['evidence']['managed_used_after_drop'] == 0
        verify_inputs(pair)
    r['results'] = old['results']
    assert old['status_counts'] == dict(collections.Counter(v['status'] for v in r['results']))
    assert old['candidates'] == sum(v['status'] == 'ok' and v['evidence']['candidate'] for v in r['results'])
elif a.report.exists():
    raise RuntimeError('Existing report requires explicit validated --resume.')
checkpoint()
for pair in pairs[len(r['results']):]:
    verify_inputs(pair)
    row = {'query_id': pair['query_id'], 'label': pair['label']}
    try:
        result = subprocess.run([str(a.probe.resolve()), '--gradient-interpolated-collection-pair' if a.collection and a.interpolated else '--gradient-collection-pair' if a.collection else '--gradient-interpolated-file-pair' if a.interpolated else '--gradient-file-pair' if a.file_api else '--gradient-pyramid-pair',
            pair['left']['normalized_path'], pair['right']['normalized_path']],
            capture_output=True, text=True, timeout=180)
        row.update(returncode=result.returncode, stderr=result.stderr)
        if result.returncode:
            row.update(status='process_error', stdout=result.stdout)
        else:
            e = json.loads(result.stdout)
            assert e['status'] == 'ok' and type(e['candidate']) is bool
            assert e['managed_used_after_drop'] == 0
            if a.collection:
                assert type(e['retrieved']) is bool
                assert e['retrieved'] or not e['candidate']
                assert type(e['indexed_features']) is int and 0 <= e['indexed_features'] <= 3000
                assert type(e['descriptor_hits']) is int and 0 <= e['descriptor_hits'] <= 9_000_000
            pixels = e['pixels']
            refusal = pixels is not None and pixels['status'] != 'ok'
            if refusal:
                assert not e['candidate']
            elif pixels is not None:
                counts = pixels['counts']
                assert len(counts) == 2
                for matched, compared, source in counts:
                    assert 0 <= matched <= compared <= source and source > 0
                accepted = all(c >= 1000 and c >= s * .3 and n >= c * .9 for n, c, s in counts)
                assert e['candidate'] == accepted
            else:
                assert e['geometry'] is None and not e['candidate']
            row.update(status='pixel_refusal' if refusal else 'ok', evidence=e)
    except subprocess.TimeoutExpired:
        row.update(status='timeout')
    verify_inputs(pair)
    r['results'].append(row)
    checkpoint()
    print(row['query_id'], row['status'], row.get('evidence', {}).get('candidate'), flush=True)
assert digest(a.probe) == r['probe_sha256']
r['measurement_complete'] = True
checkpoint()
passed = r['status_counts'] == {'ok': len(pairs)} and (not a.negative_query or r['candidates'] == 0)
raise SystemExit(0 if passed else 1)
