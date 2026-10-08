#!/usr/bin/env python3
"""Independent exact-output, resource, domain and pixel-policy audit."""
import argparse
import hashlib
import json
import math
import struct
from pathlib import Path
p=argparse.ArgumentParser()
for key in ('report','output'):p.add_argument(key,type=Path)
p.add_argument('--checkpoint',action='store_true')
a=p.parse_args();r=json.loads(a.report.read_text());digest=lambda path:hashlib.sha256(Path(path).read_bytes()).hexdigest()
assert r['status']==('running' if a.checkpoint else 'complete')
for path,value in r['input_hashes'].items():assert digest(path)==value,path
manifest=next(path for path in r['input_hashes'] if Path(path).name=='prepared.json');m=json.loads(Path(manifest).read_text());pairs=m['positive_pairs']
if r['negative_query']:
 q=next(v['right'] for v in pairs if v['query_id']==r['negative_query']);pairs=[{'query_id':q['filename']+'/'+v['filename'],'label':'different_publisher_origin','left':v,'right':q} for v in m['images']['original'] if v['group_id']!=q['group_id']]
assert r['required_pairs']==len(pairs)
assert 0<len(r['results'])<=len(pairs)
if not a.checkpoint:assert len(r['results'])==len(pairs)
positives=[];peaks=[]
def exact(v):return json.dumps(v,sort_keys=True,allow_nan=False)
for row,pair in zip(r['results'],pairs):
 assert row['query_id']==pair['query_id'] and row['label']==pair['label']
 for mode in ('direct','collection'):
  native=row[mode];e=native['evidence'];assert type(native['returncode']) is int and native['returncode']==0
  assert exact(json.loads(native['stdout']))==exact(e)
  assert e['status']=='ok' and type(e['candidate']) is bool and type(e['retrieved']) is bool
  assert type(e['managed_used']) is int and e['managed_used']==0
  assert type(e['managed_peak']) is int and 0<=e['managed_peak']<=67108864;peaks.append(e['managed_peak'])
  if e.get('geometry') is not None:
   h=e['geometry'];assert len(h)==3 and all(len(v)==3 for v in h) and all(math.isfinite(n) for v in h for n in v)
   cof=[]
   for i in range(3):
    values=[]
    for j in range(3):
     minor=[[h[y][x] for x in range(3) if x!=j] for y in range(3) if y!=i]
     values.append((-1)**(i+j)*(minor[0][0]*minor[1][1]-minor[0][1]*minor[1][0]))
    cof.append(values)
   determinant=sum(h[0][j]*cof[0][j] for j in range(3));assert math.isfinite(determinant) and determinant!=0
   inverse=[[cof[j][i] for j in range(3)] for i in range(3)]
   for matrix,side in ((h,'left'),(inverse,'right')):
    data=Path(pair[side]['normalized_path']).read_bytes();assert data[:8]==b'\x89PNG\r\n\x1a\n';w,height=struct.unpack('>II',data[16:24]);assert w>0 and height>0
    den=[matrix[2][0]*x+matrix[2][1]*y+matrix[2][2] for x,y in ((0,0),(w-1,0),(0,height-1),(w-1,height-1))]
    assert all(math.isfinite(v) and v!=0 and (v>0)==(den[0]>0) for v in den)
  if e.get('pixels') is not None:
   counts=e['pixels'];assert len(counts)==2
   for v in counts:assert len(v)==3 and all(type(n) is int for n in v) and 0<=v[0]<=v[1]<=v[2] and v[2]>0
   assert e['candidate']==all(c>=1000 and c>=s*.3 and n>=c*.9 for n,c,s in counts)
  else:assert not e['candidate']
 direct=row['direct']['evidence'];collection=row['collection']['evidence']
 if collection['retrieved']:assert exact({k:v for k,v in direct.items() if k not in ('managed_peak','retrieved')})==exact({k:v for k,v in collection.items() if k not in ('managed_peak','retrieved')})
 else:assert not direct['candidate'] and not collection['candidate']
 if direct['candidate']:positives.append(row['query_id'])
if not a.checkpoint:assert r['candidates']==len(positives)
a.output.write_text(json.dumps({'status':'verified_prefix' if a.checkpoint else 'verified_complete_measurements','verified_pairs':len(r['results']),'required_pairs':len(pairs),'candidates':positives,'maximum_managed_peak':max(peaks),'input_hashes':{str(a.report):digest(a.report),str(Path(__file__).resolve()):digest(__file__)},'scope':'Independent native raw/typed output, complete ordered labels, current input pins and pixel acceptance arithmetic; not independent geometric ground truth or broad coverage.'},indent=2)+'\n')
