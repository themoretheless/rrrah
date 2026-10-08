#!/usr/bin/env python3
"""Finite supplied-gradient region experiment with pinned inputs and native parity."""
import argparse, hashlib, json, pathlib, subprocess
p=argparse.ArgumentParser();p.add_argument('manifest');p.add_argument('probe');p.add_argument('whole_reference');p.add_argument('region_reference');p.add_argument('output');p.add_argument('query_ids',nargs='+');a=p.parse_args()
def digest(path): return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()
inputs={name:digest(name) for name in (a.manifest,a.probe,a.whole_reference,a.region_reference)}
m=json.loads(pathlib.Path(a.manifest).read_text());pairs={r['query_id']:r for r in m['positive_pairs']}
whole={r['query_id']:r['evidence'] for r in json.loads(pathlib.Path(a.whole_reference).read_text())['results']}
regional={r['query_id']:r['evidence'] for r in json.loads(pathlib.Path(a.region_reference).read_text())['results']}
assert len(set(a.query_ids))==len(a.query_ids)
rows=[];images={};out=pathlib.Path(a.output)
for qid in a.query_ids:
    pair=pairs[qid]
    paths=[r['normalized_path'] for r in (pair['left'],pair['right'])]
    for item in (pair['left'],pair['right']):
        assert digest(item['normalized_path'])==item['normalized_sha256'];images[item['normalized_path']]=item['normalized_sha256']
    r=subprocess.run([a.probe,'--five-all-regions-collection-pair',*paths],capture_output=True,text=True)
    assert r.returncode==0,(qid,r.returncode,r.stderr)
    e=json.loads(r.stdout);assert e['status']=='ok' and e['managed_used']==0 and e['managed_peak']<=64*1024*1024
    assert e['retrieved'] is True,(qid,'finite experiment requires retrieved native models')
    for key,value in whole[qid].items():
        if key!='managed_peak': assert e[key]==value,(qid,'whole',key)
    for key,reference_key in (('regional_geometry','geometry'),('regional_whole_candidate','candidate'),('regions','regions'),('region_support_count','region_support_count')):
        assert e[key]==regional[qid][reference_key],(qid,'regional',key)
    lanes=e['gradient_regions'];assert len(lanes)==2
    for index,lane in enumerate(lanes):
        model=e['gradient_geometry' if index==0 else 'interpolated_geometry']
        assert (lane is None)==(model is None)
        if lane is None: continue
        assert lane['geometry']==model
        assert len(lane['regions'])<=16
        supports=0
        for region in lane['regions']:
            counts=region['counts'];accepted=False
            if counts is not None:
                assert len(counts)==2
                for matched,compared,source in counts:
                    assert all(type(x) is int for x in (matched,compared,source)) and 0<=matched<=compared<=source
                accepted=all(compared>=1000 and compared>=source*0.3 and matched>=compared*0.9 for matched,compared,source in counts)
            assert type(region['accepted_region']) is bool and region['accepted_region']==accepted
            supports+=int(accepted)
        assert lane['region_support_count']==supports
    rows.append({'query_id':qid,'evidence':e})
    assert all(digest(name)==value for name,value in inputs.items())
    assert all(digest(name)==value for name,value in images.items())
    out.write_text(json.dumps({'status':'checkpoint','input_hashes':inputs,'image_hashes':images,'required_pairs':len(a.query_ids),'results':rows},indent=2)+'\n')
    print(json.dumps({'query_id':qid,'whole':e['candidate'],'binary_regions':e['region_support_count'],'gradient_regions':[None if lane is None else lane['region_support_count'] for lane in lanes]}),flush=True)
d=json.loads(out.read_text());d['status']='complete';out.write_text(json.dumps(d,indent=2)+'\n')
