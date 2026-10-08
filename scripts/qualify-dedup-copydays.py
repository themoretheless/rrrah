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
p.add_argument('--pyramid-region-grid-collection', action='store_true', help='Indexed regional proposals followed by fresh native automatic-grid confirmation.')
p.add_argument('--pyramid-region-grid', action='store_true', help='Geometry-derived4x4 regional file confirmation; keeps whole candidate separate from regional support.')
p.add_argument('--pyramid-blur', action='store_true', help='Use explicit radius3 linear filtering with32m window-work admission; acceptance thresholds unchanged.')
p.add_argument('--pyramid-collection', action='store_true', help='Use the managed indexed pyramid collection API with the same file policy.')
p.add_argument('--complementary-filter-portfolio', action='store_true', help='Registration and two explicit pyramid filters on shared decoded views;176m cumulative admission.')
p.add_argument('--complementary-filter-collection', action='store_true', help='Indexed union proposals followed by all three native confirmation policies;4000 features/16m hit cap.')
p.add_argument('--five-regions-collection', action='store_true', help='Unified five-family indexed proposals with automatic regional confirmation.')
p.add_argument('--five-collection', action='store_true', help='Union binary and both gradient proposals followed by all five native searches.')
p.add_argument('--complementary-gradient-portfolio', action='store_true', help='All five fixed searches sharing decoded views and atomic cumulative admission.')
p.add_argument('--complementary-gradient', action='store_true', help='All four fixed searches on shared decoded views with atomic source guards.')
p.add_argument('--complementary-file', action='store_true', help='Use both native searches on shared decoded views with cumulative work admission.')
p.add_argument('--negative-query', help='Compare this strong query against every different original-origin group.')
p.add_argument('--resume', action='store_true', help='Validate and continue an interrupted checkpoint without discarding executed pairs.')
a = p.parse_args()

def validate_five_regions(e):
    assert type(e['retrieved']) is bool and type(e['candidate']) is bool
    assert e['managed_used']==0
    assert type(e['managed_peak']) is int and 0<=e['managed_peak']<=64*1024*1024
    assert type(e['indexed_features']) is int and 0<=e['indexed_features']<=10000
    assert type(e['descriptor_hits']) is int and 0<=e['descriptor_hits']<=34_000_000
    assert type(e['region_support_count']) is int and len(e['regions'])<=16
    assert all(type(region['accepted_region']) is bool for region in e['regions'])
    assert e['region_support_count']==sum(region['accepted_region'] for region in e['regions'])
    if not e['retrieved']:assert e['candidate'] is False and e['regions']==[] and e['region_support_count']==0

def validate_indexed_regions(e):
    assert type(e['candidate']) is bool and type(e['retrieved']) is bool
    assert type(e['indexed_features']) is int and 0<=e['indexed_features']<=3000
    assert type(e['descriptor_hits']) is int and 0<=e['descriptor_hits']<=9_000_000
    assert e['managed_used_after_drop']==0
    assert type(e['managed_peak']) is int and 0<=e['managed_peak']<=64*1024*1024
    assert type(e['region_support_count']) is int and len(e['regions'])<=16
    assert all(type(region['accepted_region']) is bool for region in e['regions'])
    assert e['region_support_count']==sum(region['accepted_region'] for region in e['regions'])
    if not e['retrieved']:
        assert e['candidate'] is False and e['geometry'] is None and e['regions']==[] and e['region_support_count']==0

assert sum([a.pyramid_file,a.pyramid_collection,a.complementary_file,a.pyramid_blur,a.pyramid_filter_portfolio,a.complementary_filter_portfolio,a.complementary_filter_collection,a.pyramid_encoded_blur,a.pyramid_region_grid,a.pyramid_region_grid_collection,a.complementary_gradient,a.complementary_gradient_portfolio,a.five_collection,a.five_regions_collection])<=1, 'Select one public API mode.'
use_regions=a.pyramid_region_grid or a.pyramid_region_grid_collection or a.five_regions_collection
use_pyramid=a.pyramid_file or a.pyramid_collection or a.pyramid_blur or a.pyramid_filter_portfolio or a.pyramid_encoded_blur or use_regions
use_native=a.five_collection or a.five_regions_collection or use_pyramid or a.complementary_gradient_portfolio or a.complementary_gradient or a.complementary_file or a.complementary_filter_portfolio or a.complementary_filter_collection
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
if use_regions:
    state['mode']='pyramid_region_grid_collection' if a.pyramid_region_grid_collection else 'pyramid_region_grid'
    state['scope']='Geometry-derived uniform4x4 regions, fixed native pyramid/radius3 policy. Whole candidate unchanged; regional supports are separate evidence, not whole-image copy decisions.544m conservative cumulative regional cap. No label/oracle-based domain selection.'
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
if a.complementary_gradient:
    state['mode']='complementary_gradient'
    state['scope']='Four fixed native searches on one shared decoded-view lifecycle. Original registration/two-filter decisions and additional gradient geometry/pixels reported separately. All phases must finish. Not semantic/burst, all-query precision or full-library qualification.'
if a.complementary_gradient_portfolio:
    state['mode']='complementary_gradient_portfolio'
    state['scope']='Five fixed native searches sharing decoded views, including both gradient recipes. Every phase and final source guard must complete; cumulative work limits apply. Not full-library or semantic/burst qualification.'
if a.five_collection or a.five_regions_collection:
    state['mode']='five_regions_collection' if a.five_regions_collection else 'five_search_collection'
    state['scope']='Two-source indexed binary/two-gradient proposal union followed by all-five shared-view confirmation. Retrieval loss is separate from a pixel rejection. Not full386-image collection, scale, semantic/burst or full-library qualification.'
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
if a.report.exists() and not a.resume:
    raise RuntimeError('Existing report requires explicit validated --resume.')
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
            if a.five_regions_collection:validate_five_regions(row['evidence'])
            if a.pyramid_region_grid_collection:validate_indexed_regions(row['evidence'])
            if a.five_collection or a.five_regions_collection:
                e=row['evidence']
                assert type(e['retrieved']) is bool and (e['retrieved'] or not e['candidate'])
                assert e['managed_used']==0
            if a.complementary_gradient_portfolio or ((a.five_collection or a.five_regions_collection) and row['evidence']['retrieved']):
                e=row['evidence']
                assert e['managed_used']==0
                assert type(e['managed_peak']) is int and 0 < e['managed_peak'] <= 64_000_000
                for field,count in [('five_accepted_searches',5),('four_accepted_searches',4),('joined_accepted_searches',3)]:
                    assert len(e[field])==count and all(type(value) is bool for value in e[field])
                assert e['four_accepted_searches']==e['joined_accepted_searches']+[e['gradient_candidate']]
                assert e['five_accepted_searches']==e['four_accepted_searches']+[e['interpolated_candidate']]
                assert type(e['gradient_candidate']) is bool and type(e['interpolated_candidate']) is bool
                assert e['four_candidate']==any(e['four_accepted_searches'])
                assert e['candidate']==any(e['five_accepted_searches'])
        for side in ['left','right']:
            value=pair[side]
            assert hashlib.sha256(pathlib.Path(value['normalized_path']).read_bytes()).hexdigest()==value['normalized_sha256']
    assert previous['status_counts']==dict(collections.Counter(row['status'] for row in completed))
    assert previous['candidates']==sum(row.get('evidence',{}).get('candidate',False) for row in completed)
    if use_regions:
        assert previous['region_supported_pairs']==sum(row.get('evidence',{}).get('region_support_count',0)>0 for row in completed)
        assert previous['region_support_total']==sum(row.get('evidence',{}).get('region_support_count',0) for row in completed)
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
            command = [str(a.probe.resolve()), '--pyramid-collection-pair' if a.pyramid_collection else '--pyramid-filter-portfolio-pair' if a.pyramid_filter_portfolio else '--pyramid-region-grid-collection-pair' if a.pyramid_region_grid_collection else '--pyramid-region-grid-pair' if a.pyramid_region_grid else '--pyramid-encoded-blur-pair' if a.pyramid_encoded_blur else '--pyramid-blur-pair' if a.pyramid_blur else '--pyramid-file-pair',
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
        if a.complementary_gradient:
            command=[str(a.probe.resolve()),'--complementary-gradient-pair',
                     pair['left']['normalized_path'],pair['right']['normalized_path']]
        if a.complementary_gradient_portfolio:
            command=[str(a.probe.resolve()),'--complementary-gradient-portfolio-pair',
                     pair['left']['normalized_path'],pair['right']['normalized_path']]
        if a.five_collection or a.five_regions_collection:
            command=[str(a.probe.resolve()),'--five-regions-collection-pair' if a.five_regions_collection else '--five-collection-pair',pair['left']['normalized_path'],pair['right']['normalized_path']]
        result = subprocess.run(command, capture_output=True, text=True, timeout=180)
        row.update(returncode=result.returncode, stderr=result.stderr)
        if result.returncode == 0:
            evidence = json.loads(result.stdout)
            if not use_native:
                evidence['identity_diagnostic_counts'] = evidence.pop('oracle_filtered_counts')
            if a.pyramid_region_grid_collection:validate_indexed_regions(evidence)
            if a.five_regions_collection:validate_five_regions(evidence)
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
    if use_regions:
        state['region_supported_pairs']=sum(r.get('evidence',{}).get('region_support_count',0)>0 for r in state['results'])
        state['region_support_total']=sum(r.get('evidence',{}).get('region_support_count',0) for r in state['results'])
    temporary = a.report.with_suffix('.tmp')
    temporary.write_text(json.dumps(state, indent=2) + '\n')
    temporary.replace(a.report)
    print(row['query_id'], row['status'], flush=True)
state['status'] = 'complete_measurement_not_full_library_qualification'
a.report.write_text(json.dumps(state, indent=2) + '\n')
ok=all(r['status']=='ok' for r in state['results'])
if a.negative_query:
    ok=ok and state['candidates']==0
    if use_regions:
        ok=ok and state['region_supported_pairs']==0
raise SystemExit(0 if ok else 1)
