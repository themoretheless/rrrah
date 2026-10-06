#!/usr/bin/env python3
"""Measure every strong Copydays query with fixed native portfolio admission."""
import argparse
import collections
import hashlib
import json
import pathlib
import subprocess
import time

p = argparse.ArgumentParser()
p.add_argument('manifest', type=pathlib.Path)
p.add_argument('probe', type=pathlib.Path)
p.add_argument('report', type=pathlib.Path)
p.add_argument('--pyramid-file', action='store_true', help='Use the managed public pyramid file API with fixed three-level policy.')
p.add_argument('--pyramid-filter-portfolio', action='store_true', help='Two explicit filters share one native pyramid geometry;44m cumulative work.')
p.add_argument('--pyramid-encoded-blur', action='store_true', help='Explicit radius3 encoded-sRGB comparison;32m work, unchanged native geometry and acceptance thresholds.')
p.add_argument('--pyramid-blur', action='store_true', help='Use explicit radius3 linear filtering with32m window-work admission; acceptance thresholds unchanged.')
p.add_argument('--pyramid-collection', action='store_true', help='Use the managed indexed pyramid collection API with the same file policy.')
p.add_argument('--complementary-filter-portfolio', action='store_true', help='Registration and two explicit pyramid filters on shared decoded views;176m cumulative admission.')
p.add_argument('--complementary-filter-collection', action='store_true', help='Indexed union proposals followed by all three native confirmation policies;4000 features/16m hit cap.')
p.add_argument('--complementary-file', action='store_true', help='Use both native searches on shared decoded views with cumulative work admission.')
p.add_argument('--negative-query', help='Compare this strong query against every different original-origin group.')
p.add_argument('--resume', action='store_true', help='Validate and continue an interrupted checkpoint without discarding executed pairs.')
a = p.parse_args()
assert sum([a.pyramid_file,a.pyramid_collection,a.complementary_file,a.pyramid_blur,a.pyramid_filter_portfolio,a.complementary_filter_portfolio,a.complementary_filter_collection,a.pyramid_encoded_blur])<=1, 'Select one public API mode.'
use_pyramid=a.pyramid_file or a.pyramid_collection or a.pyramid_blur or a.pyramid_filter_portfolio or a.pyramid_encoded_blur
use_native=use_pyramid or a.complementary_file or a.complementary_filter_portfolio or a.complementary_filter_collection
m = json.loads(a.manifest.read_text())
assert len(m['positive_pairs']) == m['required_positive_pairs'] == 229
probe_hash = hashlib.sha256(a.probe.read_bytes()).hexdigest()
state = {'manifest_sha256': hashlib.sha256(a.manifest.read_bytes()).hexdigest(),
         'probe_sha256': probe_hash, 'required_pairs': 229, 'results': [],
         'scope': 'All strong-subset publisher-origin pairs. Fixed portfolio admission; '
                  'copy labels do not prove pixel equality. No negative precision gate yet. '
                  'Identity diagnostic supplied to existing probe is not a known homography '
                  'and does not participate in native model selection or acceptance.'}
if use_pyramid:
    state['scope'] = ('All 229 strong-subset publisher-origin pairs through the managed public '
                      'pyramid file API. Fixed three-level constrained photometric policy. '
                      'No oracle geometry. This is copy-origin recovery measurement, not '
                      'full-library qualification or an independent negative precision gate.')
state['mode'] = 'pyramid_collection' if a.pyramid_collection else 'pyramid_file' if a.pyramid_file else 'portfolio'
if a.pyramid_filter_portfolio:
    state['mode']='pyramid_filter_portfolio'
    state['scope']+=' Two explicit radius1/radius3 filters with44m cumulative admission; matching and geometry run once. Both phases must complete.'
if a.pyramid_encoded_blur:
    state['mode']='pyramid_encoded_blur_radius3'
    state['scope']+=' Explicit radius3 encoded-sRGB windows with32m work; same native geometry and numerical acceptance thresholds, separately declared comparison space. Diagnostic not promoted default.'
if a.pyramid_blur:
    state['mode']='pyramid_blur_radius3'
    state['scope']+=' Explicit radius3 linear windows,32m work cap; same geometry, gain/offset bounds, coverage and residual thresholds. Experimental diagnostic, not a promoted default.'
if a.complementary_file:
    state['mode']='complementary_file'
    state['scope']='All229 strong-subset publisher-origin pairs through shared-view complementary native file searches with fixed individual acceptance and cumulative work admission. No oracle geometry, no semantic/burst precision or full-copy certificate.'
if a.complementary_filter_portfolio:
    state['mode']='complementary_filter_portfolio'
    state['scope']='All229 strong pairs through shared decoded views; original fixed registration and pyramid searches plus explicit radius3 pixel verification on unchanged pyramid geometry. All phases complete;176m cumulative work. Not broad coverage or semantic/burst precision.'
if a.complementary_filter_collection:
    state['mode']='complementary_filter_collection'
    state['scope']='All229 strong pairs through indexed union proposals and shared-view three-phase confirmation. Fixed native policies;4000 features,16m retrieval hits including same-file hits. No oracle geometry; not all-query semantic/burst precision or broad full-library qualification.'
if a.pyramid_collection:
    state['scope'] += ' Indexed collection proposal followed by fresh file confirmation.'
pairs=m['positive_pairs']
if a.negative_query:
    assert use_native, 'Negative query mode requires a public native API mode.'
    query=next(pair['right'] for pair in pairs if pair['query_id']==a.negative_query)
    pairs=[{'query_id':query['filename']+'/'+original['filename'],
            'left':original,'right':query,'label':'different_publisher_origin'}
           for original in m['images']['original'] if original['group_id']!=query['group_id']]
    assert len(pairs)==156
    state['required_pairs']=156
    state['scope']=f'One strong query against all156 different publisher-origin groups through fixed public mode {state["mode"]}. Not a general semantic/burst or all-query precision certificate.'
if a.resume:
    checkpoint_bytes=a.report.read_bytes()
    previous=json.loads(checkpoint_bytes)
    for key in ['manifest_sha256','probe_sha256','required_pairs','mode']:
        assert previous[key]==state[key], f'Checkpoint {key} does not match requested run.'
    completed=previous['results']
    assert len(completed)<=len(pairs) and previous['completed_pairs']==len(completed)
    for row,pair in zip(completed,pairs):
        assert row['query_id']==pair['query_id'] and row['label']==pair['label']
        assert row['status'] in ['ok','error','timeout','process_error']
        if row['status']=='ok':
            assert row['returncode']==0 and row['evidence']['status']=='ok'
            assert isinstance(row['evidence']['candidate'],bool)
        for side in ['left','right']:
            value=pair[side]
            assert hashlib.sha256(pathlib.Path(value['normalized_path']).read_bytes()).hexdigest()==value['normalized_sha256']
    assert previous['status_counts']==dict(collections.Counter(row['status'] for row in completed))
    assert previous['candidates']==sum(row.get('evidence',{}).get('candidate',False) for row in completed)
    state=previous
    state.pop('status',None)
    state.setdefault('resume_events',[]).append({'completed_pairs':len(completed),'checkpoint_sha256':hashlib.sha256(checkpoint_bytes).hexdigest(),'reason':'Explicit resume after prior process disappearance; pinned probe, manifest, ordered results and prior inputs validated.'})
    pairs=pairs[len(completed):]
for pair in pairs:
    row = {'query_id': pair['query_id'], 'label': pair['label']}
    started = time.monotonic()
    try:
        for side in ['left', 'right']:
            v = pair[side]
            assert hashlib.sha256(pathlib.Path(v['normalized_path']).read_bytes()).hexdigest() == v['normalized_sha256']
        assert hashlib.sha256(a.probe.read_bytes()).hexdigest() == probe_hash
        command = [str(a.probe.resolve()), '--projective-pair-portfolio',
                   pair['left']['normalized_path'], pair['right']['normalized_path'],
                   '1', '0', '0', '0', '1', '0', '0', '0', '1']
        if use_pyramid:
            command = [str(a.probe.resolve()), '--pyramid-collection-pair' if a.pyramid_collection else '--pyramid-filter-portfolio-pair' if a.pyramid_filter_portfolio else '--pyramid-encoded-blur-pair' if a.pyramid_encoded_blur else '--pyramid-blur-pair' if a.pyramid_blur else '--pyramid-file-pair',
                       pair['left']['normalized_path'], pair['right']['normalized_path']]
        if a.complementary_file:
            command=[str(a.probe.resolve()),'--complementary-file-pair',
                     pair['left']['normalized_path'],pair['right']['normalized_path']]
        if a.complementary_filter_portfolio:
            command=[str(a.probe.resolve()),'--complementary-filter-portfolio-pair',
                     pair['left']['normalized_path'],pair['right']['normalized_path']]
        if a.complementary_filter_collection:
            command=[str(a.probe.resolve()),'--complementary-filter-collection-pair',
                     pair['left']['normalized_path'],pair['right']['normalized_path']]
        result = subprocess.run(command, capture_output=True, text=True, timeout=180)
        row.update(returncode=result.returncode, stderr=result.stderr)
        if result.returncode == 0:
            evidence = json.loads(result.stdout)
            if not use_native:
                evidence['identity_diagnostic_counts'] = evidence.pop('oracle_filtered_counts')
            row.update(evidence=evidence, status=evidence['status'])
        else:
            row.update(status='process_error', stdout=result.stdout)
    except subprocess.TimeoutExpired:
        row['status'] = 'timeout'
    except Exception as error:
        row.update(status='error', error=str(error))
    row['seconds'] = time.monotonic() - started
    state['results'].append(row)
    state['completed_pairs'] = len(state['results'])
    state['status_counts'] = dict(collections.Counter(r['status'] for r in state['results']))
    state['candidates'] = sum(r.get('evidence', {}).get('candidate', False) for r in state['results'])
    temporary = a.report.with_suffix('.tmp')
    temporary.write_text(json.dumps(state, indent=2) + '\n')
    temporary.replace(a.report)
    print(row['query_id'], row['status'], flush=True)
state['status'] = 'complete_measurement_not_full_library_qualification'
a.report.write_text(json.dumps(state, indent=2) + '\n')
ok=all(r['status']=='ok' for r in state['results'])
if a.negative_query:
    ok=ok and state['candidates']==0
raise SystemExit(0 if ok else 1)
